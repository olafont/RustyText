//! Lets agents talk to the running app, over MCP (the Model Context Protocol).
//!
//! When "Let agents connect" is on in Preferences, the app listens on
//! `http://127.0.0.1:47823/mcp`. An agent sends JSON-RPC messages there with
//! HTTP POST (MCP's "Streamable HTTP" transport, answering each request with
//! plain JSON) and gets a list of tools it can call: read a document, edit it…
//!
//! This module only knows the protocol. The tools themselves are in
//! `app/agent_tools.rs`, because they need the windows and documents, which
//! belong to the UI thread. So a tool call travels like this:
//!
//! 1. a connection thread here receives it and puts a `Call` in a channel,
//! 2. it wakes the app, which runs the tool on its next frame and answers,
//! 3. the connection thread sends the answer back to the agent.
//!
//! Only programs on this computer can connect, and web pages are turned away
//! (see `is_allowed`), since a page in your browser can reach 127.0.0.1 too.

use std::io::{self, BufRead, BufReader, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};

pub const PORT: u16 = 47823;

/// How long a tool call waits for the app (it can be busy showing a file panel).
const ANSWER_TIMEOUT: Duration = Duration::from_secs(10);

/// The largest message accepted, to bound what a connection can make us hold.
const MAX_BODY: usize = 32 * 1024 * 1024;

/// Protocol versions this server speaks, newest first.
const PROTOCOL_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26"];

const INSTRUCTIONS: &str = "RustyText is a text editor running on the user's computer. \
These tools act on the documents open in it, live: the user sees every change at once. \
Call list_documents first to see what is open. Edits are not saved to disk until \
save_document is called (the Forever Note saves itself).";

/// The address agents connect to.
pub fn url() -> String {
    format!("http://127.0.0.1:{PORT}/mcp")
}

/// A tool call from an agent, waiting for the app to answer it.
pub struct Call {
    pub tool: String,
    pub arguments: Value,
    reply: mpsc::Sender<Result<String, String>>,
}

impl Call {
    /// `Ok` is the tool's output; `Err` is a message telling the agent what went wrong.
    pub fn respond(self, result: Result<String, String>) {
        let _ = self.reply.send(result); // the agent may have given up waiting
    }
}

/// What the connection threads share.
struct Shared {
    /// The tools, as MCP's `tools/list` describes them.
    tools: Value,
    calls: mpsc::Sender<Call>,
    /// Asks the app to run a frame, so it sees the new call.
    wake: Box<dyn Fn() + Send + Sync>,
}

/// The listening server. Dropping it stops listening.
pub struct Server {
    stop: Arc<AtomicBool>,
    calls: mpsc::Receiver<Call>,
    #[cfg_attr(not(test), expect(dead_code, reason = "only the tests listen on a port they must ask for"))]
    port: u16,
}

impl Server {
    /// Listens on `port` of this computer only (0 picks a free port).
    /// `tools` is a JSON array of tool descriptions; `wake` is called from
    /// another thread whenever a call is waiting in `calls()`.
    pub fn start(port: u16, tools: Value, wake: impl Fn() + Send + Sync + 'static) -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))?;
        let port = listener.local_addr()?.port();
        // Non-blocking, so the loop below can notice `stop` between connections.
        listener.set_nonblocking(true)?;
        let stop = Arc::new(AtomicBool::new(false));
        let (sender, calls) = mpsc::channel();
        let shared = Arc::new(Shared { tools, calls: sender, wake: Box::new(wake) });

        let stopped = Arc::clone(&stop);
        thread::spawn(move || {
            while !stopped.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let shared = Arc::clone(&shared);
                        thread::spawn(move || {
                            let _ = handle_connection(stream, &shared); // a broken connection only affects itself
                        });
                    }
                    Err(_) => thread::sleep(Duration::from_millis(20)), // usually: nobody is connecting
                }
            }
        });
        Ok(Self { stop, calls, port })
    }

    /// The calls that arrived since last time. The app answers each with `Call::respond`.
    pub fn calls(&self) -> impl Iterator<Item = Call> + '_ {
        self.calls.try_iter()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

// MARK: - HTTP

struct HttpRequest {
    method: String,
    path: String,
    /// Names in lower case.
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl HttpRequest {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(header, _)| header == name).map(|(_, value)| value.as_str())
    }
}

/// One request per connection: read it, answer it, close.
fn handle_connection(stream: TcpStream, shared: &Shared) -> io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let (status, body) = match read_request(&mut reader) {
        Ok(request) => answer(&request, shared),
        Err(_) => (400, None),
    };
    write_response(stream, status, body.as_deref())
}

