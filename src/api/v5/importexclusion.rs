//! Import Exclusion API endpoints (v5)
//! Manage the list of movies/series excluded from automatic import

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::models::ImportExclusionDbModel;
use crate::core::datastore::repositories::ImportExclusionRepository;
use crate::web::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportExclusionResource {
    pub id: i64,
    pub tmdb_id: Option<i32>,
    pub imdb_id: Option<String>,
    pub tvdb_id: Option<i32>,
    pub title: String,
    pub year: Option<i32>,
    pub content_type: String,
    pub added: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateImportExclusionRequest {
    pub tmdb_id: Option<i32>,
    pub imdb_id: Option<String>,
    pub tvdb_id: Option<i32>,
    pub title: String,
    pub year: Option<i32>,
    pub content_type: Option<String>,
}

fn db_to_resource(model: &ImportExclusionDbModel) -> ImportExclusionResource {
    ImportExclusionResource {
        id: model.id,
        tmdb_id: model.tmdb_id.map(|v| v as i32),
        imdb_id: model.imdb_id.clone(),
        tvdb_id: model.tvdb_id.map(|v| v as i32),
        title: model.title.clone(),
        year: model.year,
        content_type: model.content_type.clone(),
        added: model.added.to_rfc3339(),
    }
}

/// GET /api/v5/importexclusion — list all exclusions
async fn list_exclusions(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ImportExclusionResource>>, StatusCode> {
    let repo = ImportExclusionRepository::new(state.db.clone());
    let exclusions = repo
        .get_all()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(exclusions.iter().map(db_to_resource).collect()))
}

/// POST /api/v5/importexclusion — add an exclusion
async fn add_exclusion(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateImportExclusionRequest>,
) -> Result<Json<ImportExclusionResource>, StatusCode> {
    if body.title.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let content_type = body.content_type.as_deref().unwrap_or("movie");
    if content_type != "movie" && content_type != "series" {
        return Err(StatusCode::BAD_REQUEST);
    }

    let repo = ImportExclusionRepository::new(state.db.clone());
    let new_id = repo
        .add(
            body.tmdb_id,
            body.imdb_id.as_deref(),
            body.tvdb_id,
            &body.title,
            body.year,
            content_type,
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Fetch the inserted row to return it
    let inserted = repo
        .get_by_id(new_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(db_to_resource(&inserted)))
}

/// DELETE /api/v5/importexclusion/{id} — remove an exclusion
async fn delete_exclusion(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> StatusCode {
    let repo = ImportExclusionRepository::new(state.db.clone());
    match repo.delete(id).await {
        Ok(()) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_exclusions).post(add_exclusion))
        .route("/{id}", delete(delete_exclusion))
}
