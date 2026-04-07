//! Import list sync engine
//!
//! Fetches items from external lists (IMDB watchlists, IMDB lists, Trakt)
//! and auto-adds missing movies/series to the library.

use anyhow::{Context, Result};
use chrono::Utc;
use tracing::{error, info, warn};

use crate::core::datastore::models::{ImportListDbModel, MovieDbModel, SeriesDbModel};
use crate::core::datastore::repositories::{
    ImportListExclusionRepository, ImportListRepository, MovieRepository, SeriesRepository,
};
use crate::core::datastore::Database;
use crate::core::imdb::ImdbClient;

/// Result of syncing a single import list
#[derive(Debug, Default)]
pub struct ImportListSyncResult {
    pub list_id: i64,
    pub list_name: String,
    pub items_found: usize,
    pub items_added: usize,
    pub items_skipped: usize,
    pub items_excluded: usize,
    pub errors: Vec<String>,
}

/// Sync all enabled import lists that are due for sync
pub async fn sync_all_import_lists(db: &Database, imdb_client: &ImdbClient) -> Result<Vec<ImportListSyncResult>> {
    let repo = ImportListRepository::new(db.clone());
    let lists = repo.get_enabled().await?;

    if lists.is_empty() {
        info!("No enabled import lists to sync");
        return Ok(vec![]);
    }

    info!("Syncing {} enabled import lists", lists.len());
    let mut results = Vec::new();

    for list in &lists {
        // Check if this list is due for sync based on its interval
        if let Some(last_synced) = list.last_synced_at {
            let hours_since_sync = (Utc::now() - last_synced).num_hours();
            if hours_since_sync < list.sync_interval_hours as i64 {
                info!(
                    "Import list '{}' was synced {} hours ago (interval: {}h), skipping",
                    list.name, hours_since_sync, list.sync_interval_hours
                );
                continue;
            }
        }

        match sync_import_list(list, db, imdb_client).await {
            Ok(result) => {
                info!(
                    "Import list '{}': found={}, added={}, skipped={}, excluded={}, errors={}",
                    result.list_name,
                    result.items_found,
                    result.items_added,
                    result.items_skipped,
                    result.items_excluded,
                    result.errors.len()
                );
                results.push(result);
            }
            Err(e) => {
                error!("Failed to sync import list '{}': {}", list.name, e);
                results.push(ImportListSyncResult {
                    list_id: list.id,
                    list_name: list.name.clone(),
                    errors: vec![e.to_string()],
                    ..Default::default()
                });
            }
        }

        // Update last_synced_at regardless of success/failure
        if let Err(e) = repo.update_last_synced(list.id).await {
            warn!("Failed to update last_synced_at for list '{}': {}", list.name, e);
        }
    }

    Ok(results)
}

/// Sync a single import list
pub async fn sync_import_list(
    list: &ImportListDbModel,
    db: &Database,
    imdb_client: &ImdbClient,
) -> Result<ImportListSyncResult> {
    let mut result = ImportListSyncResult {
        list_id: list.id,
        list_name: list.name.clone(),
        ..Default::default()
    };

    // Fetch items based on list type
    let items = match list.list_type.as_str() {
        "imdb_watchlist" | "imdb_list" => {
            fetch_imdb_list_items(list, imdb_client).await?
        }
        "trakt_watchlist" | "trakt_list" => {
            // Trakt is stubbed for now -- return empty list
            info!("Trakt import lists not yet implemented, skipping '{}'", list.name);
            vec![]
        }
        unknown => {
            warn!("Unknown import list type '{}', skipping", unknown);
            return Ok(result);
        }
    };

    result.items_found = items.len();
    if items.is_empty() {
        return Ok(result);
    }

    let exclusion_repo = ImportListExclusionRepository::new(db.clone());

    match list.content_type.as_str() {
        "movie" => {
            process_movie_items(list, &items, db, &exclusion_repo, &mut result).await?;
        }
        "series" => {
            process_series_items(list, &items, db, &exclusion_repo, &mut result).await?;
        }
        unknown => {
            warn!("Unknown content type '{}' for list '{}'", unknown, list.name);
        }
    }

    Ok(result)
}

