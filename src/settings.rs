//! Your preferences: theme, language, default folder, and whether agents may connect.
//!
//! They live in `~/.rustytext/settings`, a small text file of `key = value` lines:
//!
//! ```text
//! theme = dark
//! language = fr
//! agents = on
//! default_folder = /Users/you/Documents
//! ```
//!
//! The current settings are kept in one place for the whole app (`get()`), since
//! every window, menu and dialog reads them. The Preferences dialog changes them
//! with `set()`, which also writes the file.

use std::path::PathBuf;
use std::sync::RwLock;
use std::{fs, io};

use eframe::egui;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Theme {
    /// Follow the system's light or dark mode.
    #[default]
    System,
    Light,
    Dark,
}

impl Theme {
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    /// The English name, shown (translated) in Preferences.
    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    /// The word used in the settings file.
    fn key(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub fn preference(self) -> egui::ThemePreference {
        match self {
            Self::System => egui::ThemePreference::System,
            Self::Light => egui::ThemePreference::Light,
            Self::Dark => egui::ThemePreference::Dark,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Language {
    #[default]
    English,
    French,
}

impl Language {
    pub const ALL: [Self; 2] = [Self::English, Self::French];

    /// The language's own name, which is what a language list shows.
    pub fn name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::French => "Français",
        }
    }

    /// The code used in the settings file.
    fn key(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::French => "fr",
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Settings {
    pub theme: Theme,
    pub language: Language,
    /// Where the Open and Save panels start, and the terminal for an unsaved document.
    pub default_folder: Option<PathBuf>,
    /// Let agents connect to the app (see `mcp.rs`). Off unless you turn it on.
    pub agent_server: bool,
}

impl Settings {
    /// Reads the settings file's text. Lines it doesn't understand are skipped,
    /// so a damaged file still gives usable settings.
    fn parse(text: &str) -> Self {
        let mut settings = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else { continue };
            let value = value.trim();
            match key.trim() {
                "theme" => {
                    if let Some(theme) = Theme::ALL.into_iter().find(|theme| theme.key() == value) {
                        settings.theme = theme;
                    }
                }
                "language" => {
                    if let Some(language) = Language::ALL.into_iter().find(|language| language.key() == value) {
                        settings.language = language;
                    }
                }
                "agents" => settings.agent_server = value == "on",
                "default_folder" if !value.is_empty() => settings.default_folder = Some(PathBuf::from(value)),
                _ => {}
            }
        }
        settings
    }

    fn to_text(&self) -> String {
        let agents = if self.agent_server { "on" } else { "off" };
        let mut text = format!("theme = {}\nlanguage = {}\nagents = {agents}\n", self.theme.key(), self.language.key());
        if let Some(folder) = &self.default_folder {
            text.push_str(&format!("default_folder = {}\n", folder.display()));
        }
        text
    }
}

static CURRENT: RwLock<Settings> =
    RwLock::new(Settings { theme: Theme::System, language: Language::English, default_folder: None, agent_server: false });

fn path() -> Option<PathBuf> {
    std::env::home_dir().map(|home| home.join(".rustytext").join("settings"))
}

/// Reads the settings file, once, when the app starts. Without one, the defaults stay.
pub fn load() {
    if let Some(text) = path().and_then(|path| fs::read_to_string(path).ok()) {
        *CURRENT.write().unwrap_or_else(|poisoned| poisoned.into_inner()) = Settings::parse(&text);
    }
}

pub fn get() -> Settings {
    CURRENT.read().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
}

pub fn agent_server() -> bool {
    CURRENT.read().unwrap_or_else(|poisoned| poisoned.into_inner()).agent_server
}

pub fn language() -> Language {
    CURRENT.read().unwrap_or_else(|poisoned| poisoned.into_inner()).language
}

/// The default folder, if one is set and still exists.
pub fn default_folder() -> Option<PathBuf> {
    get().default_folder.filter(|folder| folder.is_dir())
}

/// Makes `settings` the current ones and writes them to the settings file.
/// They apply even if the write fails; they just won't be there next time.
pub fn set(settings: Settings) -> io::Result<()> {
    let text = settings.to_text();
    *CURRENT.write().unwrap_or_else(|poisoned| poisoned.into_inner()) = settings;
    if cfg!(test) {
        return Ok(()); // tests must never touch your real settings file
    }
    let Some(path) = path() else { return Ok(()) };
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }
    fs::write(path, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_survive_a_round_trip() {
        let settings = Settings {
            theme: Theme::Dark,
            language: Language::French,
            default_folder: Some(PathBuf::from("/Users/you/My Documents")),
            agent_server: true,
        };
        assert_eq!(Settings::parse(&settings.to_text()), settings);
        assert_eq!(Settings::parse(&Settings::default().to_text()), Settings::default());
    }

    #[test]
    fn unknown_lines_are_skipped() {
        let settings = Settings::parse("nonsense\ntheme = purple\nlanguage=fr\nfont = big\ndefault_folder =\n");
        assert_eq!(settings, Settings { language: Language::French, ..Settings::default() });
    }
}
