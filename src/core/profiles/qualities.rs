#![allow(dead_code)]
//! Quality definitions and utilities

use serde::{Deserialize, Serialize};

/// Quality definition with size limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityDefinition {
    pub quality: Quality,
    pub title: String,
    pub weight: i32,
    pub min_size: Option<f64>,
    pub max_size: Option<f64>,
    pub preferred_size: Option<f64>,
}

/// Quality types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::upper_case_acronyms)]
pub enum Quality {
    #[default]
    Unknown,
    SDTV,
    #[serde(rename = "DVD")]
    Dvd,
    #[serde(rename = "WEBDL-480p")]
    WebDl480p,
    #[serde(rename = "HDTV-720p")]
    Hdtv720p,
    #[serde(rename = "WEBDL-720p")]
    WebDl720p,
    #[serde(rename = "Bluray-720p")]
    Bluray720p,
    #[serde(rename = "HDTV-1080p")]
    Hdtv1080p,
    #[serde(rename = "WEBDL-1080p")]
    WebDl1080p,
    #[serde(rename = "Bluray-1080p")]
    Bluray1080p,
    #[serde(rename = "Bluray-1080p Remux")]
    Bluray1080pRemux,
    #[serde(rename = "WEBDL-2160p")]
    WebDl2160p,
    #[serde(rename = "HDTV-2160p")]
    Hdtv2160p,
    #[serde(rename = "Bluray-2160p")]
    Bluray2160p,
    #[serde(rename = "Bluray-2160p Remux")]
    Bluray2160pRemux,
}

impl Quality {
    /// Get a human-readable display name matching the serde rename values
    pub fn display_name(&self) -> &'static str {
        match self {
            Quality::Unknown => "Unknown",
            Quality::SDTV => "SDTV",
            Quality::Dvd => "DVD",
            Quality::WebDl480p => "WEBDL-480p",
            Quality::Hdtv720p => "HDTV-720p",
            Quality::WebDl720p => "WEBDL-720p",
            Quality::Bluray720p => "Bluray-720p",
            Quality::Hdtv1080p => "HDTV-1080p",
            Quality::WebDl1080p => "WEBDL-1080p",
            Quality::Bluray1080p => "Bluray-1080p",
            Quality::Bluray1080pRemux => "Bluray-1080p Remux",
            Quality::WebDl2160p => "WEBDL-2160p",
            Quality::Hdtv2160p => "HDTV-2160p",
            Quality::Bluray2160p => "Bluray-2160p",
            Quality::Bluray2160pRemux => "Bluray-2160p Remux",
        }
    }

    /// Get the resolution width for this quality
    pub fn resolution_width(&self) -> i32 {
        match self {
            Quality::Unknown => 0,
            Quality::SDTV | Quality::Dvd | Quality::WebDl480p => 480,
            Quality::Hdtv720p | Quality::WebDl720p | Quality::Bluray720p => 720,
            Quality::Hdtv1080p
            | Quality::WebDl1080p
            | Quality::Bluray1080p
            | Quality::Bluray1080pRemux => 1080,
            Quality::WebDl2160p
            | Quality::Hdtv2160p
            | Quality::Bluray2160p
            | Quality::Bluray2160pRemux => 2160,
        }
    }

    /// Check if this is a high definition quality
    pub fn is_hd(&self) -> bool {
        matches!(self.resolution_width(), 720 | 1080 | 2160)
    }

    /// Check if this is 4K/UHD
    pub fn is_uhd(&self) -> bool {
        self.resolution_width() >= 2160
    }

    /// Get the weight (priority) of this quality
    pub fn weight(&self) -> i32 {
        match self {
            Quality::Unknown => 0,
            Quality::SDTV => 1,
            Quality::Dvd => 2,
            Quality::WebDl480p => 3,
            Quality::Hdtv720p => 4,
            Quality::WebDl720p => 5,
            Quality::Bluray720p => 6,
            Quality::Hdtv1080p => 7,
            Quality::WebDl1080p => 8,
            Quality::Bluray1080p => 9,
            Quality::Bluray1080pRemux => 10,
            Quality::Hdtv2160p => 11,
            Quality::WebDl2160p => 12,
            Quality::Bluray2160p => 13,
            Quality::Bluray2160pRemux => 14,
        }
    }

    /// Compare two qualities
    pub fn compare(&self, other: &Quality) -> std::cmp::Ordering {
        self.weight().cmp(&other.weight())
    }
}

/// Quality model with revision info
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QualityModel {
    pub quality: Quality,
    #[serde(default)]
    pub revision: Revision,
}