/// An item fetched from an external list
#[derive(Debug, Clone)]
pub struct ImportListItem {
    pub external_id: String, // IMDB ID like "tt1234567"
    pub title: String,
    pub year: Option<i32>,
}

/// Fetch items from an IMDB list via the pir9-imdb service
async fn fetch_imdb_list_items(
    list: &ImportListDbModel,
    imdb_client: &ImdbClient,
) -> Result<Vec<ImportListItem>> {
    if !imdb_client.is_enabled() {
        warn!("IMDB service not enabled, cannot sync IMDB import list '{}'", list.name);
        return Ok(vec![]);
    }

    let list_url = list.list_url.as_deref().unwrap_or("");
    if list_url.is_empty() {
        warn!("Import list '{}' has no list URL configured", list.name);
        return Ok(vec![]);
    }

    // Extract IMDB IDs from the list URL
    // Supports formats: "ls012345678", "ur12345678", full URLs, or comma-separated IDs
    let imdb_ids = parse_imdb_list_ids(list_url);

    if imdb_ids.is_empty() {
        warn!("No IMDB IDs found in list URL for '{}'", list.name);
        return Ok(vec![]);
    }

    let mut items = Vec::new();

    for imdb_id in &imdb_ids {
        // For individual IMDB IDs (tt-prefixed), look up directly
        if imdb_id.starts_with("tt") {
            match list.content_type.as_str() {
                "movie" => {
                    if let Ok(Some(movie)) = imdb_client.get_movie(imdb_id).await {
                        items.push(ImportListItem {
                            external_id: movie.imdb_id.clone(),
                            title: movie.title,
                            year: movie.year,
                        });
                    }
                }
                "series" => {
                    if let Ok(Some(series)) = imdb_client.get_series(imdb_id).await {
                        items.push(ImportListItem {
                            external_id: series.imdb_id.clone(),
                            title: series.title,
                            year: series.start_year,
                        });
                    }
                }
                _ => {}
            }
        }
    }

    Ok(items)
}

/// Parse IMDB IDs from a list URL or ID string
///
/// Supports:
///   - Comma-separated IMDB IDs: "tt1234567,tt7654321"
///   - IMDB list IDs: "ls012345678" (resolved by searching -- not yet implemented)
///   - Full URLs: "https://www.imdb.com/title/tt1234567/"
fn parse_imdb_list_ids(input: &str) -> Vec<String> {
    let mut ids = Vec::new();

    // Try comma-separated IDs first
    for part in input.split(',') {
        let trimmed = part.trim();

        // Extract tt-prefixed IMDB IDs from URLs or raw IDs
        if let Some(tt_id) = extract_imdb_id(trimmed) {
            ids.push(tt_id);
        } else if trimmed.starts_with("ls") || trimmed.starts_with("ur") {
            // List/user IDs -- pass through for future list-level API support
            ids.push(trimmed.to_string());
        }
    }

    ids
}

/// Extract a tt-prefixed IMDB ID from a string (URL or raw ID)
fn extract_imdb_id(s: &str) -> Option<String> {
    // Direct tt ID
    if s.starts_with("tt") && s.len() >= 9 {
        // Take just the tt + digits portion
        let id: String = s.chars().take_while(|c| c.is_alphanumeric()).collect();
        if id.starts_with("tt") {
            return Some(id);
        }
    }

    // URL containing /title/ttNNNNNNN
    if let Some(pos) = s.find("/title/tt") {
        let start = pos + 7; // length of "/title/"
        let rest = &s[start..];
        let id: String = rest.chars().take_while(|c| c.is_alphanumeric()).collect();
        if id.starts_with("tt") {
            return Some(id);
        }
    }

    None
}

