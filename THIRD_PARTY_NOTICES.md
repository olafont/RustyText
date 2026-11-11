# Third-party notices

RustyText is licensed under the GNU General Public License version 3 ([LICENSE](LICENSE)).
It includes or is built from the works below, under their own licenses, all compatible with
the GPL-3.0.

## Adapted code

### Marine Data Simulator and SolidCraft

`src/title_bar.rs` (the title bar under the macOS traffic lights: hidden OS title bar, drag
area, double-click to zoom) follows Marine Data Simulator's `crates/app/src/ui/title_bar.rs`
(GPL-3.0-only, same author), which itself follows SolidCraft's
`crates/ui-egui/src/window_chrome.rs` (https://github.com/tangency-labs/solidcraft), licensed
MIT OR Apache-2.0. Used here under the MIT License:

```text
MIT License

Copyright (c) 2026 SolidCraft contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Adapted designs

### glow and glamour

The Markdown pane (`src/glow.rs`) imitates the look of Charm's `glow` terminal app
(https://github.com/charmbracelet/glow): its colours are those of the "dark" style of the
glamour library (https://github.com/charmbracelet/glamour), both MIT-licensed,
© Charmbracelet, Inc. No code from either is included; the rendering is written for RustyText.

## Rust crates

The app is built from the Rust crates listed in `Cargo.lock`, among them egui and eframe (UI),
winit (windows), rfd (file panels), arboard (clipboard), portable-pty and vt100 (terminal),
pulldown-cmark (Markdown), chrono (dates) and serde_json (MCP messages). Their licenses are
permissive: MIT, Apache-2.0 (some with the LLVM exception), BSD-2-Clause, BSD-3-Clause, ISC,
Zlib, Unicode-3.0, BSL-1.0, 0BSD or Unlicense; where a crate offers a choice, a permissive one
is used. Each crate's license text and copyright notice ship with its source (`cargo vendor`
or crates.io).

To list them: `cargo metadata --format-version 1` (field `license` of each package).

## Fonts

egui bundles fonts for text and symbols (crate `epaint_default_fonts`, which carries their
license texts): Ubuntu-Light (Ubuntu Font Licence 1.0), Hack (MIT; its Bitstream Vera parts
under the Bitstream Vera License), Noto Emoji (SIL Open Font License 1.1) and emoji-icon-font
(MIT). They are compiled into the app.

## Icon

The app icon (`assets/icon.svg` and the PNGs rendered from it) was made for RustyText and is
covered by the same license as the rest of the project.
