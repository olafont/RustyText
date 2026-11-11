//! Emojis offered by the Emoji menu, grouped by category.
//!
//! Only single-codepoint emojis: egui can't combine multi-codepoint sequences
//! (skin tones, families, flags), so those would render as separate glyphs.

pub const CATEGORIES: &[(&str, &[&str])] = &[
    ("Smileys", &[
        "😀", "😃", "😄", "😁", "😆", "😅", "😂", "🙂",
        "😉", "😊", "😇", "😍", "😘", "😎", "🤓", "🤔",
        "😐", "😴", "😮", "😢", "😭", "😡", "🤯", "🥳",
    ]),
    ("Gestures", &[
        "👍", "👎", "👌", "👏", "🙌", "🙏", "💪", "👋",
        "🤝", "✋", "👉", "👀",
    ]),
    ("Symbols", &[
        "💖", "💔", "💯", "✅", "❌", "⚡", "🔥", "⭐",
        "💡", "❓", "❗", "🎉", "🚀", "💤", "🎵", "🏆",
    ]),
    ("Animals & Nature", &[
        "🦀", "🐍", "🐶", "🐱", "🦊", "🐻", "🐼", "🐸",
        "🐵", "🦄", "🐝", "🐢", "🐙", "🌲", "🌵", "🌸",
        "🌻", "🍀", "🌈", "🌙", "🌍", "🌊",
    ]),
    ("Food & Drink", &[
        "🍎", "🍌", "🍉", "🍇", "🍓", "🍒", "🍕", "🍔",
        "🌮", "🍣", "🍩", "🍪", "🎂", "☕", "🍺", "🍷",
    ]),
    ("Objects", &[
        "💻", "📱", "📁", "📄", "📝", "📌", "📎", "📦",
        "🔒", "🔑", "🔧", "🔨", "🐛", "📅", "⏰", "🔍",
    ]),
    ("Travel & Places", &[
        "🚗", "🚲", "🚂", "🚢", "🏠", "🏰", "🗻", "🌋",
        "⛺", "🎡", "🏖", "🌃",
    ]),
];
