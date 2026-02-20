//! Command API endpoints (v3)
//! Delegates to v5 implementation for actual command execution

use axum::{
    extract::{Path, State},
    response::{IntoResponse, Json},
    http::StatusCode,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::datastore::repositories::CommandRepository;
use crate::web::AppState;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandResource {
    pub id: i32,
    pub name: String,
    pub command_name: String,
    pub message: Option<String>,
    pub body: serde_json::Value,
    pub priority: String,
    pub status: String,
    pub result: String,
    pub queued: String,
    pub started: Option<String>,
    pub ended: Option<String>,
    pub duration: Option<String>,
    pub trigger: String,
    pub state_change_time: Option<String>,
    pub send_updates_to_client: bool,
    pub update_scheduled_task: bool,
    pub last_execution_time: Option<String>,
}

impl From<crate::core::datastore::repositories::CommandDbModel> for CommandResource {
    fn from(cmd: crate::core::datastore::repositories::CommandDbModel) -> Self {
        let body: serde_json::Value = cmd.body
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or(serde_json::json!({}));

        Self {
            id: cmd.id as i32,
            name: cmd.name,
            command_name: cmd.command_name,
            message: cmd.message,
            body,
            priority: cmd.priority,
            status: cmd.status,
            result: cmd.result.unwrap_or_else(|| "unknown".to_string()),
            queued: cmd.queued.to_rfc3339(),
            started: cmd.started.map(|d| d.to_rfc3339()),
            ended: cmd.ended.map(|d| d.to_rfc3339()),
            duration: cmd.duration,
            trigger: cmd.trigger,
            state_change_time: Some(cmd.state_change_time.to_rfc3339()),
            send_updates_to_client: cmd.send_updates_to_client,
            update_scheduled_task: cmd.update_scheduled_task,
            last_execution_time: cmd.last_execution_time.map(|d| d.to_rfc3339()),
        }
    }
}

/// Error type for command operations
#[derive(Debug)]
pub enum CommandError {
    NotFound,
    Validation(String),
    Internal(String),
}

impl IntoResponse for CommandError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            CommandError::NotFound => (StatusCode::NOT_FOUND, "Command not found".to_string()),
            CommandError::Validation(msg) => (StatusCode::BAD_REQUEST, msg),
            CommandError::Internal(msg) => {
                tracing::error!("Command error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string())
            }
        };

        (status, Json(serde_json::json!({ "message": message }))).into_response()
    }
}

/// GET /api/v3/command - List all commands
pub async fn get_commands(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<CommandResource>>, CommandError> {
    let repo = CommandRepository::new(state.db.clone());

    let commands = repo.get_all().await
        .map_err(|e| CommandError::Internal(format!("Failed to fetch commands: {}", e)))?;

    let resources: Vec<CommandResource> = commands.into_iter().map(Into::into).collect();
    Ok(Json(resources))
}

/// GET /api/v3/command/{id} - Get a specific command
pub async fn get_command(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<Json<CommandResource>, CommandError> {
    let repo = CommandRepository::new(state.db.clone());

    let command = repo.get_by_id(id as i64).await
        .map_err(|e| CommandError::Internal(format!("Failed to fetch command: {}", e)))?
        .ok_or(CommandError::NotFound)?;

    Ok(Json(command.into()))
}

/// POST /api/v3/command - Create/queue a new command
/// Uses the same implementation as v5 for actual command execution
pub async fn create_command(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<CommandResource>, CommandError> {
    let name = body.get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CommandError::Validation("Command name is required".to_string()))?;

    let repo = CommandRepository::new(state.db.clone());

    let body_str = serde_json::to_string(&body).ok();
    let id = repo.insert(name, name, body_str.as_deref(), "manual").await
        .map_err(|e| CommandError::Internal(format!("Failed to create command: {}", e)))?;

    tracing::info!("v3: Queued command: id={}, name={}", id, name);

    // Fetch the created command to return
    let command = repo.get_by_id(id).await
        .map_err(|e| CommandError::Internal(format!("Failed to fetch command: {}", e)))?
        .ok_or(CommandError::NotFound)?;

    // Spawn background task to execute the command (same as v5)
    tokio::spawn({
        let db = state.db.clone();
        let event_bus = state.event_bus.clone();
        let metadata_service = state.metadata_service.clone();
        let cmd_id = id;
        let cmd_name = name.to_string();
        let cmd_body = body.clone();
        async move {
            use crate::core::messaging::Message;

            let repo = CommandRepository::new(db.clone());
            if let Err(e) = repo.start_command(cmd_id).await {
                tracing::error!("Failed to start command {}: {}", cmd_id, e);
                return;
            }

            // Publish command started event
            event_bus.publish(Message::CommandStarted {
                command_id: cmd_id,
                name: cmd_name.clone(),
                message: Some(format!("Starting {}", cmd_name)),
            }).await;

            // Execute command based on type (reuse v5 with metadata service)
            let options = crate::api::v5::command::CommandExecutionOptions {
                hybrid_event_bus: None,
                metadata_service: Some(metadata_service),
            };
            let result = crate::api::v5::command::execute_command_with_options(&cmd_name, &cmd_body, &db, &event_bus, options).await;

            // Mark as completed or failed
            match result {
                Ok(msg) => {
                    if let Err(e) = repo.update_status(cmd_id, "completed", Some("successful")).await {
                        tracing::error!("Failed to complete command {}: {}", cmd_id, e);
                    } else {
                        tracing::info!("v3: Completed command: id={}, name={}, result={}", cmd_id, cmd_name, msg);
                    }
                    event_bus.publish(Message::CommandCompleted {
                        command_id: cmd_id,
                        name: cmd_name,
                        message: Some(msg),
                    }).await;
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    if let Err(e) = repo.update_status(cmd_id, "failed", Some("failed")).await {
                        tracing::error!("Failed to mark command {} as failed: {}", cmd_id, e);
                    }
                    tracing::error!("v3: Failed command: id={}, name={}, error={}", cmd_id, cmd_name, error_msg);
                    event_bus.publish(Message::CommandFailed {
                        command_id: cmd_id,
                        name: cmd_name,
                        message: None,
                        error: error_msg,
                    }).await;
                }
            }
        }
    });

    Ok(Json(command.into()))
}

/// DELETE /api/v3/command/{id} - Cancel a command
pub async fn delete_command(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<Json<serde_json::Value>, CommandError> {
    let repo = CommandRepository::new(state.db.clone());

    // Mark as cancelled
    repo.update_status(id as i64, "cancelled", Some("cancelled")).await
        .map_err(|e| CommandError::Internal(format!("Failed to cancel command: {}", e)))?;

    Ok(Json(serde_json::json!({})))
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_commands).post(create_command))
        .route("/{id}", get(get_command).delete(delete_command))
}
