//! Internationalization (i18n)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Supported languages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    English,
    Chinese,
    Japanese,
    Korean,
    German,
    French,
    Spanish,
    Italian,
    Portuguese,
    Dutch,
    Polish,
    Czech,
    Arabic,
    Farsi,
    Turkish,
    Russian,
}

/// Localization manager
pub struct Localization {
    current: Language,
    translations: HashMap<Language, HashMap<String, String>>,
}

impl Localization {
    /// Create new localization manager
    #[must_use]
    pub fn new(lang: Language) -> Self {
        Self {
            current: lang,
            translations: HashMap::new(),
        }
    }

    /// Get translation for key
    #[must_use]
    pub fn t(&self, key: &str) -> String {
        self.translations
            .get(&self.current)
            .and_then(|t| t.get(key).cloned())
            .unwrap_or_else(|| key.to_string())
    }

    /// Set current language
    pub fn set_language(&mut self, lang: Language) {
        self.current = lang;
    }
}
