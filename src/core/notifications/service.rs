//! Notification service
//! Subscribes to events and dispatches notifications to configured providers

use anyhow::Result;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use super::{
    create_provider_from_model, HealthInfo, NotificationEventType, NotificationPayload, ReleaseInfo,
};
use crate::core::datastore::repositories::NotificationRepository;
use crate::core::datastore::Database;
use crate::core::messaging::{EventBus, Message};

/// Service that listens to events and sends notifications
pub struct NotificationService {
    db: Database,
    event_bus: EventBus,
}

impl NotificationService {
    pub fn new(db: Database, event_bus: EventBus) -> Self {
        Self { db, event_bus }
    }

    /// Start listening for events and dispatching notifications
    /// This should be spawned as a background task
    pub async fn start_event_listener(self: Arc<Self>) {
        info!("Starting notification service event listener");
        let mut rx = self.event_bus.subscribe();

        loop {
            match rx.recv().await {
                Ok(message) => {
                    if let Err(e) = self.handle_message(&message).await {
                        error!("Failed to handle message for notifications: {}", e);
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    warn!("Notification service lagged behind by {} messages", n);
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    info!("Event bus closed, notification service stopping");
                    break;
                }
            }
        }
    }

    /// Handle an incoming message and dispatch notifications if appropriate
    async fn handle_message(&self, message: &Message) -> Result<()> {
        let payload = match self.message_to_payload(message) {
            Some(p) => p,
            None => return Ok(()), // Not a notifiable event
        };

        self.send_notifications(&payload).await
    }

    /// Convert a Message to a NotificationPayload if it's a notifiable event
    fn message_to_payload(&self, message: &Message) -> Option<NotificationPayload> {
        match message {
            Message::ReleaseGrabbed {
                series_id,
                release_title,
                indexer,
                size,
                ..
            } => Some(NotificationPayload {
                event_type: NotificationEventType::Grab,
                title: "Episode Grabbed".to_string(),
                message: format!("Release grabbed: {}", release_title),
                series_title: None, // Would need to look up series
                series_id: Some(*series_id),
                episode_info: None,
                release_info: Some(ReleaseInfo {
                    release_title: release_title.clone(),
                    indexer: indexer.clone(),
                    size: *size,
                    quality: None,
                }),
                health_info: None,
            }),

            Message::DownloadCompleted {
                series_id,
                episode_file_id: _,
                ..
            } => Some(NotificationPayload {
                event_type: NotificationEventType::Download,
                title: "Episode Downloaded".to_string(),
                message: "Episode file has been imported".to_string(),
                series_title: None,
                series_id: Some(*series_id),
                episode_info: None,
                release_info: None,
                health_info: None,
            }),

            Message::EpisodeFileImported {
                series_id,
                episode_ids,
                ..
            } => Some(NotificationPayload {
                event_type: NotificationEventType::Download,
                title: "Episode Imported".to_string(),
                message: format!("Imported {} episode(s)", episode_ids.len()),
                series_title: None,
                series_id: Some(*series_id),
                episode_info: None,
                release_info: None,
                health_info: None,
            }),

            Message::SeriesDeleted { series_id, title } => Some(NotificationPayload {
                event_type: NotificationEventType::SeriesDelete,
                title: "Series Deleted".to_string(),
                message: format!("Series '{}' has been deleted", title),
                series_title: Some(title.clone()),
                series_id: Some(*series_id),
                episode_info: None,
                release_info: None,
                health_info: None,
            }),

            Message::EpisodeFileDeleted { series_id, .. } => Some(NotificationPayload {
                event_type: NotificationEventType::EpisodeFileDelete,
                title: "Episode File Deleted".to_string(),
                message: "An episode file has been deleted".to_string(),
                series_title: None,
                series_id: Some(*series_id),
                episode_info: None,
                release_info: None,
                health_info: None,
            }),

            Message::HealthCheckChanged => Some(NotificationPayload {
                event_type: NotificationEventType::HealthIssue,
                title: "Health Check".to_string(),
                message: "System health status has changed".to_string(),
                series_title: None,
                series_id: None,
                episode_info: None,
                release_info: None,
                health_info: Some(HealthInfo {
                    source: "System".to_string(),
                    check_type: "HealthCheck".to_string(),
                    message: "Health status changed".to_string(),
                    wiki_url: None,
                }),
            }),

            // Events that don't trigger notifications
            Message::CommandStarted { .. }
            | Message::CommandUpdated { .. }
            | Message::CommandCompleted { .. }
            | Message::CommandFailed { .. }
            | Message::SeriesAdded { .. }
            | Message::SeriesUpdated { .. }
            | Message::SeriesRefreshed { .. }
            | Message::SeriesScanned { .. }
            | Message::MovieAdded { .. }
            | Message::MovieUpdated { .. }
            | Message::MovieDeleted { .. }
            | Message::MovieRefreshed { .. }
            | Message::MovieFileImported { .. }
            | Message::MovieFileDeleted { .. }
            | Message::EpisodeAdded { .. }
            | Message::EpisodeUpdated { .. }
            | Message::EpisodeSearchRequested { .. }
            | Message::SeasonSearchRequested { .. }
            | Message::SeriesSearchRequested { .. }
            | Message::DownloadStarted { .. }
            | Message::DownloadFailed { .. }
            | Message::QueueUpdated
            | Message::ConfigUpdated
            | Message::NotificationSent { .. }
            // Distributed scanning events (internal, no notifications)
            | Message::ScanRequest { .. }
            | Message::ScanResult { .. }
            | Message::WorkerOnline { .. }
            | Message::WorkerOffline { .. }
            | Message::WorkerHeartbeat { .. } => None,
        }
    }

    /// Send notifications to all providers enabled for the event type
    async fn send_notifications(&self, payload: &NotificationPayload) -> Result<()> {
        let repo = NotificationRepository::new(self.db.clone());
        let event_key = payload.event_type.as_event_key();

        let notifications = repo.get_enabled_for_event(event_key).await?;

        if notifications.is_empty() {
            debug!("No notifications configured for event: {}", event_key);
            return Ok(());
        }

        info!(
            "Sending {} notification(s) for event: {}",
            notifications.len(),
            event_key
        );

        for notification in notifications {
            let provider = match create_provider_from_model(&notification) {
                Ok(p) => p,
                Err(e) => {
                    error!(
                        "Failed to create provider for notification '{}': {}",
                        notification.name, e
                    );
                    continue;
                }
            };

            match provider.send(payload).await {
                Ok(()) => {
                    info!(
                        "Successfully sent notification '{}' via {}",
                        notification.name,
                        provider.implementation()
                    );
                    // Publish success event
                    self.event_bus
                        .publish(Message::NotificationSent {
                            notification_type: provider.implementation().to_string(),
                            success: true,
                        })
                        .await;
                }
                Err(e) => {
                    error!(
                        "Failed to send notification '{}' via {}: {}",
                        notification.name,
                        provider.implementation(),
                        e
                    );
                    // Publish failure event
                    self.event_bus
                        .publish(Message::NotificationSent {
                            notification_type: provider.implementation().to_string(),
                            success: false,
                        })
                        .await;
                }
            }
        }

        Ok(())
    }

    /// Test a specific notification configuration
    pub async fn test_notification(&self, notification_id: i64) -> Result<()> {
        let repo = NotificationRepository::new(self.db.clone());
        let notification = repo
            .get_by_id(notification_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Notification not found"))?;

        let provider = create_provider_from_model(&notification)?;
        provider.test().await
    }

    /// Test all configured notifications
    pub async fn test_all_notifications(&self) -> Vec<(String, Result<()>)> {
        let repo = NotificationRepository::new(self.db.clone());
        let notifications = match repo.get_all().await {
            Ok(n) => n,
            Err(e) => {
                return vec![("Database".to_string(), Err(e))];
            }
        };

        let mut results = Vec::new();

        for notification in notifications {
            let name = notification.name.clone();
            let result = match create_provider_from_model(&notification) {
                Ok(provider) => provider.test().await,
                Err(e) => Err(e),
            };
            results.push((name, result));
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type_mapping() {
        assert_eq!(NotificationEventType::Grab.as_event_key(), "grab");
        assert_eq!(NotificationEventType::Download.as_event_key(), "download");
        assert_eq!(
            NotificationEventType::SeriesDelete.as_event_key(),
            "series_delete"
        );
    }
}
