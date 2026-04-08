#![allow(dead_code)]
//! Download history tracking

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::datastore::models::HistoryDbModel;
use crate::core::datastore::repositories::HistoryRepository;
use crate::core::datastore::Database;

/// Download history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadHistory {
    pub id: i64,
    pub series_id: i64,
    pub episode_ids: Vec<i64>,
    pub download_id: String,
    pub source_title: String,
    pub quality: crate::core::profiles::qualities::QualityModel,
    pub languages: Vec<crate::core::profiles::languages::Language>,
    pub protocol: super::Protocol,
    pub indexer: String,
    pub download_client: String,
    pub event_type: HistoryEventType,
    pub date: DateTime<Utc>,
    pub data: serde_json::Value,
}

/// History event types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HistoryEventType {
    Unknown,
    Grabbed,
    DownloadFailed,
    DownloadFolderImported,
    DownloadIgnored,
    FileImported,
    FileDeleted,
    FileRenamed,
}

impl HistoryEventType {
    pub fn to_db_int(self) -> i32 {
        match self {
            Self::Unknown => 0,
            Self::Grabbed => 1,
            Self::DownloadFailed => 2,
            Self::DownloadFolderImported => 3,
            Self::FileDeleted => 4,
            Self::FileRenamed => 5,
            Self::DownloadIgnored => 6,
            Self::FileImported => 3,
        }
    }
}

/// History service — records events in the history table via `HistoryRepository`
pub struct HistoryService {
    db: Database,
}

impl HistoryService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Record a grab event (event_type = 1)
    pub async fn record_grab(
        &self,
        series_id: i64,
        episode_ids: Vec<i64>,
        release: &super::super::indexers::ReleaseInfo,
    ) -> anyhow::Result<()> {
        let repo = HistoryRepository::new(self.db.clone());
        let quality_json =
            serde_json::to_string(&release.quality).unwrap_or_else(|_| "{}".to_string());
        let languages_json =
            serde_json::to_string(&release.languages).unwrap_or_else(|_| "[]".to_string());
        let data = serde_json::json!({
            "indexer": release.indexer,
            "releaseGroup": release.release_group,
            "size": release.size,
            "downloadUrl": release.download_url,
            "guid": release.guid,
        });

        for &episode_id in &episode_ids {
            let model = HistoryDbModel {
                id: 0,
                series_id: Some(series_id),
                episode_id: Some(episode_id),
                movie_id: None,
                source_title: release.title.clone(),
                quality: quality_json.clone().into(),
                languages: languages_json.clone().into(),
                custom_formats: "[]".to_string().into(),
                custom_format_score: 0,
                quality_cutoff_not_met: false,
                date: Utc::now(),
                download_id: release.download_url.clone(),
                event_type: HistoryEventType::Grabbed.to_db_int(),
                data: data.to_string().into(),
            };
            repo.insert(&model).await?;
        }
        Ok(())
    }

    /// Record a download failure (event_type = 2)
    pub async fn record_download_failed(
        &self,
        series_id: i64,
        episode_ids: Vec<i64>,
        download_id: &str,
        message: &str,
    ) -> anyhow::Result<()> {
        let repo = HistoryRepository::new(self.db.clone());
        let data = serde_json::json!({ "message": message });

        for &episode_id in &episode_ids {
            let model = HistoryDbModel {
                id: 0,
                series_id: Some(series_id),
                episode_id: Some(episode_id),
                movie_id: None,
                source_title: String::new(),
                quality: "{}".to_string().into(),
                languages: "[]".to_string().into(),
                custom_formats: "[]".to_string().into(),
                custom_format_score: 0,
                quality_cutoff_not_met: false,
                date: Utc::now(),
                download_id: Some(download_id.to_string()),
                event_type: HistoryEventType::DownloadFailed.to_db_int(),
                data: data.to_string().into(),
            };
            repo.insert(&model).await?;
        }
        Ok(())
    }

    /// Record a successful import (event_type = 3)
    pub async fn record_import(
        &self,
        series_id: i64,
        episode_ids: Vec<i64>,
        episode_file_id: i64,
        is_upgrade: bool,
    ) -> anyhow::Result<()> {
        let repo = HistoryRepository::new(self.db.clone());
        let data = serde_json::json!({
            "episodeFileId": episode_file_id,
            "isUpgrade": is_upgrade,
        });

        for &episode_id in &episode_ids {
            let model = HistoryDbModel {
                id: 0,
                series_id: Some(series_id),
                episode_id: Some(episode_id),
                movie_id: None,
                source_title: String::new(),
                quality: "{}".to_string().into(),
                languages: "[]".to_string().into(),
                custom_formats: "[]".to_string().into(),
                custom_format_score: 0,
                quality_cutoff_not_met: false,
                date: Utc::now(),
                download_id: None,
                event_type: HistoryEventType::DownloadFolderImported.to_db_int(),
                data: data.to_string().into(),
            };
            repo.insert(&model).await?;
        }
        Ok(())
    }
}
