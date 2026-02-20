//! Import List API endpoints

use axum::{
    extract::Path,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::web::AppState;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ImportListResource {
    #[serde(default)]
    pub id: i32,
    pub name: String,
    #[serde(default)]
    pub fields: Vec<FieldResource>,
    #[serde(default)]
    pub implementation_name: String,
    #[serde(default)]
    pub implementation: String,
    #[serde(default)]
    pub config_contract: String,
    pub info_link: Option<String>,
    pub message: Option<ProviderMessage>,
    #[serde(default)]
    pub tags: Vec<i32>,
    #[serde(default)]
    pub presets: Vec<serde_json::Value>,
    #[serde(default)]
    pub enable_automatic_add: bool,
    #[serde(default)]
    pub search_for_missing_episodes: bool,
    #[serde(default = "default_should_monitor")]
    pub should_monitor: String,
    pub root_folder_path: Option<String>,
    #[serde(default = "default_quality_profile_id")]
    pub quality_profile_id: i32,
    #[serde(default = "default_series_type")]
    pub series_type: String,
    #[serde(default = "default_true")]
    pub season_folder: bool,
    #[serde(default)]
    pub list_type: String,
    #[serde(default = "default_list_order")]
    pub list_order: i32,
    #[serde(default = "default_refresh_interval")]
    pub min_refresh_interval: String,
}

fn default_should_monitor() -> String {
    "all".to_string()
}

fn default_quality_profile_id() -> i32 {
    1
}

fn default_series_type() -> String {
    "standard".to_string()
}

fn default_true() -> bool {
    true
}

fn default_list_order() -> i32 {
    1
}

fn default_refresh_interval() -> String {
    "PT12H".to_string()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FieldResource {
    #[serde(default)]
    pub order: i32,
    pub name: String,
    #[serde(default)]
    pub label: String,
    pub unit: Option<String>,
    pub help_text: Option<String>,
    pub help_text_warning: Option<String>,
    pub help_link: Option<String>,
    pub value: Option<serde_json::Value>,
    #[serde(rename = "type", default = "default_field_type")]
    pub field_type: String,
    #[serde(default)]
    pub advanced: bool,
    pub select_options: Option<Vec<serde_json::Value>>,
    pub select_options_provider_action: Option<String>,
    pub section: Option<String>,
    pub hidden: Option<String>,
    pub privacy: Option<String>,
    pub placeholder: Option<String>,
    pub is_float: Option<bool>,
}

fn default_field_type() -> String {
    "textbox".to_string()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProviderMessage {
    pub message: String,
    #[serde(rename = "type")]
    pub message_type: String,
}

/// GET /api/v3/importlist
pub async fn get_import_lists() -> Json<Vec<ImportListResource>> {
    Json(vec![])
}

/// GET /api/v3/importlist/:id
pub async fn get_import_list(Path(id): Path<i32>) -> Json<Option<ImportListResource>> {
    let _ = id;
    Json(None)
}

/// POST /api/v3/importlist
pub async fn create_import_list(Json(body): Json<ImportListResource>) -> Json<ImportListResource> {
    Json(body)
}

/// PUT /api/v3/importlist/:id
pub async fn update_import_list(
    Path(id): Path<i32>,
    Json(mut body): Json<ImportListResource>,
) -> Json<ImportListResource> {
    body.id = id;
    Json(body)
}

/// DELETE /api/v3/importlist/:id
pub async fn delete_import_list(Path(id): Path<i32>) -> Json<serde_json::Value> {
    let _ = id;
    Json(serde_json::json!({}))
}

/// POST /api/v3/importlist/test
pub async fn test_import_list(Json(_body): Json<ImportListResource>) -> Json<serde_json::Value> {
    Json(serde_json::json!({}))
}

/// POST /api/v3/importlist/testall
pub async fn test_all_import_lists() -> Json<Vec<serde_json::Value>> {
    Json(vec![])
}

/// GET /api/v3/importlist/schema
pub async fn get_import_list_schema() -> Json<Vec<ImportListResource>> {
    let schemas = vec![
        create_trakt_list_schema(),
        create_trakt_user_schema(),
        create_trakt_popular_schema(),
        create_imdb_list_schema(),
        create_plex_watchlist_schema(),
        create_sonarr_schema(),
        create_simkl_user_schema(),
    ];
    Json(schemas)
}

/// Helper to create a field
fn make_field(
    order: i32,
    name: &str,
    label: &str,
    field_type: &str,
    value: Option<serde_json::Value>,
    help_text: Option<&str>,
) -> FieldResource {
    FieldResource {
        order,
        name: name.to_string(),
        label: label.to_string(),
        unit: None,
        help_text: help_text.map(|s| s.to_string()),
        help_text_warning: None,
        help_link: None,
        value,
        field_type: field_type.to_string(),
        advanced: false,
        select_options: None,
        select_options_provider_action: None,
        section: None,
        hidden: None,
        privacy: if field_type == "password" {
            Some("password".to_string())
        } else {
            None
        },
        placeholder: None,
        is_float: None,
    }
}

/// Create default import list resource
fn default_import_list(
    implementation: &str,
    implementation_name: &str,
    config_contract: &str,
    list_type: &str,
) -> ImportListResource {
    ImportListResource {
        id: 0,
        name: String::new(),
        fields: vec![],
        implementation_name: implementation_name.to_string(),
        implementation: implementation.to_string(),
        config_contract: config_contract.to_string(),
        info_link: None,
        message: None,
        tags: vec![],
        presets: vec![],
        enable_automatic_add: true,
        search_for_missing_episodes: true,
        should_monitor: "all".to_string(),
        root_folder_path: None,
        quality_profile_id: 1,
        series_type: "standard".to_string(),
        season_folder: true,
        list_type: list_type.to_string(),
        list_order: 1,
        min_refresh_interval: "PT12H".to_string(),
    }
}

/// Trakt List schema
fn create_trakt_list_schema() -> ImportListResource {
    let mut schema = default_import_list(
        "TraktListImport",
        "Trakt List",
        "TraktListSettings",
        "trakt",
    );
    schema.fields = vec![
        make_field(
            0,
            "authUser",
            "Auth User",
            "textbox",
            None,
            Some("Trakt username for authentication"),
        ),
        make_field(
            1,
            "accessToken",
            "Access Token",
            "password",
            None,
            Some("Trakt API access token"),
        ),
        make_field(
            2,
            "refreshToken",
            "Refresh Token",
            "password",
            None,
            Some("Trakt API refresh token"),
        ),
        make_field(
            3,
            "expires",
            "Expires",
            "textbox",
            None,
            Some("Token expiration time"),
        ),
        make_field(
            4,
            "username",
            "Username",
            "textbox",
            None,
            Some("Trakt username for list"),
        ),
        make_field(
            5,
            "listname",
            "List Name",
            "textbox",
            None,
            Some("Name of the Trakt list"),
        ),
        make_field(
            6,
            "traktAdditionalParameters",
            "Additional Parameters",
            "textbox",
            None,
            Some("Additional parameters to add to Trakt API request"),
        ),
    ];
    schema
}

/// Trakt User schema
fn create_trakt_user_schema() -> ImportListResource {
    let mut schema = default_import_list(
        "TraktUserImport",
        "Trakt User",
        "TraktUserSettings",
        "trakt",
    );
    schema.fields = vec![
        make_field(
            0,
            "authUser",
            "Auth User",
            "textbox",
            None,
            Some("Trakt username for authentication"),
        ),
        make_field(
            1,
            "accessToken",
            "Access Token",
            "password",
            None,
            Some("Trakt API access token"),
        ),
        make_field(
            2,
            "refreshToken",
            "Refresh Token",
            "password",
            None,
            Some("Trakt API refresh token"),
        ),
        make_field(
            3,
            "expires",
            "Expires",
            "textbox",
            None,
            Some("Token expiration time"),
        ),
        make_field(
            4,
            "traktListType",
            "List Type",
            "select",
            Some(serde_json::json!(0)),
            Some("Type of user list"),
        ),
        make_field(
            5,
            "traktAdditionalParameters",
            "Additional Parameters",
            "textbox",
            None,
            Some("Additional parameters to add to Trakt API request"),
        ),
    ];
    schema
}

/// Trakt Popular schema
fn create_trakt_popular_schema() -> ImportListResource {
    let mut schema = default_import_list(
        "TraktPopularImport",
        "Trakt Popular",
        "TraktPopularSettings",
        "trakt",
    );
    schema.fields = vec![
        make_field(
            0,
            "authUser",
            "Auth User",
            "textbox",
            None,
            Some("Trakt username for authentication"),
        ),
        make_field(
            1,
            "accessToken",
            "Access Token",
            "password",
            None,
            Some("Trakt API access token"),
        ),
        make_field(
            2,
            "refreshToken",
            "Refresh Token",
            "password",
            None,
            Some("Trakt API refresh token"),
        ),
        make_field(
            3,
            "expires",
            "Expires",
            "textbox",
            None,
            Some("Token expiration time"),
        ),
        make_field(
            4,
            "traktListType",
            "List Type",
            "select",
            Some(serde_json::json!(0)),
            Some("Popular/Trending/Anticipated/etc"),
        ),
        make_field(
            5,
            "limit",
            "Limit",
            "number",
            Some(serde_json::json!(100)),
            Some("Number of series to get"),
        ),
        make_field(
            6,
            "traktAdditionalParameters",
            "Additional Parameters",
            "textbox",
            None,
            Some("Additional parameters"),
        ),
    ];
    schema
}

/// IMDb List schema
fn create_imdb_list_schema() -> ImportListResource {
    let mut schema =
        default_import_list("IMDbListImport", "IMDb Lists", "IMDbListSettings", "imdb");
    schema.fields = vec![make_field(
        0,
        "listId",
        "List ID",
        "textbox",
        None,
        Some("IMDb List ID (e.g., ls012345678 or ur12345678)"),
    )];
    schema
}

/// Plex Watchlist schema
fn create_plex_watchlist_schema() -> ImportListResource {
    let mut schema =
        default_import_list("PlexImport", "Plex Watchlist", "PlexListSettings", "plex");
    schema.fields = vec![make_field(
        0,
        "accessToken",
        "Access Token",
        "password",
        None,
        Some("Plex authentication token"),
    )];
    schema
}

/// Sonarr (another instance) schema
fn create_sonarr_schema() -> ImportListResource {
    let mut schema = default_import_list("SonarrImport", "Sonarr", "SonarrSettings", "sonarr");
    schema.fields = vec![
        make_field(
            0,
            "baseUrl",
            "Sonarr Server",
            "textbox",
            None,
            Some("URL of the Sonarr server to import from"),
        ),
        make_field(
            1,
            "apiKey",
            "API Key",
            "password",
            None,
            Some("API key for the Sonarr server"),
        ),
        make_field(
            2,
            "profileIds",
            "Quality Profiles",
            "textbox",
            None,
            Some("Quality profile IDs to import from (comma separated)"),
        ),
        make_field(
            3,
            "tagIds",
            "Tags",
            "textbox",
            None,
            Some("Tag IDs to import from (comma separated)"),
        ),
        make_field(
            4,
            "rootFolderPaths",
            "Root Folders",
            "textbox",
            None,
            Some("Root folder paths to import from (comma separated)"),
        ),
        make_field(
            5,
            "languageProfileIds",
            "Language Profiles",
            "textbox",
            None,
            Some("Language profile IDs (comma separated)"),
        ),
    ];
    schema
}

/// Simkl User schema
fn create_simkl_user_schema() -> ImportListResource {
    let mut schema = default_import_list(
        "SimklUserImport",
        "Simkl User Watchlist",
        "SimklUserSettings",
        "simkl",
    );
    schema.fields = vec![
        make_field(
            0,
            "authUser",
            "Auth User",
            "textbox",
            None,
            Some("Simkl username"),
        ),
        make_field(
            1,
            "accessToken",
            "Access Token",
            "password",
            None,
            Some("Simkl API access token"),
        ),
        make_field(
            2,
            "listType",
            "List Type",
            "select",
            Some(serde_json::json!(0)),
            Some("Type of list to import"),
        ),
    ];
    schema
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_import_lists).post(create_import_list))
        .route(
            "/{id}",
            get(get_import_list)
                .put(update_import_list)
                .delete(delete_import_list),
        )
        .route("/test", post(test_import_list))
        .route("/testall", post(test_all_import_lists))
        .route("/schema", get(get_import_list_schema))
}
