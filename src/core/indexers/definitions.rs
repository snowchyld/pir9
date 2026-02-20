#![allow(dead_code)]
//! Indexer definitions and implementations

use serde::{Deserialize, Serialize};

/// Indexer definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexerDefinition {
    pub id: i64,
    pub name: String,
    pub implementation: String,
    pub protocol: super::Protocol,
    pub fields: Vec<IndexerFieldDefinition>,
}

/// Indexer field definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexerFieldDefinition {
    pub name: String,
    pub label: String,
    #[serde(rename = "type")]
    pub field_type: FieldType,
    pub advanced: bool,
    pub help_text: Option<String>,
    pub default_value: Option<serde_json::Value>,
}

/// Field types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FieldType {
    Text,
    Number,
    Boolean,
    Select,
    Password,
    Url,
    Path,
}

/// Built-in indexer definitions
pub fn get_builtin_indexers() -> Vec<IndexerDefinition> {
    vec![
        IndexerDefinition {
            id: 1,
            name: "Newznab".to_string(),
            implementation: "Newznab".to_string(),
            protocol: super::Protocol::Usenet,
            fields: vec![
                IndexerFieldDefinition {
                    name: "baseUrl".to_string(),
                    label: "URL".to_string(),
                    field_type: FieldType::Url,
                    advanced: false,
                    help_text: Some("Indexer URL".to_string()),
                    default_value: None,
                },
                IndexerFieldDefinition {
                    name: "apiKey".to_string(),
                    label: "API Key".to_string(),
                    field_type: FieldType::Password,
                    advanced: false,
                    help_text: Some("Indexer API key".to_string()),
                    default_value: None,
                },
            ],
        },
        IndexerDefinition {
            id: 2,
            name: "Torznab".to_string(),
            implementation: "Torznab".to_string(),
            protocol: super::Protocol::Torrent,
            fields: vec![
                IndexerFieldDefinition {
                    name: "baseUrl".to_string(),
                    label: "URL".to_string(),
                    field_type: FieldType::Url,
                    advanced: false,
                    help_text: Some("Indexer URL".to_string()),
                    default_value: None,
                },
                IndexerFieldDefinition {
                    name: "apiKey".to_string(),
                    label: "API Key".to_string(),
                    field_type: FieldType::Password,
                    advanced: false,
                    help_text: Some("Indexer API key".to_string()),
                    default_value: None,
                },
            ],
        },
    ]
}
