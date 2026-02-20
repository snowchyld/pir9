//! Settings API endpoints (v5)

use axum::{
    response::Json,
    routing::{get, put},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::web::AppState;

/// UI Settings Resource
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UiSettingsResource {
    pub id: i32,
    pub first_day_of_week: i32,
    pub calendar_week_column_header: String,
    pub short_date_format: String,
    pub long_date_format: String,
    pub time_format: String,
    pub show_relative_dates: bool,
    pub enable_color_impaired_mode: bool,
    pub movie_runtime_format: String,
    pub ui_language: i32,
    pub theme: String,
}

impl Default for UiSettingsResource {
    fn default() -> Self {
        Self {
            id: 1,
            first_day_of_week: 0,
            calendar_week_column_header: "ddd M/D".to_string(),
            short_date_format: "MMM D YYYY".to_string(),
            long_date_format: "dddd, MMMM D YYYY".to_string(),
            time_format: "h(:mm)a".to_string(),
            show_relative_dates: true,
            enable_color_impaired_mode: false,
            movie_runtime_format: "hoursMinutes".to_string(),
            ui_language: 1,
            theme: "auto".to_string(),
        }
    }
}

/// Naming Settings Resource
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NamingSettingsResource {
    pub id: i32,
    pub rename_episodes: bool,
    pub replace_illegal_characters: bool,
    pub colon_replacement_format: i32,
    pub custom_colon_replacement_format: Option<String>,
    pub multi_episode_style: i32,
    pub standard_episode_format: String,
    pub daily_episode_format: String,
    pub anime_episode_format: String,
    pub series_folder_format: String,
    pub season_folder_format: String,
    pub specials_folder_format: String,
}

impl Default for NamingSettingsResource {
    fn default() -> Self {
        Self {
            id: 1,
            rename_episodes: true,
            replace_illegal_characters: true,
            colon_replacement_format: 4,
            custom_colon_replacement_format: None,
            multi_episode_style: 0,
            standard_episode_format: "{Series Title} - S{season:00}E{episode:00} - {Episode Title} {Quality Full}".to_string(),
            daily_episode_format: "{Series Title} - {Air-Date} - {Episode Title} {Quality Full}".to_string(),
            anime_episode_format: "{Series Title} - S{season:00}E{episode:00} - {Episode Title} {Quality Full}".to_string(),
            series_folder_format: "{Series Title}".to_string(),
            season_folder_format: "Season {season}".to_string(),
            specials_folder_format: "Specials".to_string(),
        }
    }
}

/// Update Settings Resource
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSettingsResource {
    pub id: i32,
    pub mechanism: String,
    pub script_path: Option<String>,
}

impl Default for UpdateSettingsResource {
    fn default() -> Self {
        Self {
            id: 1,
            mechanism: "builtIn".to_string(),
            script_path: None,
        }
    }
}

/// Naming Example Resource
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NamingExampleResource {
    pub single_episode_example: String,
    pub multi_episode_example: String,
    pub daily_episode_example: String,
    pub anime_episode_example: String,
    pub anime_multi_episode_example: String,
    pub series_folder_example: String,
    pub season_folder_example: String,
    pub specials_folder_example: String,
}

impl Default for NamingExampleResource {
    fn default() -> Self {
        Self {
            single_episode_example: "The Series Title - S01E01 - Episode Title HDTV-720p".to_string(),
            multi_episode_example: "The Series Title - S01E01-E02-E03 - Episode Title HDTV-720p".to_string(),
            daily_episode_example: "The Series Title - 2024-01-15 - Episode Title HDTV-720p".to_string(),
            anime_episode_example: "The Series Title - S01E01 - Episode Title HDTV-720p".to_string(),
            anime_multi_episode_example: "The Series Title - S01E01-E02-E03 - Episode Title HDTV-720p".to_string(),
            series_folder_example: "The Series Title".to_string(),
            season_folder_example: "Season 1".to_string(),
            specials_folder_example: "Specials".to_string(),
        }
    }
}

// UI Settings handlers
pub async fn get_ui_settings() -> Json<UiSettingsResource> {
    Json(UiSettingsResource::default())
}

pub async fn update_ui_settings(Json(body): Json<UiSettingsResource>) -> Json<UiSettingsResource> {
    Json(body)
}

// Naming Settings handlers
pub async fn get_naming_settings() -> Json<NamingSettingsResource> {
    Json(NamingSettingsResource::default())
}

pub async fn update_naming_settings(Json(body): Json<NamingSettingsResource>) -> Json<NamingSettingsResource> {
    Json(body)
}

pub async fn get_naming_examples() -> Json<NamingExampleResource> {
    Json(NamingExampleResource::default())
}

// Update Settings handlers
pub async fn get_update_settings() -> Json<UpdateSettingsResource> {
    Json(UpdateSettingsResource::default())
}

pub async fn update_update_settings(Json(body): Json<UpdateSettingsResource>) -> Json<UpdateSettingsResource> {
    Json(body)
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/ui", get(get_ui_settings).put(update_ui_settings))
        .route("/naming", get(get_naming_settings).put(update_naming_settings))
        .route("/naming/examples", get(get_naming_examples))
        .route("/update", get(get_update_settings).put(update_update_settings))
}