/// Revision info for proper/repack
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Revision {
    pub version: i32,
    pub real: i32,
    pub is_repack: bool,
}

impl Default for Revision {
    fn default() -> Self {
        Self {
            version: 1,
            real: 0,
            is_repack: false,
        }
    }
}

/// Source type for quality
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QualitySource {
    Unknown,
    Television,
    Web,
    WebRip,
    BluRay,
    Dvd,
}

/// Resolution type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Resolution {
    R360p,
    R480p,
    R576p,
    R720p,
    R1080p,
    R2160p,
}

impl Resolution {
    pub fn width(&self) -> i32 {
        match self {
            Resolution::R360p => 480,
            Resolution::R480p => 640,
            Resolution::R576p => 768,
            Resolution::R720p => 1280,
            Resolution::R1080p => 1920,
            Resolution::R2160p => 3840,
        }
    }

    pub fn height(&self) -> i32 {
        match self {
            Resolution::R360p => 360,
            Resolution::R480p => 480,
            Resolution::R576p => 576,
            Resolution::R720p => 720,
            Resolution::R1080p => 1080,
            Resolution::R2160p => 2160,
        }
    }
}

/// Get default quality definitions
pub fn default_quality_definitions() -> Vec<QualityDefinition> {
    vec![
        QualityDefinition {
            quality: Quality::SDTV,
            title: "SDTV".to_string(),
            weight: 1,
            min_size: Some(0.0),
            max_size: Some(100.0),
            preferred_size: None,
        },
        QualityDefinition {
            quality: Quality::Dvd,
            title: "DVD".to_string(),
            weight: 2,
            min_size: Some(0.0),
            max_size: Some(100.0),
            preferred_size: None,
        },
        QualityDefinition {
            quality: Quality::WebDl480p,
            title: "WEBDL-480p".to_string(),
            weight: 3,
            min_size: Some(0.0),
            max_size: Some(100.0),
            preferred_size: None,
        },
        QualityDefinition {
            quality: Quality::Hdtv720p,
            title: "HDTV-720p".to_string(),
            weight: 4,
            min_size: Some(0.0),
            max_size: Some(100.0),
            preferred_size: None,
        },
        QualityDefinition {
            quality: Quality::WebDl720p,
            title: "WEBDL-720p".to_string(),
            weight: 5,
            min_size: Some(0.0),
            max_size: Some(100.0),
            preferred_size: None,
        },
        QualityDefinition {
            quality: Quality::Bluray720p,
            title: "Bluray-720p".to_string(),
            weight: 6,
            min_size: Some(0.0),
            max_size: Some(100.0),
            preferred_size: None,
        },
        QualityDefinition {
            quality: Quality::Hdtv1080p,
            title: "HDTV-1080p".to_string(),
            weight: 7,
            min_size: Some(0.0),
            max_size: Some(100.0),
            preferred_size: None,
        },
        QualityDefinition {
            quality: Quality::WebDl1080p,
            title: "WEBDL-1080p".to_string(),
            weight: 8,
            min_size: Some(0.0),
            max_size: Some(100.0),
            preferred_size: None,
        },
        QualityDefinition {
            quality: Quality::Bluray1080p,
            title: "Bluray-1080p".to_string(),
            weight: 9,
            min_size: Some(0.0),
            max_size: Some(100.0),
            preferred_size: None,
        },
        QualityDefinition {
            quality: Quality::Bluray1080pRemux,
            title: "Bluray-1080p Remux".to_string(),
            weight: 10,
            min_size: Some(0.0),
            max_size: Some(400.0),
            preferred_size: None,
        },
        QualityDefinition {
            quality: Quality::Hdtv2160p,
            title: "HDTV-2160p".to_string(),
            weight: 11,
            min_size: Some(0.0),
            max_size: Some(100.0),
            preferred_size: None,
        },
        QualityDefinition {
            quality: Quality::WebDl2160p,
            title: "WEBDL-2160p".to_string(),
            weight: 12,
            min_size: Some(0.0),
            max_size: Some(100.0),
            preferred_size: None,
        },
        QualityDefinition {
            quality: Quality::Bluray2160p,
            title: "Bluray-2160p".to_string(),
            weight: 13,
            min_size: Some(0.0),
            max_size: Some(100.0),
            preferred_size: None,
        },
        QualityDefinition {
            quality: Quality::Bluray2160pRemux,
            title: "Bluray-2160p Remux".to_string(),
            weight: 14,
            min_size: Some(0.0),
            max_size: Some(400.0),
            preferred_size: None,
        },
    ]
}