/// Process movie items from an import list
async fn process_movie_items(
    list: &ImportListDbModel,
    items: &[ImportListItem],
    db: &Database,
    exclusion_repo: &ImportListExclusionRepository,
    result: &mut ImportListSyncResult,
) -> Result<()> {
    let movie_repo = MovieRepository::new(db.clone());

    for item in items {
        // Check per-list exclusion
        match exclusion_repo.is_excluded(list.id, &item.external_id).await {
            Ok(true) => {
                result.items_excluded += 1;
                continue;
            }
            Err(e) => {
                warn!("Failed to check exclusion for '{}': {}", item.external_id, e);
            }
            _ => {}
        }

        // Check if movie already exists in library by IMDB ID
        match movie_repo.get_by_imdb_id(&item.external_id).await {
            Ok(Some(_)) => {
                // Already in library, mark as processed so we don't check again
                let _ = exclusion_repo
                    .add(list.id, &item.external_id, &item.title, "movie")
                    .await;
                result.items_skipped += 1;
                continue;
            }
            Err(e) => {
                result.errors.push(format!(
                    "Failed to check existing movie '{}': {}",
                    item.title, e
                ));
                continue;
            }
            Ok(None) => {}
        }

        // Add the movie
        let clean = item.title.to_lowercase()
            .replace(|c: char| !c.is_alphanumeric() && c != ' ', " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let slug = item.title.to_lowercase()
            .replace(|c: char| !c.is_alphanumeric() && c != ' ', "-")
            .replace(' ', "-")
            .replace("--", "-")
            .trim_matches('-')
            .to_string();

        let movie = MovieDbModel {
            id: 0,
            tmdb_id: 0,
            imdb_id: Some(item.external_id.clone()),
            title: item.title.clone(),
            clean_title: clean,
            sort_title: item.title.to_lowercase(),
            status: 0, // TBA
            overview: None,
            monitored: list.monitored,
            quality_profile_id: list.quality_profile_id,
            title_slug: slug,
            path: format!("{}/{}", list.root_folder_path.trim_end_matches('/'), item.title),
            root_folder_path: list.root_folder_path.clone(),
            year: item.year.unwrap_or(0),
            release_date: None,
            physical_release_date: None,
            digital_release_date: None,
            runtime: 0,
            studio: None,
            certification: None,
            genres: "[]".to_string(),
            tags: list.tags.clone(),
            images: "[]".to_string(),
            has_file: false,
            movie_file_id: None,
            added: Utc::now(),
            last_info_sync: None,
            imdb_rating: None,
            imdb_votes: None,
        };

        match movie_repo.insert(&movie).await {
            Ok(new_id) => {
                info!(
                    "Import list '{}': added movie '{}' (id={}, imdb={})",
                    list.name, item.title, new_id, item.external_id
                );
                // Mark as processed
                let _ = exclusion_repo
                    .add(list.id, &item.external_id, &item.title, "movie")
                    .await;
                result.items_added += 1;
            }
            Err(e) => {
                result.errors.push(format!(
                    "Failed to insert movie '{}': {}",
                    item.title, e
                ));
            }
        }
    }

    Ok(())
}

/// Process series items from an import list
async fn process_series_items(
    list: &ImportListDbModel,
    items: &[ImportListItem],
    db: &Database,
    exclusion_repo: &ImportListExclusionRepository,
    result: &mut ImportListSyncResult,
) -> Result<()> {
    let series_repo = SeriesRepository::new(db.clone());

    for item in items {
        // Check per-list exclusion
        match exclusion_repo.is_excluded(list.id, &item.external_id).await {
            Ok(true) => {
                result.items_excluded += 1;
                continue;
            }
            Err(e) => {
                warn!("Failed to check exclusion for '{}': {}", item.external_id, e);
            }
            _ => {}
        }

        // Check if series already exists in library by IMDB ID
        match series_repo.get_by_imdb_id(&item.external_id).await {
            Ok(Some(_)) => {
                let _ = exclusion_repo
                    .add(list.id, &item.external_id, &item.title, "series")
                    .await;
                result.items_skipped += 1;
                continue;
            }
            Err(e) => {
                result.errors.push(format!(
                    "Failed to check existing series '{}': {}",
                    item.title, e
                ));
                continue;
            }
            Ok(None) => {}
        }

        // Add the series
        let clean = item.title.to_lowercase()
            .replace(|c: char| !c.is_alphanumeric() && c != ' ', " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let slug = item.title.to_lowercase()
            .replace(|c: char| !c.is_alphanumeric() && c != ' ', "-")
            .replace(' ', "-")
            .replace("--", "-")
            .trim_matches('-')
            .to_string();

        let series = SeriesDbModel {
            id: 0,
            tvdb_id: 0,
            tv_rage_id: 0,
            tv_maze_id: 0,
            imdb_id: Some(item.external_id.clone()),
            tmdb_id: 0,
            title: item.title.clone(),
            clean_title: clean,
            sort_title: item.title.to_lowercase(),
            status: 0, // Continuing
            overview: None,
            monitored: list.monitored,
            monitor_new_items: 0,
            quality_profile_id: list.quality_profile_id,
            language_profile_id: None,
            season_folder: true,
            series_type: 0, // Standard
            title_slug: slug,
            path: format!("{}/{}", list.root_folder_path.trim_end_matches('/'), item.title),
            root_folder_path: list.root_folder_path.clone(),
            year: item.year.unwrap_or(0),
            first_aired: None,
            last_aired: None,
            runtime: 0,
            network: None,
            certification: None,
            use_scene_numbering: false,
            episode_ordering: "aired".to_string(),
            added: Utc::now(),
            last_info_sync: None,
            imdb_rating: None,
            imdb_votes: None,
        };

        match series_repo.insert(&series).await {
            Ok(new_id) => {
                info!(
                    "Import list '{}': added series '{}' (id={}, imdb={})",
                    list.name, item.title, new_id, item.external_id
                );
                let _ = exclusion_repo
                    .add(list.id, &item.external_id, &item.title, "series")
                    .await;
                result.items_added += 1;
            }
            Err(e) => {
                result.errors.push(format!(
                    "Failed to insert series '{}': {}",
                    item.title, e
                ));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_imdb_id_direct() {
        assert_eq!(extract_imdb_id("tt1234567"), Some("tt1234567".to_string()));
        assert_eq!(extract_imdb_id("tt12345678"), Some("tt12345678".to_string()));
    }

    #[test]
    fn test_extract_imdb_id_from_url() {
        assert_eq!(
            extract_imdb_id("https://www.imdb.com/title/tt1234567/"),
            Some("tt1234567".to_string())
        );
        assert_eq!(
            extract_imdb_id("https://www.imdb.com/title/tt1234567"),
            Some("tt1234567".to_string())
        );
    }

    #[test]
    fn test_extract_imdb_id_none() {
        assert_eq!(extract_imdb_id("ls012345678"), None);
        assert_eq!(extract_imdb_id("random text"), None);
        assert_eq!(extract_imdb_id(""), None);
    }

    #[test]
    fn test_parse_imdb_list_ids() {
        let ids = parse_imdb_list_ids("tt1234567, tt7654321");
        assert_eq!(ids, vec!["tt1234567", "tt7654321"]);

        let ids = parse_imdb_list_ids("tt1234567");
        assert_eq!(ids, vec!["tt1234567"]);

        let ids = parse_imdb_list_ids("ls012345678");
        assert_eq!(ids, vec!["ls012345678"]);

        let ids = parse_imdb_list_ids("https://www.imdb.com/title/tt1234567/");
        assert_eq!(ids, vec!["tt1234567"]);
    }
}
