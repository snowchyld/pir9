//! Profiles module
//! Quality profiles, language profiles, and delay profiles

pub mod qualities;
pub mod languages;
pub mod delay;

use serde::{Deserialize, Serialize};

/// Quality profile definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityProfile {
    pub id: i64,
    pub name: String,
    pub upgrade_allowed: bool,
    pub cutoff: QualityWrapper,
    pub items: Vec<QualityProfileItem>,
    pub min_format_score: i32,
    pub cutoff_format_score: i32,
    pub format_items: Vec<FormatItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityWrapper {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityProfileItem {
    pub quality: QualityWrapper,
    pub items: Vec<QualityWrapper>,
    pub allowed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatItem {
    pub format: i64,
    pub score: i32,
}

/// Language profile definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageProfile {
    pub id: i64,
    pub name: String,
    pub upgrade_allowed: bool,
    pub cutoff: LanguageWrapper,
    pub languages: Vec<LanguageItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageWrapper {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageItem {
    pub language: LanguageWrapper,
    pub allowed: bool,
}

/// Delay profile definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelayProfile {
    pub id: i64,
    pub enable_usenet: bool,
    pub enable_torrent: bool,
    pub preferred_protocol: String,
    pub usenet_delay: i32,
    pub torrent_delay: i32,
    pub bypass_if_highest_quality: bool,
    pub bypass_if_above_custom_format_score: i32,
    pub tags: Vec<i64>,
}
