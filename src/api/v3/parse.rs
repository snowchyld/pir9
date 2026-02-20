//! Parse API endpoints

use axum::{extract::Query, response::Json, routing::get, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::web::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct ParseQuery {
    pub title: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseResource {
    pub title: Option<String>,
    pub parsed_episode_info: Option<ParsedEpisodeInfo>,
    pub series: Option<serde_json::Value>,
    pub episodes: Vec<serde_json::Value>,
    pub languages: Vec<LanguageResource>,
    pub quality: Option<QualityModel>,
    pub custom_formats: Vec<serde_json::Value>,
    pub custom_format_score: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedEpisodeInfo {
    pub release_title: String,
    pub series_title: String,
    pub series_title_info: SeriesTitleInfo,
    pub quality: QualityModel,
    pub season_number: i32,
    pub episode_numbers: Vec<i32>,
    pub absolute_episode_numbers: Vec<i32>,
    pub special_absolute_episode_numbers: Vec<i32>,
    pub languages: Vec<LanguageResource>,
    pub full_season: bool,
    pub is_partial_season: bool,
    pub is_multi_season: bool,
    pub is_season_extra: bool,
    pub is_split_episode: bool,
    pub is_mini_series: bool,
    pub special: bool,
    pub release_group: Option<String>,
    pub release_hash: Option<String>,
    pub season_part: i32,
    pub release_tokens: String,
    pub daily_part: Option<i32>,
    pub is_daily: bool,
    pub is_absolute_numbering: bool,
    pub is_possible_special_episode: bool,
    pub is_possible_scene_season_special: bool,
    pub release_type: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesTitleInfo {
    pub title: String,
    pub title_without_year: String,
    pub year: i32,
    pub all_titles: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageResource {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityModel {
    pub quality: QualityResource,
    pub revision: RevisionResource,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityResource {
    pub id: i32,
    pub name: String,
    pub source: String,
    pub resolution: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionResource {
    pub version: i32,
    pub real: i32,
    pub is_repack: bool,
}

/// GET /api/v3/parse
pub async fn parse(_query: Query<ParseQuery>) -> Json<ParseResource> {
    Json(ParseResource {
        title: None,
        parsed_episode_info: None,
        series: None,
        episodes: vec![],
        languages: vec![],
        quality: None,
        custom_formats: vec![],
        custom_format_score: 0,
    })
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/", get(parse))
}
