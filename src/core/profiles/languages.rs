#![allow(dead_code)]
//! Language profile definitions

use serde::{Deserialize, Serialize};
use once_cell::sync::Lazy;

/// Language definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Language {
    pub id: i32,
    pub name: String,
}

impl Language {
    /// Create a new language (returns LanguageStatic for const construction)
    #[allow(clippy::new_ret_no_self)]
    pub const fn new(id: i32, name: &'static str) -> LanguageStatic {
        LanguageStatic { id, name }
    }

    /// Get language by ID
    pub fn from_id(id: i32) -> Option<Self> {
        LANGUAGES.iter()
            .find(|l| l.id == id)
            .cloned()
    }

    /// English language
    pub fn english() -> Self {
        Self { id: 1, name: "English".to_string() }
    }
}

/// Static language definition (for const contexts)
#[derive(Debug, Clone, Copy)]
pub struct LanguageStatic {
    pub id: i32,
    pub name: &'static str,
}

impl From<LanguageStatic> for Language {
    fn from(s: LanguageStatic) -> Self {
        Language { id: s.id, name: s.name.to_string() }
    }
}

/// All supported languages
pub static LANGUAGES: Lazy<Vec<Language>> = Lazy::new(|| {
    vec![
        Language { id: 0, name: "Unknown".to_string() },
        Language { id: 1, name: "English".to_string() },
        Language { id: 2, name: "French".to_string() },
        Language { id: 3, name: "Spanish".to_string() },
        Language { id: 4, name: "German".to_string() },
        Language { id: 5, name: "Italian".to_string() },
        Language { id: 6, name: "Danish".to_string() },
        Language { id: 7, name: "Dutch".to_string() },
        Language { id: 8, name: "Japanese".to_string() },
        Language { id: 9, name: "Cantonese".to_string() },
        Language { id: 10, name: "Mandarin".to_string() },
        Language { id: 11, name: "Russian".to_string() },
        Language { id: 12, name: "Polish".to_string() },
        Language { id: 13, name: "Vietnamese".to_string() },
        Language { id: 14, name: "Swedish".to_string() },
        Language { id: 15, name: "Norwegian".to_string() },
        Language { id: 16, name: "Finnish".to_string() },
        Language { id: 17, name: "Turkish".to_string() },
        Language { id: 18, name: "Portuguese".to_string() },
        Language { id: 19, name: "Flemish".to_string() },
        Language { id: 20, name: "Greek".to_string() },
        Language { id: 21, name: "Korean".to_string() },
        Language { id: 22, name: "Hungarian".to_string() },
        Language { id: 23, name: "Hebrew".to_string() },
        Language { id: 24, name: "Lithuanian".to_string() },
        Language { id: 25, name: "Czech".to_string() },
        Language { id: 26, name: "Arabic".to_string() },
        Language { id: 27, name: "Hindi".to_string() },
        Language { id: 28, name: "Romanian".to_string() },
        Language { id: 29, name: "Thai".to_string() },
        Language { id: 30, name: "Brazilian".to_string() },
        Language { id: 31, name: "Latvian".to_string() },
        Language { id: 32, name: "Ukrainian".to_string() },
        Language { id: 33, name: "Persian".to_string() },
        Language { id: 34, name: "Indonesian".to_string() },
        Language { id: 35, name: "Catalan".to_string() },
        Language { id: 36, name: "Georgian".to_string() },
    ]
});
