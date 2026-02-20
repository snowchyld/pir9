#![allow(dead_code)]
//! Delay profile definitions

use serde::{Deserialize, Serialize};

/// Delay profile for managing download delays
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelayProfile {
    pub id: i64,
    pub enable_usenet: bool,
    pub enable_torrent: bool,
    pub preferred_protocol: Protocol,
    pub usenet_delay: i32,
    pub torrent_delay: i32,
    pub bypass_if_highest_quality: bool,
    pub bypass_if_above_custom_format_score: i32,
    pub tags: Vec<i64>,
}

/// Download protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Protocol {
    #[default]
    Unknown,
    Usenet,
    Torrent,
}

impl DelayProfile {
    /// Create a default delay profile
    pub fn default_profile() -> Self {
        Self {
            id: 1,
            enable_usenet: true,
            enable_torrent: true,
            preferred_protocol: Protocol::Usenet,
            usenet_delay: 0,
            torrent_delay: 0,
            bypass_if_highest_quality: false,
            bypass_if_above_custom_format_score: 0,
            tags: vec![],
        }
    }
    
    /// Get the delay for a specific protocol
    pub fn get_delay(&self, protocol: Protocol) -> i32 {
        match protocol {
            Protocol::Usenet => self.usenet_delay,
            Protocol::Torrent => self.torrent_delay,
            _ => 0,
        }
    }
}
