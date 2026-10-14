//! User-facing strings in English and German.

use crate::calc::CalcError;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    De,
}

#[derive(Clone, Copy)]
pub enum Msg {
    MenuCalculator,
    CopyResult,
    Quit,
    MenuNotation,
    DecimalComma,
    DecimalPoint,
    MenuView,
    MenuTheme,
    MenuOpacity,
    Light,
    Dark,
    MenuLanguage,
    ClickToCopy,
    Copied,
    ShiftClickPower,
    Undo,
    Clear,
}

impl Lang {
    pub fn key(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::De => "de",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "en" => Some(Lang::En),
            "de" => Some(Lang::De),
            _ => None,
        }
    }

    /// The language's name in itself, as listed in the language menu.
    pub fn name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::De => "Deutsch",
        }
    }

    pub fn tr(self, msg: Msg) -> &'static str {
        let (en, de) = match msg {
            Msg::MenuCalculator => ("Calculator", "Rechner"),
            Msg::CopyResult => ("Copy result", "Ergebnis kopieren"),
            Msg::Quit => ("Quit", "Beenden"),
            Msg::MenuNotation => ("Notation", "Schreibweise"),
            Msg::DecimalComma => ("Decimal comma (1.234,5)", "Dezimalkomma (1.234,5)"),
            Msg::DecimalPoint => ("Decimal point (1,234.5)", "Dezimalpunkt (1,234.5)"),
            Msg::MenuView => ("View", "Ansicht"),
            Msg::MenuTheme => ("Theme", "Theme"),
            Msg::MenuOpacity => ("Opacity when unfocused", "Deckkraft ohne Fokus"),
            Msg::Light => ("Light", "Hell"),
            Msg::Dark => ("Dark", "Dunkel"),
            Msg::MenuLanguage => ("Language", "Sprache"),
            Msg::ClickToCopy => ("Click to copy", "Klicken zum Kopieren"),
            Msg::Copied => ("copied", "kopiert"),
            Msg::ShiftClickPower => ("Shift+click: xʸ", "Shift+Klick: xʸ"),
            Msg::Undo => ("Undo", "Zurück"),
            Msg::Clear => ("Clear", "Löschen"),
        };
        self.pick(en, de)
    }

    pub fn error(self, e: CalcError) -> &'static str {
        let (en, de) = match e {
            CalcError::Empty => ("", ""),
            CalcError::Syntax => ("Invalid expression", "Ungültiger Ausdruck"),
            CalcError::Grouping => ("Invalid digit grouping", "Ungültige Tausendertrennung"),
            CalcError::DivisionByZero => ("Division by zero", "Division durch null"),
            CalcError::NegativeRoot => ("Root of a negative number", "Wurzel aus negativer Zahl"),
            CalcError::Overflow => ("Number too large", "Zahl zu groß"),
        };
        self.pick(en, de)
    }

    pub fn percent(self, n: u8) -> String {
        match self {
            Lang::En => format!("{n}%"),
            Lang::De => format!("{n} %"),
        }
    }

    fn pick(self, en: &'static str, de: &'static str) -> &'static str {
        match self {
            Lang::En => en,
            Lang::De => de,
        }
    }
}