fn read_request(reader: &mut impl BufRead) -> io::Result<HttpRequest> {
    let invalid = |what: &str| io::Error::new(io::ErrorKind::InvalidData, what.to_owned());

    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let method = parts.next().ok_or_else(|| invalid("no method"))?.to_owned();
    let path = parts.next().ok_or_else(|| invalid("no path"))?.to_owned();

    let mut headers = Vec::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            return Err(invalid("the headers were cut short"));
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        let (name, value) = line.split_once(':').ok_or_else(|| invalid("a header without a colon"))?;
        headers.push((name.trim().to_ascii_lowercase(), value.trim().to_owned()));
    }

    let mut request = HttpRequest { method, path, headers, body: Vec::new() };
    let length: usize = match request.header("content-length") {
        Some(length) => length.parse().map_err(|_| invalid("content-length is not a number"))?,
        None => 0,
    };
    if length > MAX_BODY {
        return Err(invalid("the message is too large"));
    }
    request.body.resize(length, 0);
    reader.read_exact(&mut request.body)?;
    Ok(request)
}

fn write_response(mut stream: TcpStream, status: u16, body: Option<&str>) -> io::Result<()> {
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        _ => "Method Not Allowed",
    };
    let body = body.unwrap_or_default();
    let mut head = format!("HTTP/1.1 {status} {reason}\r\nConnection: close\r\nContent-Length: {}\r\n", body.len());
    if !body.is_empty() {
        head.push_str("Content-Type: application/json\r\n");
    }
    if status == 405 {
        head.push_str("Allow: POST\r\n");
    }
    stream.write_all(format!("{head}\r\n{body}").as_bytes())?;
    stream.flush()
}

/// Whether the request comes from a program on this computer talking to us on purpose.
///
/// A web page can make the browser send requests to 127.0.0.1. Two headers give
/// it away: `Origin` (the page's site, which browsers always add to a POST from
/// a page) and `Host` (the name the browser thinks it is talking to, which is
/// the attacker's own domain in a "DNS rebinding" attack).
fn is_allowed(request: &HttpRequest) -> bool {
    let local = |authority: &str| {
        let host = match authority.rsplit_once(':') {
            Some((host, port)) if port.bytes().all(|byte| byte.is_ascii_digit()) => host,
            _ => authority,
        };
        matches!(host, "127.0.0.1" | "localhost" | "[::1]")
    };
    let origin_ok = request.header("origin").is_none_or(|origin| origin.strip_prefix("http://").is_some_and(local));
    origin_ok && request.header("host").is_some_and(local)
}

/// The HTTP status and JSON body to answer `request` with.
fn answer(request: &HttpRequest, shared: &Shared) -> (u16, Option<String>) {
    if !is_allowed(request) {
        return (403, None);
    }
    if request.path.split('?').next() != Some("/mcp") {
        return (404, None);
    }
    if request.method != "POST" {
        return (405, None); // no event stream to GET, no session to DELETE
    }
    let Ok(message) = serde_json::from_slice::<Value>(&request.body) else {
        return (400, Some(error_response(Value::Null, -32700, "Parse error").to_string()));
    };
    match handle_message(&message, shared) {
        Some(response) => (200, Some(response.to_string())),
        None => (202, None), // a notification: nothing to answer
    }
}

// MARK: - JSON-RPC

fn error_response(id: Value, code: i32, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// Answers one JSON-RPC message. Notifications (no `id`) get no answer.
fn handle_message(message: &Value, shared: &Shared) -> Option<Value> {
    let id = message.get("id").filter(|id| !id.is_null()).cloned();
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        // Not a request. The agent's answers to us would land here, but we never ask anything.
        return (!message.is_object()).then(|| error_response(Value::Null, -32600, "Invalid request"));
    };
    let id = id?;
    let params = message.get("params").cloned().unwrap_or(Value::Null);

    let result = match method {
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or_default();
            let version = PROTOCOL_VERSIONS.iter().find(|version| **version == asked).unwrap_or(&PROTOCOL_VERSIONS[1]);
            json!({
                "protocolVersion": version,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "rustytext", "title": "RustyText", "version": env!("CARGO_PKG_VERSION") },
                "instructions": INSTRUCTIONS,
            })
        }
        "ping" => json!({}),
        "tools/list" => json!({ "tools": shared.tools }),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
            let known = shared.tools.as_array().is_some_and(|tools| tools.iter().any(|tool| tool["name"] == name));
            if !known {
                return Some(error_response(id, -32602, &format!("Unknown tool: {name}")));
            }
            let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            let outcome = call_tool(name, arguments, shared);
            json!({
                "content": [{ "type": "text", "text": outcome.as_ref().unwrap_or_else(|message| message) }],
                "isError": outcome.is_err(),
            })
        }
        _ => return Some(error_response(id, -32601, &format!("Method not found: {method}"))),
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

