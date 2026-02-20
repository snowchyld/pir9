//! TVMaze API client
//!
//! Used to backfill episode air dates that aren't available in IMDB datasets.
//! TVMaze provides a free, no-auth API with show lookups by IMDB ID.

use anyhow::Result;
use serde::Deserialize;
use tracing::debug;

const TVMAZE_BASE_URL: &str = "https://api.tvmaze.com";

/// A single episode from the TVMaze API
#[derive(Debug, Deserialize)]
pub struct TvMazeEpisode {
    pub season: i32,
    pub number: Option<i32>,
    pub name: Option<String>,
    pub airdate: Option<String>,
}

/// Response from the TVMaze show lookup endpoint
#[derive(Debug, Deserialize)]
struct TvMazeShow {
    id: i64,
}

/// Look up a TVMaze show ID by IMDB ID (numeric, without "tt" prefix).
/// Returns None if the show isn't found on TVMaze.
pub async fn lookup_show(client: &reqwest::Client, imdb_id: i64) -> Result<Option<i64>> {
    let url = format!(
        "{}/lookup/shows?imdb=tt{:07}",
        TVMAZE_BASE_URL, imdb_id
    );

    let response = client.get(&url).send().await?;

    if response.status().as_u16() == 404 {
        debug!("TVMaze: no show found for tt{:07}", imdb_id);
        return Ok(None);
    }

    if !response.status().is_success() {
        anyhow::bail!(
            "TVMaze lookup failed for tt{:07}: HTTP {}",
            imdb_id,
            response.status()
        );
    }

    let show: TvMazeShow = response.json().await?;
    Ok(Some(show.id))
}

/// Get all episodes for a TVMaze show.
pub async fn get_episodes(client: &reqwest::Client, show_id: i64) -> Result<Vec<TvMazeEpisode>> {
    let url = format!("{}/shows/{}/episodes", TVMAZE_BASE_URL, show_id);

    let response = client.get(&url).send().await?;

    if response.status().as_u16() == 404 {
        return Ok(vec![]);
    }

    if !response.status().is_success() {
        anyhow::bail!(
            "TVMaze episodes failed for show {}: HTTP {}",
            show_id,
            response.status()
        );
    }

    let episodes: Vec<TvMazeEpisode> = response.json().await?;
    Ok(episodes)
}
