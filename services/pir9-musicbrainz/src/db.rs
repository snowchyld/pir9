//! Database operations for the MusicBrainz service

use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use tracing::info;

use crate::models::*;

/// Info needed to resume a previously failed/cancelled sync
pub struct ResumeInfo {
    pub sync_id: i64,
    pub last_processed_id: String,
    pub rows_processed: i64,
    pub rows_inserted: i64,
    pub rows_updated: i64,
}

/// Repository for database operations
#[derive(Clone)]
pub struct DbRepository {
    pool: PgPool,
}

impl DbRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get pool for direct access (used by sync)
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    // ── Artist queries ──────────────────────────────────────────────

    /// Search artists by name
    ///
    /// Relevance ranking: exact match > starts-with > contains.
    /// Within each tier, sorts by rating_count (popularity) then rating.
    pub async fn search_artists(&self, query: &str, limit: u32) -> Result<Vec<MbArtist>> {
        let exact_pattern = query.to_string();
        let starts_pattern = format!("{}%", query);
        let contains_pattern = format!("%{}%", query);
        let limit = limit.min(100) as i64;

        let rows = sqlx::query(
            r#"
            SELECT mbid, name, sort_name, artist_type, gender, area,
                   begin_date, end_date, disambiguation, genres, tags,
                   rating, rating_count, last_synced_at
            FROM mb_artists
            WHERE name ILIKE $3 OR sort_name ILIKE $3
            ORDER BY
                CASE
                    WHEN name ILIKE $1 OR sort_name ILIKE $1 THEN 0
                    WHEN name ILIKE $2 OR sort_name ILIKE $2 THEN 1
                    ELSE 2
                END,
                rating_count DESC NULLS LAST,
                rating DESC NULLS LAST
            LIMIT $4
            "#,
        )
        .bind(&exact_pattern)
        .bind(&starts_pattern)
        .bind(&contains_pattern)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let results: Vec<MbArtist> = rows
            .iter()
            .map(|row| {
                let db_artist = DbArtist {
                    mbid: row.get("mbid"),
                    name: row.get("name"),
                    sort_name: row.get("sort_name"),
                    artist_type: row.get("artist_type"),
                    gender: row.get("gender"),
                    area: row.get("area"),
                    begin_date: row.get("begin_date"),
                    end_date: row.get("end_date"),
                    disambiguation: row.get("disambiguation"),
                    genres: row.get("genres"),
                    tags: row.get("tags"),
                    rating: row.get("rating"),
                    rating_count: row.get("rating_count"),
                    last_synced_at: row.get("last_synced_at"),
                };
                db_artist.to_api()
            })
            .collect();

        Ok(results)
    }

    /// Get an artist by MBID
    pub async fn get_artist(&self, mbid: &str) -> Result<Option<MbArtist>> {
        let row = sqlx::query(
            r#"
            SELECT mbid, name, sort_name, artist_type, gender, area,
                   begin_date, end_date, disambiguation, genres, tags,
                   rating, rating_count, last_synced_at
            FROM mb_artists
            WHERE mbid = $1
            "#,
        )
        .bind(mbid)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| {
            let db_artist = DbArtist {
                mbid: row.get("mbid"),
                name: row.get("name"),
                sort_name: row.get("sort_name"),
                artist_type: row.get("artist_type"),
                gender: row.get("gender"),
                area: row.get("area"),
                begin_date: row.get("begin_date"),
                end_date: row.get("end_date"),
                disambiguation: row.get("disambiguation"),
                genres: row.get("genres"),
                tags: row.get("tags"),
                rating: row.get("rating"),
                rating_count: row.get("rating_count"),
                last_synced_at: row.get("last_synced_at"),
            };
            db_artist.to_api()
        }))
    }

    /// Get release groups (albums) for an artist
    pub async fn get_artist_albums(&self, artist_mbid: &str) -> Result<Vec<MbReleaseGroup>> {
        let rows = sqlx::query(
            r#"
            SELECT rg.mbid, rg.title, rg.artist_mbid, rg.artist_name,
                   rg.primary_type, rg.secondary_types, rg.first_release_date,
                   rg.genres, rg.tags, rg.rating, rg.rating_count, rg.last_synced_at,
                   ca.front_url AS cover_art_url
            FROM mb_release_groups rg
            LEFT JOIN mb_releases r ON r.release_group_mbid = rg.mbid
            LEFT JOIN mb_cover_art ca ON ca.release_mbid = r.mbid AND ca.has_front = true
            WHERE rg.artist_mbid = $1
            ORDER BY rg.first_release_date ASC NULLS LAST
            "#,
        )
        .bind(artist_mbid)
        .fetch_all(&self.pool)
        .await?;

        // Deduplicate release groups (a group may join to multiple releases with cover art)
        let mut seen = std::collections::HashSet::new();
        let mut results: Vec<MbReleaseGroup> = Vec::new();

        for row in &rows {
            let mbid: String = row.get("mbid");
            if !seen.insert(mbid.clone()) {
                continue;
            }

            let db_rg = DbReleaseGroup {
                mbid,
                title: row.get("title"),
                artist_mbid: row.get("artist_mbid"),
                artist_name: row.get("artist_name"),
                primary_type: row.get("primary_type"),
                secondary_types: row.get("secondary_types"),
                first_release_date: row.get("first_release_date"),
                genres: row.get("genres"),
                tags: row.get("tags"),
                rating: row.get("rating"),
                rating_count: row.get("rating_count"),
                last_synced_at: row.get("last_synced_at"),
            };
            let mut api = db_rg.to_api();
            api.cover_art_url = row.get("cover_art_url");
            results.push(api);
        }

        Ok(results)
    }

    /// Get a release group by MBID
    pub async fn get_release_group(&self, mbid: &str) -> Result<Option<MbReleaseGroup>> {
        let row = sqlx::query(
            r#"
            SELECT rg.mbid, rg.title, rg.artist_mbid, rg.artist_name,
                   rg.primary_type, rg.secondary_types, rg.first_release_date,
                   rg.genres, rg.tags, rg.rating, rg.rating_count, rg.last_synced_at,
                   ca.front_url AS cover_art_url
            FROM mb_release_groups rg
            LEFT JOIN mb_releases r ON r.release_group_mbid = rg.mbid
            LEFT JOIN mb_cover_art ca ON ca.release_mbid = r.mbid AND ca.has_front = true
            WHERE rg.mbid = $1
            LIMIT 1
            "#,
        )
        .bind(mbid)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| {
            let db_rg = DbReleaseGroup {
                mbid: row.get("mbid"),
                title: row.get("title"),
                artist_mbid: row.get("artist_mbid"),
                artist_name: row.get("artist_name"),
                primary_type: row.get("primary_type"),
                secondary_types: row.get("secondary_types"),
                first_release_date: row.get("first_release_date"),
                genres: row.get("genres"),
                tags: row.get("tags"),
                rating: row.get("rating"),
                rating_count: row.get("rating_count"),
                last_synced_at: row.get("last_synced_at"),
            };
            let mut api = db_rg.to_api();
            api.cover_art_url = row.get("cover_art_url");
            api
        }))
    }

    /// Get releases for a release group
    pub async fn get_releases_for_group(&self, release_group_mbid: &str) -> Result<Vec<MbRelease>> {
        let rows = sqlx::query(
            r#"
            SELECT mbid, release_group_mbid, title, artist_mbid, artist_name,
                   date, country, status, barcode, packaging, track_count, last_synced_at
            FROM mb_releases
            WHERE release_group_mbid = $1
            ORDER BY date ASC NULLS LAST
            "#,
        )
        .bind(release_group_mbid)
        .fetch_all(&self.pool)
        .await?;

        let results: Vec<MbRelease> = rows
            .iter()
            .map(|row| {
                let db_release = DbRelease {
                    mbid: row.get("mbid"),
                    release_group_mbid: row.get("release_group_mbid"),
                    title: row.get("title"),
                    artist_mbid: row.get("artist_mbid"),
                    artist_name: row.get("artist_name"),
                    date: row.get("date"),
                    country: row.get("country"),
                    status: row.get("status"),
                    barcode: row.get("barcode"),
                    packaging: row.get("packaging"),
                    track_count: row.get("track_count"),
                    last_synced_at: row.get("last_synced_at"),
                };
                db_release.to_api()
            })
            .collect();

        Ok(results)
    }

    /// Get track listing for a specific release
    pub async fn get_release_tracks(&self, release_mbid: &str) -> Result<Vec<MbReleaseTrack>> {
        use sqlx::Row;

        let rows = sqlx::query(
            r#"
            SELECT disc_number, position, title, recording_mbid, length_ms
            FROM mb_release_tracks
            WHERE release_mbid = $1
            ORDER BY disc_number, position
            "#,
        )
        .bind(release_mbid)
        .fetch_all(&self.pool)
        .await?;

        let results = rows
            .iter()
            .map(|row| MbReleaseTrack {
                disc_number: row.get("disc_number"),
                position: row.get("position"),
                title: row.get("title"),
                recording_mbid: row.get("recording_mbid"),
                length_ms: row.get("length_ms"),
            })
            .collect();

        Ok(results)
    }

    /// Upsert a batch of release tracks using UNNEST for bulk efficiency.
    /// Accepts tracks from multiple releases in one call.
    /// Chunks into 10K-row inserts to limit per-query WAL pressure.
    pub async fn upsert_release_tracks_bulk(&self, tracks: &[DbReleaseTrack]) -> Result<u64> {
        if tracks.is_empty() {
            return Ok(0);
        }

        // Collect distinct release MBIDs and delete their old tracks in one query
        let release_mbids: Vec<&str> = tracks
            .iter()
            .map(|t| t.release_mbid.as_str())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        sqlx::query("DELETE FROM mb_release_tracks WHERE release_mbid = ANY($1)")
            .bind(&release_mbids)
            .execute(&self.pool)
            .await?;

        // Insert in chunks to avoid massive single-query WAL writes
        const CHUNK_SIZE: usize = 10_000;
        let mut total_affected = 0u64;

        for chunk in tracks.chunks(CHUNK_SIZE) {
            let mut t_release_mbids = Vec::with_capacity(chunk.len());
            let mut t_disc_numbers = Vec::with_capacity(chunk.len());
            let mut t_positions = Vec::with_capacity(chunk.len());
            let mut t_titles = Vec::with_capacity(chunk.len());
            let mut t_recording_mbids = Vec::with_capacity(chunk.len());
            let mut t_length_ms = Vec::with_capacity(chunk.len());

            for track in chunk {
                t_release_mbids.push(track.release_mbid.as_str());
                t_disc_numbers.push(track.disc_number);
                t_positions.push(track.position);
                t_titles.push(track.title.as_str());
                t_recording_mbids.push(track.recording_mbid.as_deref());
                t_length_ms.push(track.length_ms);
            }

            let result = sqlx::query(
                r#"
                INSERT INTO mb_release_tracks (release_mbid, disc_number, position, title, recording_mbid, length_ms)
                SELECT * FROM UNNEST(
                    $1::text[], $2::int[], $3::int[], $4::text[], $5::text[], $6::int[]
                )
                ON CONFLICT (release_mbid, disc_number, position) DO UPDATE SET
                    title = EXCLUDED.title,
                    recording_mbid = EXCLUDED.recording_mbid,
                    length_ms = EXCLUDED.length_ms
                "#,
            )
            .bind(&t_release_mbids)
            .bind(&t_disc_numbers)
            .bind(&t_positions)
            .bind(&t_titles)
            .bind(&t_recording_mbids)
            .bind(&t_length_ms)
            .execute(&self.pool)
            .await?;

            total_affected += result.rows_affected();
        }

        Ok(total_affected)
    }

    // ── Statistics ───────────────────────────────────────────────────

    /// Get database statistics
    pub async fn get_stats(&self) -> Result<MbStats> {
        let artist_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mb_artists")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);

        let release_group_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM mb_release_groups")
                .fetch_one(&self.pool)
                .await
                .unwrap_or(0);

        let release_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mb_releases")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);

        let cover_art_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mb_cover_art")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);

        let label_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mb_labels")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);

        let recording_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mb_recordings")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);

        let work_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mb_works")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);

        let area_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mb_areas")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);

        let series_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mb_series")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);

        let event_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mb_events")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);

        let instrument_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mb_instruments")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);

        let place_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mb_places")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);

        // Get last sync time
        let last_sync: Option<String> = sqlx::query_scalar(
            r#"
            SELECT TO_CHAR(completed_at, 'YYYY-MM-DD"T"HH24:MI:SS"Z"')
            FROM mb_sync_status
            WHERE status = 'completed'
            ORDER BY completed_at DESC
            LIMIT 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await
        .unwrap_or(None);

        // Get database size
        let db_size: Option<i64> =
            sqlx::query_scalar("SELECT pg_database_size(current_database())")
                .fetch_optional(&self.pool)
                .await
                .unwrap_or(None);

        Ok(MbStats {
            artist_count,
            release_group_count,
            release_count,
            cover_art_count,
            label_count,
            recording_count,
            work_count,
            area_count,
            series_count,
            event_count,
            instrument_count,
            place_count,
            last_sync,
            db_size_bytes: db_size,
        })
    }

    // ── Sync status ─────────────────────────────────────────────────

    /// Get sync status for all datasets
    pub async fn get_sync_status(&self) -> Result<SyncStatus> {
        async fn get_dataset_status(
            pool: &PgPool,
            dataset: &str,
        ) -> Result<Option<DatasetSyncStatus>> {
            let row = sqlx::query(
                r#"
                SELECT dataset_name, rows_processed, rows_inserted, rows_updated,
                       started_at, completed_at, status, error_message
                FROM mb_sync_status
                WHERE dataset_name = $1
                ORDER BY CASE WHEN status = 'running' THEN 0 ELSE 1 END, started_at DESC
                LIMIT 1
                "#,
            )
            .bind(dataset)
            .fetch_optional(pool)
            .await?;

            Ok(row.map(|r| DatasetSyncStatus {
                dataset_name: r.get("dataset_name"),
                rows_processed: r.get("rows_processed"),
                rows_inserted: r.get("rows_inserted"),
                rows_updated: r.get("rows_updated"),
                started_at: r
                    .get::<chrono::DateTime<chrono::Utc>, _>("started_at")
                    .to_rfc3339(),
                completed_at: r
                    .get::<Option<chrono::DateTime<chrono::Utc>>, _>("completed_at")
                    .map(|d| d.to_rfc3339()),
                status: r.get("status"),
                error_message: r.get("error_message"),
                is_running: false,
                download_progress: None,
                download_size_bytes: None,
                download_bytes_done: None,
                current_phase: None,
            }))
        }

        Ok(SyncStatus {
            is_running: false, // Enriched by the handler from the in-memory SyncHandle
            artists: get_dataset_status(&self.pool, "artist.tar.xz").await?,
            release_groups: get_dataset_status(&self.pool, "release-group.tar.xz").await?,
            releases: get_dataset_status(&self.pool, "release.tar.xz").await?,
            labels: get_dataset_status(&self.pool, "label.tar.xz").await?,
            recordings: get_dataset_status(&self.pool, "recording.tar.xz").await?,
            works: get_dataset_status(&self.pool, "work.tar.xz").await?,
            areas: get_dataset_status(&self.pool, "area.tar.xz").await?,
            series: get_dataset_status(&self.pool, "series.tar.xz").await?,
            events: get_dataset_status(&self.pool, "event.tar.xz").await?,
            instruments: get_dataset_status(&self.pool, "instrument.tar.xz").await?,
            places: get_dataset_status(&self.pool, "place.tar.xz").await?,
        })
    }

    /// Start a sync record
    pub async fn start_sync(&self, dataset_name: &str) -> Result<i64> {
        let id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO mb_sync_status (dataset_name, started_at, status, rows_processed, rows_inserted, rows_updated)
            VALUES ($1, NOW(), 'running', 0, 0, 0)
            RETURNING id
            "#,
        )
        .bind(dataset_name)
        .fetch_one(&self.pool)
        .await?;

        Ok(id)
    }

    /// Update sync progress and save resume checkpoint
    pub async fn update_sync_progress_with_resume(
        &self,
        sync_id: i64,
        rows_processed: i64,
        rows_inserted: i64,
        rows_updated: i64,
        last_processed_id: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE mb_sync_status
            SET rows_processed = $2, rows_inserted = $3, rows_updated = $4, last_processed_id = $5
            WHERE id = $1
            "#,
        )
        .bind(sync_id)
        .bind(rows_processed)
        .bind(rows_inserted)
        .bind(rows_updated)
        .bind(last_processed_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get the last completed sync time for a dataset (for skip-if-recent)
    pub async fn last_completed_sync_time(&self, dataset: &str) -> Result<Option<DateTime<Utc>>> {
        let result: Option<DateTime<Utc>> = sqlx::query_scalar(
            r#"
            SELECT completed_at
            FROM mb_sync_status
            WHERE dataset_name = $1 AND status = 'completed'
            ORDER BY completed_at DESC
            LIMIT 1
            "#,
        )
        .bind(dataset)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    /// Find a failed/cancelled sync that can be resumed
    pub async fn get_resumable_sync(&self, dataset: &str) -> Result<Option<ResumeInfo>> {
        let row = sqlx::query(
            r#"
            SELECT id, last_processed_id, rows_processed, rows_inserted, rows_updated
            FROM mb_sync_status
            WHERE dataset_name = $1
              AND status IN ('failed', 'cancelled')
              AND last_processed_id IS NOT NULL
              AND last_processed_id != ''
            ORDER BY started_at DESC
            LIMIT 1
            "#,
        )
        .bind(dataset)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| ResumeInfo {
            sync_id: r.get("id"),
            last_processed_id: r.get("last_processed_id"),
            rows_processed: r.get("rows_processed"),
            rows_inserted: r.get("rows_inserted"),
            rows_updated: r.get("rows_updated"),
        }))
    }

    /// Resume a previously failed/cancelled sync
    pub async fn resume_sync(&self, sync_id: i64) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE mb_sync_status
            SET status = 'running', completed_at = NULL, error_message = NULL
            WHERE id = $1
            "#,
        )
        .bind(sync_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Mark any stale 'running' syncs as failed (startup cleanup after crash)
    pub async fn fail_stale_running_syncs(&self) -> Result<i64> {
        let result = sqlx::query(
            r#"
            UPDATE mb_sync_status
            SET status = 'failed', completed_at = NOW(), error_message = 'Marked as failed on startup (stale running sync)'
            WHERE status = 'running'
            "#,
        )
        .execute(&self.pool)
        .await?;

        let count = result.rows_affected() as i64;
        if count > 0 {
            info!(
                "Cleaned up {} stale running sync(s) from previous run",
                count
            );
        }
        Ok(count)
    }

    /// Cancel a sync by setting its status
    pub async fn cancel_sync(&self, sync_id: i64) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE mb_sync_status
            SET status = 'cancelled', completed_at = NOW(), error_message = 'Cancelled by user'
            WHERE id = $1
            "#,
        )
        .bind(sync_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Complete a sync
    pub async fn complete_sync(&self, sync_id: i64) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE mb_sync_status
            SET status = 'completed', completed_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(sync_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Fail a sync
    pub async fn fail_sync(&self, sync_id: i64, error: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE mb_sync_status
            SET status = 'failed', completed_at = NOW(), error_message = $2
            WHERE id = $1
            "#,
        )
        .bind(sync_id)
        .bind(error)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // ── Batch upsert methods (UNNEST-based) ─────────────────────────

    /// Batch upsert artists using UNNEST arrays
    pub async fn upsert_artist_batch(&self, batch: &[DbArtist]) -> Result<u64> {
        if batch.is_empty() {
            return Ok(0);
        }

        let mut mbids = Vec::with_capacity(batch.len());
        let mut names = Vec::with_capacity(batch.len());
        let mut sort_names = Vec::with_capacity(batch.len());
        let mut artist_types = Vec::with_capacity(batch.len());
        let mut genders = Vec::with_capacity(batch.len());
        let mut areas = Vec::with_capacity(batch.len());
        let mut begin_dates = Vec::with_capacity(batch.len());
        let mut end_dates = Vec::with_capacity(batch.len());
        let mut disambiguations = Vec::with_capacity(batch.len());
        let mut genres_vec = Vec::with_capacity(batch.len());
        let mut tags_vec = Vec::with_capacity(batch.len());
        let mut ratings = Vec::with_capacity(batch.len());
        let mut rating_counts = Vec::with_capacity(batch.len());
        let mut synced_ats = Vec::with_capacity(batch.len());

        for a in batch {
            mbids.push(a.mbid.as_str());
            names.push(a.name.as_str());
            sort_names.push(a.sort_name.as_str());
            artist_types.push(a.artist_type.as_deref());
            genders.push(a.gender.as_deref());
            areas.push(a.area.as_deref());
            begin_dates.push(a.begin_date.as_deref());
            end_dates.push(a.end_date.as_deref());
            disambiguations.push(a.disambiguation.as_deref());
            genres_vec.push(a.genres.as_str());
            tags_vec.push(a.tags.as_str());
            ratings.push(a.rating);
            rating_counts.push(a.rating_count);
            synced_ats.push(a.last_synced_at);
        }

        let result = sqlx::query(
            r#"
            INSERT INTO mb_artists (mbid, name, sort_name, artist_type, gender, area,
                                    begin_date, end_date, disambiguation, genres, tags,
                                    rating, rating_count, last_synced_at)
            SELECT * FROM UNNEST(
                $1::text[], $2::text[], $3::text[], $4::text[], $5::text[], $6::text[],
                $7::text[], $8::text[], $9::text[], $10::text[], $11::text[],
                $12::float8[], $13::int[], $14::timestamptz[]
            )
            ON CONFLICT (mbid) DO UPDATE SET
                name = EXCLUDED.name,
                sort_name = EXCLUDED.sort_name,
                artist_type = EXCLUDED.artist_type,
                gender = EXCLUDED.gender,
                area = EXCLUDED.area,
                begin_date = EXCLUDED.begin_date,
                end_date = EXCLUDED.end_date,
                disambiguation = EXCLUDED.disambiguation,
                genres = EXCLUDED.genres,
                tags = EXCLUDED.tags,
                rating = COALESCE(EXCLUDED.rating, mb_artists.rating),
                rating_count = COALESCE(EXCLUDED.rating_count, mb_artists.rating_count),
                last_synced_at = EXCLUDED.last_synced_at
            "#,
        )
        .bind(&mbids)
        .bind(&names)
        .bind(&sort_names)
        .bind(&artist_types)
        .bind(&genders)
        .bind(&areas)
        .bind(&begin_dates)
        .bind(&end_dates)
        .bind(&disambiguations)
        .bind(&genres_vec)
        .bind(&tags_vec)
        .bind(&ratings)
        .bind(&rating_counts)
        .bind(&synced_ats)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Batch upsert release groups using UNNEST arrays
    pub async fn upsert_release_group_batch(&self, batch: &[DbReleaseGroup]) -> Result<u64> {
        if batch.is_empty() {
            return Ok(0);
        }

        let mut mbids = Vec::with_capacity(batch.len());
        let mut titles = Vec::with_capacity(batch.len());
        let mut artist_mbids = Vec::with_capacity(batch.len());
        let mut artist_names = Vec::with_capacity(batch.len());
        let mut primary_types = Vec::with_capacity(batch.len());
        let mut secondary_types_vec = Vec::with_capacity(batch.len());
        let mut first_release_dates = Vec::with_capacity(batch.len());
        let mut genres_vec = Vec::with_capacity(batch.len());
        let mut tags_vec = Vec::with_capacity(batch.len());
        let mut ratings = Vec::with_capacity(batch.len());
        let mut rating_counts = Vec::with_capacity(batch.len());
        let mut synced_ats = Vec::with_capacity(batch.len());

        for rg in batch {
            mbids.push(rg.mbid.as_str());
            titles.push(rg.title.as_str());
            artist_mbids.push(rg.artist_mbid.as_str());
            artist_names.push(rg.artist_name.as_str());
            primary_types.push(rg.primary_type.as_deref());
            secondary_types_vec.push(rg.secondary_types.as_str());
            first_release_dates.push(rg.first_release_date.as_deref());
            genres_vec.push(rg.genres.as_str());
            tags_vec.push(rg.tags.as_str());
            ratings.push(rg.rating);
            rating_counts.push(rg.rating_count);
            synced_ats.push(rg.last_synced_at);
        }

        let result = sqlx::query(
            r#"
            INSERT INTO mb_release_groups (mbid, title, artist_mbid, artist_name,
                                           primary_type, secondary_types, first_release_date,
                                           genres, tags, rating, rating_count, last_synced_at)
            SELECT * FROM UNNEST(
                $1::text[], $2::text[], $3::text[], $4::text[],
                $5::text[], $6::text[], $7::text[],
                $8::text[], $9::text[], $10::float8[], $11::int[], $12::timestamptz[]
            )
            ON CONFLICT (mbid) DO UPDATE SET
                title = EXCLUDED.title,
                artist_mbid = EXCLUDED.artist_mbid,
                artist_name = EXCLUDED.artist_name,
                primary_type = EXCLUDED.primary_type,
                secondary_types = EXCLUDED.secondary_types,
                first_release_date = EXCLUDED.first_release_date,
                genres = EXCLUDED.genres,
                tags = EXCLUDED.tags,
                rating = COALESCE(EXCLUDED.rating, mb_release_groups.rating),
                rating_count = COALESCE(EXCLUDED.rating_count, mb_release_groups.rating_count),
                last_synced_at = EXCLUDED.last_synced_at
            "#,
        )
        .bind(&mbids)
        .bind(&titles)
        .bind(&artist_mbids)
        .bind(&artist_names)
        .bind(&primary_types)
        .bind(&secondary_types_vec)
        .bind(&first_release_dates)
        .bind(&genres_vec)
        .bind(&tags_vec)
        .bind(&ratings)
        .bind(&rating_counts)
        .bind(&synced_ats)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Batch upsert releases using UNNEST arrays
    pub async fn upsert_release_batch(&self, batch: &[DbRelease]) -> Result<u64> {
        if batch.is_empty() {
            return Ok(0);
        }

        let mut mbids = Vec::with_capacity(batch.len());
        let mut release_group_mbids = Vec::with_capacity(batch.len());
        let mut titles = Vec::with_capacity(batch.len());
        let mut artist_mbids = Vec::with_capacity(batch.len());
        let mut artist_names = Vec::with_capacity(batch.len());
        let mut dates = Vec::with_capacity(batch.len());
        let mut countries = Vec::with_capacity(batch.len());
        let mut statuses = Vec::with_capacity(batch.len());
        let mut barcodes = Vec::with_capacity(batch.len());
        let mut packagings = Vec::with_capacity(batch.len());
        let mut track_counts = Vec::with_capacity(batch.len());
        let mut synced_ats = Vec::with_capacity(batch.len());

        for r in batch {
            mbids.push(r.mbid.as_str());
            release_group_mbids.push(r.release_group_mbid.as_str());
            titles.push(r.title.as_str());
            artist_mbids.push(r.artist_mbid.as_deref());
            artist_names.push(r.artist_name.as_deref());
            dates.push(r.date.as_deref());
            countries.push(r.country.as_deref());
            statuses.push(r.status.as_deref());
            barcodes.push(r.barcode.as_deref());
            packagings.push(r.packaging.as_deref());
            track_counts.push(r.track_count);
            synced_ats.push(r.last_synced_at);
        }

        let result = sqlx::query(
            r#"
            INSERT INTO mb_releases (mbid, release_group_mbid, title, artist_mbid, artist_name,
                                     date, country, status, barcode, packaging, track_count, last_synced_at)
            SELECT * FROM UNNEST(
                $1::text[], $2::text[], $3::text[], $4::text[], $5::text[],
                $6::text[], $7::text[], $8::text[], $9::text[], $10::text[], $11::int[], $12::timestamptz[]
            )
            ON CONFLICT (mbid) DO UPDATE SET
                release_group_mbid = EXCLUDED.release_group_mbid,
                title = EXCLUDED.title,
                artist_mbid = EXCLUDED.artist_mbid,
                artist_name = EXCLUDED.artist_name,
                date = EXCLUDED.date,
                country = EXCLUDED.country,
                status = EXCLUDED.status,
                barcode = EXCLUDED.barcode,
                packaging = EXCLUDED.packaging,
                track_count = EXCLUDED.track_count,
                last_synced_at = EXCLUDED.last_synced_at
            "#,
        )
        .bind(&mbids)
        .bind(&release_group_mbids)
        .bind(&titles)
        .bind(&artist_mbids)
        .bind(&artist_names)
        .bind(&dates)
        .bind(&countries)
        .bind(&statuses)
        .bind(&barcodes)
        .bind(&packagings)
        .bind(&track_counts)
        .bind(&synced_ats)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    // ── Label queries ────────────────────────────────────────────────

    /// Search labels by name
    pub async fn search_labels(&self, query: &str, limit: u32) -> Result<Vec<MbLabel>> {
        let exact_pattern = query.to_string();
        let starts_pattern = format!("{}%", query);
        let contains_pattern = format!("%{}%", query);
        let limit = limit.min(100) as i64;

        let rows = sqlx::query(
            r#"
            SELECT mbid, name, sort_name, label_type, area, label_code,
                   begin_date, end_date, disambiguation, genres, tags, last_synced_at
            FROM mb_labels
            WHERE name ILIKE $3
            ORDER BY
                CASE
                    WHEN name ILIKE $1 THEN 0
                    WHEN name ILIKE $2 THEN 1
                    ELSE 2
                END,
                name ASC
            LIMIT $4
            "#,
        )
        .bind(&exact_pattern)
        .bind(&starts_pattern)
        .bind(&contains_pattern)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let results: Vec<MbLabel> = rows
            .iter()
            .map(|row| {
                let db = DbLabel {
                    mbid: row.get("mbid"),
                    name: row.get("name"),
                    sort_name: row.get("sort_name"),
                    label_type: row.get("label_type"),
                    area: row.get("area"),
                    label_code: row.get("label_code"),
                    begin_date: row.get("begin_date"),
                    end_date: row.get("end_date"),
                    disambiguation: row.get("disambiguation"),
                    genres: row.get("genres"),
                    tags: row.get("tags"),
                    last_synced_at: row.get("last_synced_at"),
                };
                db.to_api()
            })
            .collect();

        Ok(results)
    }

    /// Get a label by MBID
    pub async fn get_label(&self, mbid: &str) -> Result<Option<MbLabel>> {
        let row = sqlx::query(
            r#"
            SELECT mbid, name, sort_name, label_type, area, label_code,
                   begin_date, end_date, disambiguation, genres, tags, last_synced_at
            FROM mb_labels
            WHERE mbid = $1
            "#,
        )
        .bind(mbid)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| {
            let db = DbLabel {
                mbid: row.get("mbid"),
                name: row.get("name"),
                sort_name: row.get("sort_name"),
                label_type: row.get("label_type"),
                area: row.get("area"),
                label_code: row.get("label_code"),
                begin_date: row.get("begin_date"),
                end_date: row.get("end_date"),
                disambiguation: row.get("disambiguation"),
                genres: row.get("genres"),
                tags: row.get("tags"),
                last_synced_at: row.get("last_synced_at"),
            };
            db.to_api()
        }))
    }

    // ── Recording queries ──────────────────────────────────────────

    /// Search recordings by title
    pub async fn search_recordings(&self, query: &str, limit: u32) -> Result<Vec<MbRecording>> {
        let exact_pattern = query.to_string();
        let starts_pattern = format!("{}%", query);
        let contains_pattern = format!("%{}%", query);
        let limit = limit.min(100) as i64;

        let rows = sqlx::query(
            r#"
            SELECT mbid, title, artist_mbid, artist_name, length_ms,
                   first_release_date, isrcs, genres, tags, last_synced_at
            FROM mb_recordings
            WHERE title ILIKE $3
            ORDER BY
                CASE
                    WHEN title ILIKE $1 THEN 0
                    WHEN title ILIKE $2 THEN 1
                    ELSE 2
                END,
                title ASC
            LIMIT $4
            "#,
        )
        .bind(&exact_pattern)
        .bind(&starts_pattern)
        .bind(&contains_pattern)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let results: Vec<MbRecording> = rows
            .iter()
            .map(|row| {
                let db = DbRecording {
                    mbid: row.get("mbid"),
                    title: row.get("title"),
                    artist_mbid: row.get("artist_mbid"),
                    artist_name: row.get("artist_name"),
                    length_ms: row.get("length_ms"),
                    first_release_date: row.get("first_release_date"),
                    isrcs: row.get("isrcs"),
                    genres: row.get("genres"),
                    tags: row.get("tags"),
                    last_synced_at: row.get("last_synced_at"),
                };
                db.to_api()
            })
            .collect();

        Ok(results)
    }

    /// Get a recording by MBID
    pub async fn get_recording(&self, mbid: &str) -> Result<Option<MbRecording>> {
        let row = sqlx::query(
            r#"
            SELECT mbid, title, artist_mbid, artist_name, length_ms,
                   first_release_date, isrcs, genres, tags, last_synced_at
            FROM mb_recordings
            WHERE mbid = $1
            "#,
        )
        .bind(mbid)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| {
            let db = DbRecording {
                mbid: row.get("mbid"),
                title: row.get("title"),
                artist_mbid: row.get("artist_mbid"),
                artist_name: row.get("artist_name"),
                length_ms: row.get("length_ms"),
                first_release_date: row.get("first_release_date"),
                isrcs: row.get("isrcs"),
                genres: row.get("genres"),
                tags: row.get("tags"),
                last_synced_at: row.get("last_synced_at"),
            };
            db.to_api()
        }))
    }

    // ── Batch upsert: Labels ───────────────────────────────────────

    pub async fn upsert_label_batch(&self, batch: &[DbLabel]) -> Result<u64> {
        if batch.is_empty() {
            return Ok(0);
        }

        let mut mbids = Vec::with_capacity(batch.len());
        let mut names = Vec::with_capacity(batch.len());
        let mut sort_names = Vec::with_capacity(batch.len());
        let mut label_types = Vec::with_capacity(batch.len());
        let mut areas = Vec::with_capacity(batch.len());
        let mut label_codes = Vec::with_capacity(batch.len());
        let mut begin_dates = Vec::with_capacity(batch.len());
        let mut end_dates = Vec::with_capacity(batch.len());
        let mut disambiguations = Vec::with_capacity(batch.len());
        let mut genres_vec = Vec::with_capacity(batch.len());
        let mut tags_vec = Vec::with_capacity(batch.len());
        let mut synced_ats = Vec::with_capacity(batch.len());

        for l in batch {
            mbids.push(l.mbid.as_str());
            names.push(l.name.as_str());
            sort_names.push(l.sort_name.as_deref());
            label_types.push(l.label_type.as_deref());
            areas.push(l.area.as_deref());
            label_codes.push(l.label_code);
            begin_dates.push(l.begin_date.as_deref());
            end_dates.push(l.end_date.as_deref());
            disambiguations.push(l.disambiguation.as_deref());
            genres_vec.push(l.genres.as_str());
            tags_vec.push(l.tags.as_str());
            synced_ats.push(l.last_synced_at);
        }

        let result = sqlx::query(
            r#"
            INSERT INTO mb_labels (mbid, name, sort_name, label_type, area, label_code,
                                   begin_date, end_date, disambiguation, genres, tags, last_synced_at)
            SELECT * FROM UNNEST(
                $1::text[], $2::text[], $3::text[], $4::text[], $5::text[], $6::int[],
                $7::text[], $8::text[], $9::text[], $10::text[], $11::text[], $12::timestamptz[]
            )
            ON CONFLICT (mbid) DO UPDATE SET
                name = EXCLUDED.name,
                sort_name = EXCLUDED.sort_name,
                label_type = EXCLUDED.label_type,
                area = EXCLUDED.area,
                label_code = EXCLUDED.label_code,
                begin_date = EXCLUDED.begin_date,
                end_date = EXCLUDED.end_date,
                disambiguation = EXCLUDED.disambiguation,
                genres = EXCLUDED.genres,
                tags = EXCLUDED.tags,
                last_synced_at = EXCLUDED.last_synced_at
            "#,
        )
        .bind(&mbids)
        .bind(&names)
        .bind(&sort_names)
        .bind(&label_types)
        .bind(&areas)
        .bind(&label_codes)
        .bind(&begin_dates)
        .bind(&end_dates)
        .bind(&disambiguations)
        .bind(&genres_vec)
        .bind(&tags_vec)
        .bind(&synced_ats)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    // ── Batch upsert: Recordings ───────────────────────────────────

    pub async fn upsert_recording_batch(&self, batch: &[DbRecording]) -> Result<u64> {
        if batch.is_empty() {
            return Ok(0);
        }

        let mut mbids = Vec::with_capacity(batch.len());
        let mut titles = Vec::with_capacity(batch.len());
        let mut artist_mbids = Vec::with_capacity(batch.len());
        let mut artist_names = Vec::with_capacity(batch.len());
        let mut length_ms_vec = Vec::with_capacity(batch.len());
        let mut first_release_dates = Vec::with_capacity(batch.len());
        let mut isrcs_vec = Vec::with_capacity(batch.len());
        let mut genres_vec = Vec::with_capacity(batch.len());
        let mut tags_vec = Vec::with_capacity(batch.len());
        let mut synced_ats = Vec::with_capacity(batch.len());

        for r in batch {
            mbids.push(r.mbid.as_str());
            titles.push(r.title.as_str());
            artist_mbids.push(r.artist_mbid.as_deref());
            artist_names.push(r.artist_name.as_deref());
            length_ms_vec.push(r.length_ms);
            first_release_dates.push(r.first_release_date.as_deref());
            isrcs_vec.push(r.isrcs.as_str());
            genres_vec.push(r.genres.as_str());
            tags_vec.push(r.tags.as_str());
            synced_ats.push(r.last_synced_at);
        }

        let result = sqlx::query(
            r#"
            INSERT INTO mb_recordings (mbid, title, artist_mbid, artist_name, length_ms,
                                       first_release_date, isrcs, genres, tags, last_synced_at)
            SELECT * FROM UNNEST(
                $1::text[], $2::text[], $3::text[], $4::text[], $5::int[],
                $6::text[], $7::text[], $8::text[], $9::text[], $10::timestamptz[]
            )
            ON CONFLICT (mbid) DO UPDATE SET
                title = EXCLUDED.title,
                artist_mbid = EXCLUDED.artist_mbid,
                artist_name = EXCLUDED.artist_name,
                length_ms = EXCLUDED.length_ms,
                first_release_date = EXCLUDED.first_release_date,
                isrcs = EXCLUDED.isrcs,
                genres = EXCLUDED.genres,
                tags = EXCLUDED.tags,
                last_synced_at = EXCLUDED.last_synced_at
            "#,
        )
        .bind(&mbids)
        .bind(&titles)
        .bind(&artist_mbids)
        .bind(&artist_names)
        .bind(&length_ms_vec)
        .bind(&first_release_dates)
        .bind(&isrcs_vec)
        .bind(&genres_vec)
        .bind(&tags_vec)
        .bind(&synced_ats)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    // ── Batch upsert: Works ────────────────────────────────────────

    pub async fn upsert_work_batch(&self, batch: &[DbWork]) -> Result<u64> {
        if batch.is_empty() {
            return Ok(0);
        }

        let mut mbids = Vec::with_capacity(batch.len());
        let mut titles = Vec::with_capacity(batch.len());
        let mut work_types = Vec::with_capacity(batch.len());
        let mut languages_vec = Vec::with_capacity(batch.len());
        let mut iswcs_vec = Vec::with_capacity(batch.len());
        let mut disambiguations = Vec::with_capacity(batch.len());
        let mut genres_vec = Vec::with_capacity(batch.len());
        let mut tags_vec = Vec::with_capacity(batch.len());
        let mut synced_ats = Vec::with_capacity(batch.len());

        for w in batch {
            mbids.push(w.mbid.as_str());
            titles.push(w.title.as_str());
            work_types.push(w.work_type.as_deref());
            languages_vec.push(w.languages.as_str());
            iswcs_vec.push(w.iswcs.as_str());
            disambiguations.push(w.disambiguation.as_deref());
            genres_vec.push(w.genres.as_str());
            tags_vec.push(w.tags.as_str());
            synced_ats.push(w.last_synced_at);
        }

        let result = sqlx::query(
            r#"
            INSERT INTO mb_works (mbid, title, work_type, languages, iswcs,
                                  disambiguation, genres, tags, last_synced_at)
            SELECT * FROM UNNEST(
                $1::text[], $2::text[], $3::text[], $4::text[], $5::text[],
                $6::text[], $7::text[], $8::text[], $9::timestamptz[]
            )
            ON CONFLICT (mbid) DO UPDATE SET
                title = EXCLUDED.title,
                work_type = EXCLUDED.work_type,
                languages = EXCLUDED.languages,
                iswcs = EXCLUDED.iswcs,
                disambiguation = EXCLUDED.disambiguation,
                genres = EXCLUDED.genres,
                tags = EXCLUDED.tags,
                last_synced_at = EXCLUDED.last_synced_at
            "#,
        )
        .bind(&mbids)
        .bind(&titles)
        .bind(&work_types)
        .bind(&languages_vec)
        .bind(&iswcs_vec)
        .bind(&disambiguations)
        .bind(&genres_vec)
        .bind(&tags_vec)
        .bind(&synced_ats)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    // ── Batch upsert: Areas ────────────────────────────────────────

    pub async fn upsert_area_batch(&self, batch: &[DbArea]) -> Result<u64> {
        if batch.is_empty() {
            return Ok(0);
        }

        let mut mbids = Vec::with_capacity(batch.len());
        let mut names = Vec::with_capacity(batch.len());
        let mut sort_names = Vec::with_capacity(batch.len());
        let mut area_types = Vec::with_capacity(batch.len());
        let mut iso_3166_1s = Vec::with_capacity(batch.len());
        let mut iso_3166_2s = Vec::with_capacity(batch.len());
        let mut disambiguations = Vec::with_capacity(batch.len());
        let mut synced_ats = Vec::with_capacity(batch.len());

        for a in batch {
            mbids.push(a.mbid.as_str());
            names.push(a.name.as_str());
            sort_names.push(a.sort_name.as_deref());
            area_types.push(a.area_type.as_deref());
            iso_3166_1s.push(a.iso_3166_1.as_deref());
            iso_3166_2s.push(a.iso_3166_2.as_deref());
            disambiguations.push(a.disambiguation.as_deref());
            synced_ats.push(a.last_synced_at);
        }

        let result = sqlx::query(
            r#"
            INSERT INTO mb_areas (mbid, name, sort_name, area_type, iso_3166_1, iso_3166_2,
                                  disambiguation, last_synced_at)
            SELECT * FROM UNNEST(
                $1::text[], $2::text[], $3::text[], $4::text[], $5::text[], $6::text[],
                $7::text[], $8::timestamptz[]
            )
            ON CONFLICT (mbid) DO UPDATE SET
                name = EXCLUDED.name,
                sort_name = EXCLUDED.sort_name,
                area_type = EXCLUDED.area_type,
                iso_3166_1 = EXCLUDED.iso_3166_1,
                iso_3166_2 = EXCLUDED.iso_3166_2,
                disambiguation = EXCLUDED.disambiguation,
                last_synced_at = EXCLUDED.last_synced_at
            "#,
        )
        .bind(&mbids)
        .bind(&names)
        .bind(&sort_names)
        .bind(&area_types)
        .bind(&iso_3166_1s)
        .bind(&iso_3166_2s)
        .bind(&disambiguations)
        .bind(&synced_ats)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    // ── Batch upsert: Series ───────────────────────────────────────

    pub async fn upsert_series_batch(&self, batch: &[DbSeries]) -> Result<u64> {
        if batch.is_empty() {
            return Ok(0);
        }

        let mut mbids = Vec::with_capacity(batch.len());
        let mut names = Vec::with_capacity(batch.len());
        let mut series_types = Vec::with_capacity(batch.len());
        let mut disambiguations = Vec::with_capacity(batch.len());
        let mut synced_ats = Vec::with_capacity(batch.len());

        for s in batch {
            mbids.push(s.mbid.as_str());
            names.push(s.name.as_str());
            series_types.push(s.series_type.as_deref());
            disambiguations.push(s.disambiguation.as_deref());
            synced_ats.push(s.last_synced_at);
        }

        let result = sqlx::query(
            r#"
            INSERT INTO mb_series (mbid, name, series_type, disambiguation, last_synced_at)
            SELECT * FROM UNNEST(
                $1::text[], $2::text[], $3::text[], $4::text[], $5::timestamptz[]
            )
            ON CONFLICT (mbid) DO UPDATE SET
                name = EXCLUDED.name,
                series_type = EXCLUDED.series_type,
                disambiguation = EXCLUDED.disambiguation,
                last_synced_at = EXCLUDED.last_synced_at
            "#,
        )
        .bind(&mbids)
        .bind(&names)
        .bind(&series_types)
        .bind(&disambiguations)
        .bind(&synced_ats)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    // ── Batch upsert: Events ───────────────────────────────────────

    pub async fn upsert_event_batch(&self, batch: &[DbEvent]) -> Result<u64> {
        if batch.is_empty() {
            return Ok(0);
        }

        let mut mbids = Vec::with_capacity(batch.len());
        let mut names = Vec::with_capacity(batch.len());
        let mut event_types = Vec::with_capacity(batch.len());
        let mut begin_dates = Vec::with_capacity(batch.len());
        let mut end_dates = Vec::with_capacity(batch.len());
        let mut times = Vec::with_capacity(batch.len());
        let mut cancelled_vec = Vec::with_capacity(batch.len());
        let mut disambiguations = Vec::with_capacity(batch.len());
        let mut synced_ats = Vec::with_capacity(batch.len());

        for e in batch {
            mbids.push(e.mbid.as_str());
            names.push(e.name.as_str());
            event_types.push(e.event_type.as_deref());
            begin_dates.push(e.begin_date.as_deref());
            end_dates.push(e.end_date.as_deref());
            times.push(e.time.as_deref());
            cancelled_vec.push(e.cancelled);
            disambiguations.push(e.disambiguation.as_deref());
            synced_ats.push(e.last_synced_at);
        }

        let result = sqlx::query(
            r#"
            INSERT INTO mb_events (mbid, name, event_type, begin_date, end_date, time,
                                   cancelled, disambiguation, last_synced_at)
            SELECT * FROM UNNEST(
                $1::text[], $2::text[], $3::text[], $4::text[], $5::text[], $6::text[],
                $7::bool[], $8::text[], $9::timestamptz[]
            )
            ON CONFLICT (mbid) DO UPDATE SET
                name = EXCLUDED.name,
                event_type = EXCLUDED.event_type,
                begin_date = EXCLUDED.begin_date,
                end_date = EXCLUDED.end_date,
                time = EXCLUDED.time,
                cancelled = EXCLUDED.cancelled,
                disambiguation = EXCLUDED.disambiguation,
                last_synced_at = EXCLUDED.last_synced_at
            "#,
        )
        .bind(&mbids)
        .bind(&names)
        .bind(&event_types)
        .bind(&begin_dates)
        .bind(&end_dates)
        .bind(&times)
        .bind(&cancelled_vec)
        .bind(&disambiguations)
        .bind(&synced_ats)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    // ── Batch upsert: Instruments ──────────────────────────────────

    pub async fn upsert_instrument_batch(&self, batch: &[DbInstrument]) -> Result<u64> {
        if batch.is_empty() {
            return Ok(0);
        }

        let mut mbids = Vec::with_capacity(batch.len());
        let mut names = Vec::with_capacity(batch.len());
        let mut instrument_types = Vec::with_capacity(batch.len());
        let mut descriptions = Vec::with_capacity(batch.len());
        let mut disambiguations = Vec::with_capacity(batch.len());
        let mut synced_ats = Vec::with_capacity(batch.len());

        for i in batch {
            mbids.push(i.mbid.as_str());
            names.push(i.name.as_str());
            instrument_types.push(i.instrument_type.as_deref());
            descriptions.push(i.description.as_deref());
            disambiguations.push(i.disambiguation.as_deref());
            synced_ats.push(i.last_synced_at);
        }

        let result = sqlx::query(
            r#"
            INSERT INTO mb_instruments (mbid, name, instrument_type, description,
                                        disambiguation, last_synced_at)
            SELECT * FROM UNNEST(
                $1::text[], $2::text[], $3::text[], $4::text[], $5::text[], $6::timestamptz[]
            )
            ON CONFLICT (mbid) DO UPDATE SET
                name = EXCLUDED.name,
                instrument_type = EXCLUDED.instrument_type,
                description = EXCLUDED.description,
                disambiguation = EXCLUDED.disambiguation,
                last_synced_at = EXCLUDED.last_synced_at
            "#,
        )
        .bind(&mbids)
        .bind(&names)
        .bind(&instrument_types)
        .bind(&descriptions)
        .bind(&disambiguations)
        .bind(&synced_ats)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    // ── Batch upsert: Places ───────────────────────────────────────

    pub async fn upsert_place_batch(&self, batch: &[DbPlace]) -> Result<u64> {
        if batch.is_empty() {
            return Ok(0);
        }

        let mut mbids = Vec::with_capacity(batch.len());
        let mut names = Vec::with_capacity(batch.len());
        let mut place_types = Vec::with_capacity(batch.len());
        let mut areas = Vec::with_capacity(batch.len());
        let mut coordinates_vec = Vec::with_capacity(batch.len());
        let mut disambiguations = Vec::with_capacity(batch.len());
        let mut synced_ats = Vec::with_capacity(batch.len());

        for p in batch {
            mbids.push(p.mbid.as_str());
            names.push(p.name.as_str());
            place_types.push(p.place_type.as_deref());
            areas.push(p.area.as_deref());
            coordinates_vec.push(p.coordinates.as_deref());
            disambiguations.push(p.disambiguation.as_deref());
            synced_ats.push(p.last_synced_at);
        }

        let result = sqlx::query(
            r#"
            INSERT INTO mb_places (mbid, name, place_type, area, coordinates,
                                   disambiguation, last_synced_at)
            SELECT * FROM UNNEST(
                $1::text[], $2::text[], $3::text[], $4::text[], $5::text[],
                $6::text[], $7::timestamptz[]
            )
            ON CONFLICT (mbid) DO UPDATE SET
                name = EXCLUDED.name,
                place_type = EXCLUDED.place_type,
                area = EXCLUDED.area,
                coordinates = EXCLUDED.coordinates,
                disambiguation = EXCLUDED.disambiguation,
                last_synced_at = EXCLUDED.last_synced_at
            "#,
        )
        .bind(&mbids)
        .bind(&names)
        .bind(&place_types)
        .bind(&areas)
        .bind(&coordinates_vec)
        .bind(&disambiguations)
        .bind(&synced_ats)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    // ── Cover art ───────────────────────────────────────────────────

    /// Update cover art cache for a release
    pub async fn update_cover_art(
        &self,
        release_mbid: &str,
        front_url: Option<&str>,
        thumbnail_250: Option<&str>,
        thumbnail_500: Option<&str>,
    ) -> Result<()> {
        let has_front = front_url.is_some();

        sqlx::query(
            r#"
            INSERT INTO mb_cover_art (release_mbid, has_front, front_url, thumbnail_250, thumbnail_500, fetched_at)
            VALUES ($1, $2, $3, $4, $5, NOW())
            ON CONFLICT (release_mbid) DO UPDATE SET
                has_front = EXCLUDED.has_front,
                front_url = EXCLUDED.front_url,
                thumbnail_250 = EXCLUDED.thumbnail_250,
                thumbnail_500 = EXCLUDED.thumbnail_500,
                fetched_at = EXCLUDED.fetched_at
            "#,
        )
        .bind(release_mbid)
        .bind(has_front)
        .bind(front_url)
        .bind(thumbnail_250)
        .bind(thumbnail_500)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