/// Hands the call to the app and waits for its answer.
fn call_tool(name: &str, arguments: Value, shared: &Shared) -> Result<String, String> {
    let (reply, answer) = mpsc::channel();
    let call = Call { tool: name.to_owned(), arguments, reply };
    shared.calls.send(call).map_err(|_| "RustyText stopped listening to agents.".to_owned())?;
    (shared.wake)();
    answer
        .recv_timeout(ANSWER_TIMEOUT)
        .unwrap_or_else(|_| Err("RustyText did not answer in time. It may be showing a file panel or a dialog.".to_owned()))
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::*;

    fn tools() -> Value {
        json!([{ "name": "echo", "description": "Says it back.", "inputSchema": { "type": "object" } }])
    }

    /// Sends one HTTP request to the server and returns the status and body of the answer.
    fn send(port: u16, method: &str, headers: &str, body: &str) -> (u16, String) {
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        let request = format!("{method} /mcp HTTP/1.1\r\n{headers}Content-Length: {}\r\n\r\n{body}", body.len());
        stream.write_all(request.as_bytes()).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        let status = response.split_whitespace().nth(1).unwrap().parse().unwrap();
        let body = response.split_once("\r\n\r\n").map_or("", |(_, body)| body).to_owned();
        (status, body)
    }

    fn post(port: u16, body: &Value) -> (u16, Value) {
        let (status, body) = send(port, "POST", "Host: 127.0.0.1\r\n", &body.to_string());
        (status, serde_json::from_str(&body).unwrap_or(Value::Null))
    }

    #[test]
    fn an_agent_can_connect_list_tools_and_call_one() {
        let server = Server::start(0, tools(), || {}).unwrap();
        let port = server.port;

        let (status, hello) = post(
            port,
            &json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-03-26" } }),
        );
        assert_eq!(status, 200);
        assert_eq!(hello["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(hello["result"]["serverInfo"]["name"], "rustytext");

        let (status, _) = post(port, &json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        assert_eq!(status, 202, "notifications are accepted without an answer");

        let (_, list) = post(port, &json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }));
        assert_eq!(list["result"]["tools"][0]["name"], "echo");

        // The "app": answers the call when it arrives.
        let agent = thread::spawn(move || {
            let call = json!({ "name": "echo", "arguments": { "text": "hi" } });
            post(port, &json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": call }))
        });
        let call = loop {
            if let Some(call) = server.calls().next() {
                break call;
            }
            thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(call.tool, "echo");
        let text = call.arguments["text"].as_str().unwrap().to_owned();
        call.respond(Ok(text));
        let (_, called) = agent.join().unwrap();
        assert_eq!(called["result"]["content"][0]["text"], "hi");
        assert_eq!(called["result"]["isError"], false);
    }

    #[test]
    fn unknown_tools_and_methods_are_errors() {
        let server = Server::start(0, tools(), || {}).unwrap();
        let call = json!({ "name": "nope" });
        let (_, answer) = post(server.port, &json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": call }));
        assert_eq!(answer["error"]["code"], -32602);
        let (_, answer) = post(server.port, &json!({ "jsonrpc": "2.0", "id": 2, "method": "resources/list" }));
        assert_eq!(answer["error"]["code"], -32601);
        let (status, _) = send(server.port, "POST", "Host: localhost\r\n", "not json");
        assert_eq!(status, 400);
    }

    #[test]
    fn web_pages_are_turned_away() {
        let server = Server::start(0, tools(), || {}).unwrap();
        let ping = json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" }).to_string();
        let status = |headers: &str| send(server.port, "POST", headers, &ping).0;

        assert_eq!(status("Host: 127.0.0.1:47823\r\n"), 200);
        assert_eq!(status("Host: localhost\r\nOrigin: http://localhost:3000\r\n"), 200);
        assert_eq!(status("Host: 127.0.0.1\r\nOrigin: https://evil.example\r\n"), 403, "a page on another site");
        assert_eq!(status("Host: evil.example\r\n"), 403, "DNS rebinding");
        assert_eq!(status(""), 403, "no Host header");
        assert_eq!(send(server.port, "GET", "Host: 127.0.0.1\r\n", "").0, 405);
    }

    #[test]
    fn stops_listening_when_dropped() {
        let server = Server::start(0, tools(), || {}).unwrap();
        let port = server.port;
        drop(server);
        thread::sleep(Duration::from_millis(100));
        assert!(TcpStream::connect((Ipv4Addr::LOCALHOST, port)).is_err());
    }
}
