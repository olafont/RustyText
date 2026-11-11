//! Translations of the interface.
//!
//! The code is written in English: `tr("Save")` gives the text in the language
//! chosen in Preferences, or the English text itself when there's no translation.
//! To add a language, add it to `settings::Language` and give it a table here.

use crate::settings::{self, Language};

/// `english` in the current language.
pub fn tr(english: &'static str) -> &'static str {
    translate(settings::language(), english)
}

/// Like `tr`, for text with blanks: each `{}` is replaced by the next of `values`.
pub fn tr_fill(english: &'static str, values: &[&str]) -> String {
    fill(tr(english), values)
}

pub fn translate(language: Language, english: &'static str) -> &'static str {
    let table = match language {
        Language::English => return english,
        Language::French => FRENCH,
    };
    table.iter().find(|(source, _)| *source == english).map_or(english, |(_, translation)| translation)
}

fn fill(template: &str, values: &[&str]) -> String {
    let mut parts = template.split("{}");
    let mut text = parts.next().unwrap_or_default().to_owned();
    for (index, part) in parts.enumerate() {
        text.push_str(values.get(index).copied().unwrap_or("{}"));
        text.push_str(part);
    }
    text
}

const FRENCH: &[(&str, &str)] = &[
    // Menus
    ("File", "Fichier"),
    ("New", "Nouveau"),
    ("Open…", "Ouvrir…"),
    ("New Window", "Nouvelle fenêtre"),
    ("Close", "Fermer"),
    ("Save", "Enregistrer"),
    ("Save As…", "Enregistrer sous…"),
    ("Preferences…", "Préférences…"),
    ("Quit", "Quitter"),
    ("Edit", "Édition"),
    ("Undo", "Annuler"),
    ("Redo", "Rétablir"),
    ("Cut", "Couper"),
    ("Copy", "Copier"),
    ("Paste", "Coller"),
    ("Select All", "Tout sélectionner"),
    ("Emoji", "Emoji"),
    ("Find", "Rechercher"),
    ("Find…", "Rechercher…"),
    ("Find & Replace…", "Rechercher et remplacer…"),
    ("Find Next", "Rechercher le suivant"),
    ("Find Previous", "Rechercher le précédent"),
    ("Use Selection for Find", "Rechercher la sélection"),
    ("Text", "Texte"),
    ("Go to Line…", "Aller à la ligne…"),
    ("Soft Wrap", "Retour à la ligne automatique"),
    ("Tools", "Outils"),
    ("Word Count…", "Nombre de mots…"),
    ("Markdown", "Markdown"),
    ("Forever Note", "Forever Note"),
    ("Terminal", "Terminal"),
    // Text tools
    ("Change Case", "Changer la casse"),
    ("Lines", "Lignes"),
    ("Whitespace", "Espaces"),
    ("UPPERCASE", "MAJUSCULES"),
    ("lowercase", "minuscules"),
    ("Title Case", "Majuscule À Chaque Mot"),
    ("snake_case", "snake_case"),
    ("camelCase", "camelCase"),
    ("Sort Lines", "Trier les lignes"),
    ("Reverse Lines", "Inverser l’ordre des lignes"),
    ("Remove Duplicate Lines", "Supprimer les lignes en double"),
    ("Remove Blank Lines", "Supprimer les lignes vides"),
    ("Trim Trailing Whitespace", "Supprimer les espaces en fin de ligne"),
    ("Tabs to Spaces", "Tabulations en espaces"),
    ("Leading Spaces to Tabs", "Espaces de début de ligne en tabulations"),
    ("Normalize Line Endings (LF)", "Normaliser les fins de ligne (LF)"),
    // Emoji categories
    ("Smileys", "Smileys"),
    ("Gestures", "Gestes"),
    ("Symbols", "Symboles"),
    ("Animals & Nature", "Animaux et nature"),
    ("Food & Drink", "Nourriture et boissons"),
    ("Objects", "Objets"),
    ("Travel & Places", "Voyages et lieux"),
    // Tabs, find bar, status bar
    ("untitled text", "texte sans titre"),
    ("Find:", "Rechercher :"),
    ("Previous", "Précédent"),
    ("Next", "Suivant"),
    ("Match case", "Respecter la casse"),
    ("Replace…", "Remplacer…"),
    ("Done", "Terminé"),
    ("With:", "Par :"),
    ("Replace", "Remplacer"),
    ("Replace All", "Tout remplacer"),
    ("L: {}  C: {}    {} lines  {} chars    UTF-8", "L : {}  C : {}    {} lignes  {} caractères    UTF-8"),
    // Dialogs
    ("Go to Line", "Aller à la ligne"),
    ("Go", "Aller"),
    ("Cancel", "Annuler"),
    ("Save changes to “{}” before closing?", "Enregistrer les modifications de « {} » avant de fermer ?"),
    ("Don’t Save", "Ne pas enregistrer"),
    ("OK", "OK"),
    ("Word Count", "Nombre de mots"),
    ("Word Count (selection)", "Nombre de mots (sélection)"),
    ("Words", "Mots"),
    ("Characters", "Caractères"),
    ("Characters (no spaces)", "Caractères (sans les espaces)"),
    ("Paragraphs", "Paragraphes"),
    ("Closing this window will quit RustyText", "Fermer cette fenêtre quittera RustyText"),
    ("This also closes 1 other window.", "Cela ferme aussi 1 autre fenêtre."),
    ("This also closes the other windows.", "Cela ferme aussi les autres fenêtres."),
    (
        "You’ll be asked about any unsaved changes.",
        "Une confirmation sera demandée pour les modifications non enregistrées.",
    ),
    // Preferences
    ("Preferences", "Préférences"),
    ("Theme", "Thème"),
    ("System", "Système"),
    ("Light", "Clair"),
    ("Dark", "Sombre"),
    ("Language", "Langue"),
    ("Default folder", "Dossier par défaut"),
    ("Not set", "Non défini"),
    ("Choose…", "Choisir…"),
    ("Clear", "Effacer"),
    ("Where the Open and Save panels start.", "Là où s’ouvrent les panneaux Ouvrir et Enregistrer."),
    ("Agents", "Agents"),
    ("Let agents connect (MCP)", "Autoriser la connexion des agents (MCP)"),
    ("Agents can read and change your open documents.", "Les agents peuvent lire et modifier vos documents ouverts."),
    // Errors
    ("Couldn’t read the clipboard: {}", "Impossible de lire le presse-papiers : {}"),
    ("Couldn’t open {}: {}", "Impossible d’ouvrir {} : {}"),
    ("Couldn’t open your notes: {}", "Impossible d’ouvrir vos notes : {}"),
    ("Couldn’t save {}: {}", "Impossible d’enregistrer {} : {}"),
    ("Couldn’t save your preferences: {}", "Impossible d’enregistrer vos préférences : {}"),
    ("Couldn’t listen on port {}: {}", "Impossible d’écouter sur le port {} : {}"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emoji;
    use crate::settings::Theme;
    use crate::tools::TextTool;

    /// Every `tr("…")` and `tr_fill("…")` text in `source`.
    fn translated_texts(source: &'static str) -> Vec<&'static str> {
        let mut texts = Vec::new();
        for call in ["tr(\"", "tr_fill(\""] {
            let mut rest = source;
            while let Some(start) = rest.find(call) {
                rest = &rest[start + call.len()..];
                let end = rest.find('"').expect("the text ends with a quote");
                texts.push(&rest[..end]);
            }
        }
        texts
    }

    #[test]
    fn fills_blanks_in_order() {
        assert_eq!(fill("Couldn’t open {}: {}", &["a.txt", "no such file"]), "Couldn’t open a.txt: no such file");
        assert_eq!(fill("no blanks", &["unused"]), "no blanks");
    }

    #[test]
    fn english_is_left_alone_and_french_is_translated() {
        assert_eq!(translate(Language::English, "Save As…"), "Save As…");
        assert_eq!(translate(Language::French, "Save As…"), "Enregistrer sous…");
        assert_eq!(translate(Language::French, "not in the table"), "not in the table");
    }

    /// Catches a new or reworded piece of interface text that was not added to the table.
    #[test]
    fn everything_on_screen_has_a_french_translation() {
        let mut texts = translated_texts(include_str!("app.rs"));
        texts.extend(translated_texts(include_str!("document.rs")));
        texts.extend(translated_texts(include_str!("app/agent_tools.rs")));
        assert!(texts.len() > 50, "the texts in the source were found");
        // Texts that reach `tr` through a variable.
        texts.extend(TextTool::CASE.iter().chain(TextTool::LINES).chain(TextTool::WHITESPACE).map(|tool| tool.label()));
        texts.extend(emoji::CATEGORIES.iter().map(|(category, _)| *category));
        texts.extend(Theme::ALL.map(Theme::label));

        for text in texts {
            assert!(FRENCH.iter().any(|(english, _)| *english == text), "no French for {text:?}");
        }
    }

    #[test]
    fn translations_keep_the_blanks() {
        for (index, (english, french)) in FRENCH.iter().enumerate() {
            assert_eq!(english.matches("{}").count(), french.matches("{}").count(), "blanks in {english:?}");
            assert!(!FRENCH[..index].iter().any(|(earlier, _)| earlier == english), "{english:?} is listed twice");
        }
    }
}
