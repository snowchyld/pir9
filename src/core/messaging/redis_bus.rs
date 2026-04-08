//! Redis-backed event bus for distributed deployments
//!
//! Uses Redis Streams for durable job dispatch and result delivery,
//! and Redis pub/sub for ephemeral broadcasts (progress, heartbeats, UI events).
//!
//! Stream topology:
//!   pir9:stream:jobs    — server → workers (consumer group: pir9-workers)
//!   pir9:stream:results — workers → server (consumer group: pir9-server)
//!   pir9:events         — pub/sub for ephemeral broadcasts

#[cfg(feature = "redis-events")]
use anyhow::{Context, Result};
#[cfg(feature = "redis-events")]
use redis::aio::ConnectionManager;
#[cfg(feature = "redis-events")]
use redis::AsyncCommands;
use tokio::sync::broadcast;
#[cfg(feature = "redis-events")]
use tokio::sync::mpsc;
#[cfg(feature = "redis-events")]
use tracing::{debug, error, info, trace, warn};

#[cfg(feature = "redis-events")]
use crate::core::messaging::{
    Message, MessageCategory, REDIS_JOB_STREAM, REDIS_RESULT_STREAM, REDIS_SERVER_GROUP,
    REDIS_WORKER_GROUP, STREAM_MAXLEN,
};

/// Redis channel name for ephemeral pub/sub events
#[cfg(feature = "redis-events")]
const REDIS_CHANNEL: &str = "pir9:events";

/// Redis-backed event bus using Streams + pub/sub
///
/// - Streams for durable job dispatch and result delivery
/// - Pub/sub for ephemeral broadcasts (progress, heartbeats, UI events)
#[cfg(feature = "redis-events")]
pub struct RedisEventBus {
    /// Local broadcast sender for in-process subscribers
    local_sender: broadcast::Sender<Message>,
    /// Redis connection manager for commands (XADD, XACK, PUBLISH)
    redis_conn: ConnectionManager,
    /// Redis URL for creating additional connections
    redis_url: String,
    /// Instance ID to prevent pub/sub echo
    instance_id: String,
}

#[cfg(feature = "redis-events")]
impl std::fmt::Debug for RedisEventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedisEventBus")
            .field("instance_id", &self.instance_id)
            .field("redis_url", &"<redacted>")
            .finish()
    }
}

#[cfg(feature = "redis-events")]
impl RedisEventBus {
    /// Create a new Redis event bus
    pub async fn new(redis_url: &str) -> Result<Self> {
        let client = redis::Client::open(redis_url).context("Failed to create Redis client")?;

        let redis_conn = ConnectionManager::new(client)
            .await
            .context("Failed to connect to Redis")?;

        let (local_sender, _) = broadcast::channel(1000);

        let instance_id = uuid::Uuid::new_v4().to_string();

        info!("Redis event bus connected, instance_id={}", instance_id);

        Ok(Self {
            local_sender,
            redis_conn,
            redis_url: redis_url.to_string(),
            instance_id,
        })
    }

    /// Create streams and consumer groups if they don't exist (idempotent).
    /// Must be called on startup before any stream operations.
    pub async fn ensure_streams(&self) -> Result<()> {
        let mut conn = self.redis_conn.clone();

        // Create job stream + worker consumer group
        match redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(REDIS_JOB_STREAM)
            .arg(REDIS_WORKER_GROUP)
            .arg("0")
            .arg("MKSTREAM")
            .query_async::<String>(&mut conn)
            .await
        {
            Ok(_) => info!(
                "Created consumer group {} on {}",
                REDIS_WORKER_GROUP, REDIS_JOB_STREAM
            ),
            Err(e) if e.to_string().contains("BUSYGROUP") => {
                debug!(
                    "Consumer group {} already exists on {}",
                    REDIS_WORKER_GROUP, REDIS_JOB_STREAM
                );
            }
            Err(e) => return Err(e).context("Failed to create job stream consumer group"),
        }

        // Create result stream + server consumer group
        match redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(REDIS_RESULT_STREAM)
            .arg(REDIS_SERVER_GROUP)
            .arg("0")
            .arg("MKSTREAM")
            .query_async::<String>(&mut conn)
            .await
        {
            Ok(_) => info!(
                "Created consumer group {} on {}",
                REDIS_SERVER_GROUP, REDIS_RESULT_STREAM
            ),
            Err(e) if e.to_string().contains("BUSYGROUP") => {
                debug!(
                    "Consumer group {} already exists on {}",
                    REDIS_SERVER_GROUP, REDIS_RESULT_STREAM
                );
            }
            Err(e) => return Err(e).context("Failed to create result stream consumer group"),
        }

        info!("Redis streams initialized");
        Ok(())
    }

    /// Subscribe to local events (within this process)
    pub fn subscribe(&self) -> broadcast::Receiver<Message> {
        self.local_sender.subscribe()
    }

    /// Publish a message, routing by category:
    /// - Result → XADD to result stream (durable)
    /// - Job → XADD to job stream (durable) — prefer enqueue_job() for clarity
    /// - Ephemeral → PUBLISH to pub/sub channel
    ///
    /// Always broadcasts locally for in-process subscribers.
    pub async fn publish(&self, message: Message) {
        // Always broadcast locally first
        let _ = self.local_sender.send(message.clone());

        match message.category() {
            MessageCategory::Result => {
                self.xadd_to_stream(REDIS_RESULT_STREAM, &message).await;
            }
            MessageCategory::Job => {
                self.xadd_to_stream(REDIS_JOB_STREAM, &message).await;
            }
            MessageCategory::Ephemeral => {
                self.publish_to_pubsub(&message).await;
            }
        }
    }

    /// Enqueue a worker-bound job into the job stream (XADD).
    /// Replaces the old LPUSH-based queue.
    pub async fn enqueue_job(&self, message: Message) {
        self.xadd_to_stream(REDIS_JOB_STREAM, &message).await;
    }

    /// XADD a message to a stream with approximate MAXLEN trimming.
    async fn xadd_to_stream(&self, stream: &str, message: &Message) {
        let json = match serde_json::to_string(message) {
            Ok(j) => j,
            Err(e) => {
                error!("Failed to serialize message for stream {}: {}", stream, e);
                return;
            }
        };

        let mut conn = self.redis_conn.clone();
        let result: redis::RedisResult<String> = redis::cmd("XADD")
            .arg(stream)
            .arg("MAXLEN")
            .arg("~")
            .arg(STREAM_MAXLEN)
            .arg("*")
            .arg("msg")
            .arg(&json)
            .query_async(&mut conn)
            .await;

        match result {
            Ok(id) => debug!("XADD {} → {}", stream, id),
            Err(e) => error!("XADD to {} failed: {}", stream, e),
        }
    }

    /// Publish an ephemeral message to Redis pub/sub with instance_id envelope.
    async fn publish_to_pubsub(&self, message: &Message) {
        let envelope = RedisMessageEnvelope {
            instance_id: self.instance_id.clone(),
            message: message.clone(),
        };

        let json = match serde_json::to_string(&envelope) {
            Ok(j) => j,
            Err(e) => {
                error!("Failed to serialize message for pub/sub: {}", e);
                return;
            }
        };

        let mut conn = self.redis_conn.clone();
        if let Err(e) = conn.publish::<_, _, ()>(REDIS_CHANNEL, &json).await {
            warn!("Failed to PUBLISH to {}: {}", REDIS_CHANNEL, e);
        } else {
            trace!("Published to pub/sub channel {}", REDIS_CHANNEL);
        }
    }

    /// Read new entries from a stream using XREADGROUP (non-blocking).
    ///
    /// Returns immediately with up to `count` new entries, or empty vec if none.
    /// Callers should sleep between polls to avoid busy-looping.
    ///
    /// **Why non-blocking**: `ConnectionManager` wraps `MultiplexedConnection`, which
    /// pipelines commands over a single TCP connection. `XREADGROUP BLOCK` holds the
    /// connection open, desynchronizing command/response pairing for subsequent commands
    /// (PEL reads return empty even when entries exist). Non-blocking read + app-level
    /// sleep avoids this entirely.
    pub async fn read_stream_entries(
        conn: &mut ConnectionManager,
        stream: &str,
        group: &str,
        consumer: &str,
        count: usize,
    ) -> Vec<(String, Message)> {
        let result: redis::RedisResult<redis::Value> = redis::cmd("XREADGROUP")
            .arg("GROUP")
            .arg(group)
            .arg(consumer)
            .arg("COUNT")
            .arg(count)
            .arg("STREAMS")
            .arg(stream)
            .arg(">")
            .query_async(conn)
            .await;

        match result {
            Ok(value) => parse_stream_response(&value),
            Err(e) => {
                let msg = e.to_string();
                // Nil responses are expected when no new entries exist
                if !msg.contains("nil") {
                    debug!("XREADGROUP on {}: {}", stream, msg);
                }
                vec![]
            }
        }
    }

    /// Read entries already in this consumer's Pending Entry List (PEL).
    ///
    /// Uses `0` instead of `>` — returns entries that were previously delivered
    /// to this consumer but not yet ACK'd (e.g., from prior runs or XAUTOCLAIM).
    /// No BLOCK — returns immediately. An empty result means the PEL is clear.
    pub async fn read_pending_entries(
        conn: &mut ConnectionManager,
        stream: &str,
        group: &str,
        consumer: &str,
        count: usize,
    ) -> Vec<(String, Message)> {
        let result: redis::RedisResult<redis::Value> = redis::cmd("XREADGROUP")
            .arg("GROUP")
            .arg(group)
            .arg(consumer)
            .arg("COUNT")
            .arg(count)
            .arg("STREAMS")
            .arg(stream)
            .arg("0")
            .query_async(conn)
            .await;

        match result {
            Ok(ref value) => parse_stream_response(value),
            Err(e) => {
                let msg = e.to_string();
                if !msg.contains("nil") {
                    warn!("XREADGROUP (PEL) error on {}: {}", stream, msg);
                }
                vec![]
            }
        }
    }

    /// ACK a stream entry (marks it as processed, removes from pending list).
    pub async fn ack_stream_entry(
        conn: &mut ConnectionManager,
        stream: &str,
        group: &str,
        stream_id: &str,
    ) {
        let result: redis::RedisResult<i64> = redis::cmd("XACK")
            .arg(stream)
            .arg(group)
            .arg(stream_id)
            .query_async(conn)
            .await;

        match result {
            Ok(n) => trace!("XACK {} {} → {} entries", stream, stream_id, n),
            Err(e) => warn!("XACK {} {} failed: {}", stream, stream_id, e),
        }
    }

    /// Reclaim stale messages from crashed consumers via XAUTOCLAIM.
    /// Returns reclaimed entries as (stream_id, Message).
    pub async fn autoclaim_stale(
        conn: &mut ConnectionManager,
        stream: &str,
        group: &str,
        consumer: &str,
        min_idle_ms: usize,
    ) -> Vec<(String, Message)> {
        // XAUTOCLAIM <stream> <group> <consumer> <min-idle-ms> <start> COUNT 10
        let result: redis::RedisResult<redis::Value> = redis::cmd("XAUTOCLAIM")
            .arg(stream)
            .arg(group)
            .arg(consumer)
            .arg(min_idle_ms)
            .arg("0-0")
            .arg("COUNT")
            .arg(10)
            .query_async(conn)
            .await;

        match result {
            Ok(value) => parse_autoclaim_response(&value),
            Err(e) => {
                let msg = e.to_string();
                if !msg.contains("nil") && !msg.contains("timed out") && !msg.contains("timeout") {
                    debug!("XAUTOCLAIM on {}: {}", stream, msg);
                }
                vec![]
            }
        }
    }

    /// Start the pub/sub subscriber loop for ephemeral messages.
    ///
    /// This should be spawned as a background task. It subscribes to the Redis
    /// channel and forwards received messages to the local broadcast channel.
    pub async fn start_subscriber(self: std::sync::Arc<Self>) -> Result<()> {
        // RESP3 is required for push-based pub/sub on multiplexed connections
        let resp3_url = if self.redis_url.contains('?') {
            format!("{}&protocol=resp3", self.redis_url)
        } else {
            format!("{}?protocol=resp3", self.redis_url)
        };
        let client = redis::Client::open(resp3_url.as_str())
            .context("Failed to create Redis client for subscriber")?;

        let (tx, mut rx) = mpsc::unbounded_channel();
        let config = redis::AsyncConnectionConfig::new().set_push_sender(tx);

        let mut conn = client
            .get_multiplexed_async_connection_with_config(&config)
            .await
            .context("Failed to create Redis connection")?;

        conn.subscribe(REDIS_CHANNEL)
            .await
            .context("Failed to subscribe to Redis channel")?;

        info!(
            "Redis pub/sub subscriber started on channel {}",
            REDIS_CHANNEL
        );

        while let Some(push_info) = rx.recv().await {
            if push_info.kind != redis::PushKind::Message {
                continue;
            }
            let payload: String = match push_info.data.get(1) {
                Some(val) => match redis::FromRedisValue::from_redis_value(val.clone()) {
                    Ok(p) => p,
                    Err(e) => {
                        error!("Failed to decode Redis message payload: {}", e);
                        continue;
                    }
                },
                None => continue,
            };

            let envelope: RedisMessageEnvelope = match serde_json::from_str(&payload) {
                Ok(e) => e,
                Err(e) => {
                    error!("Failed to deserialize Redis message: {}", e);
                    continue;
                }
            };

            // Skip messages from this instance (prevent echo)
            if envelope.instance_id == self.instance_id {
                continue;
            }

            trace!(
                "Received pub/sub message from instance {}, forwarding locally",
                envelope.instance_id
            );
            let _ = self.local_sender.send(envelope.message);
        }

        warn!("Redis pub/sub subscriber loop ended");
        Ok(())
    }

    /// Start a result stream reader that feeds results into the local broadcast.
    ///
    /// This should be spawned as a background task on the SERVER side only.
    /// Workers don't need to read from the result stream.
    pub async fn start_result_stream_reader(self: std::sync::Arc<Self>) -> Result<()> {
        let client = redis::Client::open(self.redis_url.as_str())
            .context("Failed to create Redis client for result stream reader")?;
        let mut conn = ConnectionManager::new(client)
            .await
            .context("Failed to connect to Redis for result stream reader")?;

        // Fixed consumer ID — only one server reads results. Deterministic so
        // PEL entries survive restarts (same fix as worker deterministic IDs).
        let consumer_id = "server-main".to_string();
        info!(
            "Result stream reader started (consumer={}, stream={})",
            consumer_id, REDIS_RESULT_STREAM
        );

        // First: drain any entries left in our PEL from a prior server instance
        loop {
            let pending = Self::read_pending_entries(
                &mut conn,
                REDIS_RESULT_STREAM,
                REDIS_SERVER_GROUP,
                &consumer_id,
                10,
            )
            .await;

            if pending.is_empty() {
                break;
            }

            info!("Draining {} pending result(s) from PEL", pending.len());
            for (stream_id, message) in pending {
                let _ = self.local_sender.send(message);
                Self::ack_stream_entry(
                    &mut conn,
                    REDIS_RESULT_STREAM,
                    REDIS_SERVER_GROUP,
                    &stream_id,
                )
                .await;
            }
        }

        // Main loop: poll for new entries + check PEL
        loop {
            // Check PEL first (entries from XAUTOCLAIM or re-delivery)
            let pending = Self::read_pending_entries(
                &mut conn,
                REDIS_RESULT_STREAM,
                REDIS_SERVER_GROUP,
                &consumer_id,
                10,
            )
            .await;

            if !pending.is_empty() {
                debug!("Result stream: processing {} PEL entries", pending.len());
                for (stream_id, message) in pending {
                    debug!(
                        "Result stream → local broadcast: {} ({})",
                        stream_id,
                        message_type_label(&message)
                    );
                    let _ = self.local_sender.send(message);
                    Self::ack_stream_entry(
                        &mut conn,
                        REDIS_RESULT_STREAM,
                        REDIS_SERVER_GROUP,
                        &stream_id,
                    )
                    .await;
                }
                // Don't sleep — check for more entries immediately
                continue;
            }

            // PEL empty — poll for new entries (non-blocking)
            let new_entries = Self::read_stream_entries(
                &mut conn,
                REDIS_RESULT_STREAM,
                REDIS_SERVER_GROUP,
                &consumer_id,
                10,
            )
            .await;

            if !new_entries.is_empty() {
                debug!("Result stream: received {} new entries", new_entries.len());
                for (stream_id, message) in new_entries {
                    debug!(
                        "Result stream → local broadcast: {} ({})",
                        stream_id,
                        message_type_label(&message)
                    );
                    let _ = self.local_sender.send(message);
                    Self::ack_stream_entry(
                        &mut conn,
                        REDIS_RESULT_STREAM,
                        REDIS_SERVER_GROUP,
                        &stream_id,
                    )
                    .await;
                }
                // Don't sleep — check for more entries immediately
                continue;
            }

            // Nothing to process — sleep before next poll
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    }

    /// Get the Redis URL (for creating dedicated connections in workers)
    pub fn redis_url(&self) -> &str {
        &self.redis_url
    }
}

/// Parse XREADGROUP / XRANGE response into (stream_id, Message) pairs.
///
/// XREADGROUP returns:
/// ```text
/// [[stream_name, [[entry_id, [field, value, ...]], ...]]]
/// ```
#[cfg(feature = "redis-events")]
fn parse_stream_response(value: &redis::Value) -> Vec<(String, Message)> {
    let mut entries = Vec::new();

    // Response is an array of [stream_key, entries]
    let streams = match value {
        redis::Value::Array(arr) => arr,
        redis::Value::Nil => return entries,
        other => {
            debug!("Unexpected XREADGROUP response type: {:?}", other);
            return entries;
        }
    };

    for stream_data in streams {
        let stream_arr = match stream_data {
            redis::Value::Array(arr) if arr.len() >= 2 => arr,
            _ => continue,
        };

        // stream_arr[1] is the array of entries
        let entry_list = match &stream_arr[1] {
            redis::Value::Array(arr) => arr,
            _ => continue,
        };

        for entry in entry_list {
            if let Some((id, msg)) = parse_stream_entry(entry) {
                entries.push((id, msg));
            }
        }
    }

    entries
}

/// Parse a single stream entry: [entry_id, [field, value, ...]]
#[cfg(feature = "redis-events")]
fn parse_stream_entry(entry: &redis::Value) -> Option<(String, Message)> {
    let arr = match entry {
        redis::Value::Array(arr) if arr.len() >= 2 => arr,
        _ => return None,
    };

    // Entry ID (clone needed — from_redis_value takes owned Value in redis 1.0)
    let id: String = match redis::FromRedisValue::from_redis_value(arr[0].clone()) {
        Ok(id) => id,
        Err(_) => return None,
    };

    // Fields array: [field, value, field, value, ...]
    let fields = match &arr[1] {
        redis::Value::Array(arr) => arr,
        _ => return None,
    };

    // Find the "msg" field
    let mut i = 0;
    while i + 1 < fields.len() {
        let field: String = match redis::FromRedisValue::from_redis_value(fields[i].clone()) {
            Ok(f) => f,
            Err(_) => return None,
        };
        if field == "msg" {
            let json: String = match redis::FromRedisValue::from_redis_value(fields[i + 1].clone())
            {
                Ok(j) => j,
                Err(_) => return None,
            };
            return match serde_json::from_str::<Message>(&json) {
                Ok(msg) => Some((id, msg)),
                Err(e) => {
                    error!("Failed to deserialize stream entry {}: {}", id, e);
                    None
                }
            };
        }
        i += 2;
    }
    None
}

/// Parse XAUTOCLAIM response: [cursor, [[entry_id, [field, value, ...]], ...], [deleted_ids]]
#[cfg(feature = "redis-events")]
fn parse_autoclaim_response(value: &redis::Value) -> Vec<(String, Message)> {
    let arr = match value {
        redis::Value::Array(arr) if arr.len() >= 2 => arr,
        _ => return vec![],
    };

    // arr[1] is the array of claimed entries
    let entry_list = match &arr[1] {
        redis::Value::Array(arr) => arr,
        _ => return vec![],
    };

    let mut entries = Vec::new();
    for entry in entry_list {
        if let Some((id, msg)) = parse_stream_entry(entry) {
            entries.push((id, msg));
        }
    }

    entries
}

/// Quick label for log messages
#[cfg(feature = "redis-events")]
fn message_type_label(msg: &Message) -> &'static str {
    match msg {
        Message::ScanResult { .. } => "ScanResult",
        Message::ProbeFileResult { .. } => "ProbeFileResult",
        Message::HashFileResult { .. } => "HashFileResult",
        Message::ImportFilesResult { .. } => "ImportFilesResult",
        Message::DeletePathsResult { .. } => "DeletePathsResult",
        _ => "Other",
    }
}

/// Wrapper for messages sent through Redis pub/sub
///
/// Includes the instance ID to prevent echo (receiving our own messages)
#[cfg(feature = "redis-events")]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct RedisMessageEnvelope {
    instance_id: String,
    message: Message,
}

#[cfg(feature = "redis-events")]
impl Clone for RedisEventBus {
    fn clone(&self) -> Self {
        Self {
            local_sender: self.local_sender.clone(),
            redis_conn: self.redis_conn.clone(),
            redis_url: self.redis_url.clone(),
            instance_id: self.instance_id.clone(),
        }
    }
}

// ============================================================================
// Unified EventBus that works with or without Redis
// ============================================================================

use crate::core::messaging::Message as LocalMessage;
use tokio::sync::broadcast as tokio_broadcast;

/// Event bus backend type
#[derive(Debug, Clone)]
pub enum EventBusBackend {
    /// In-process only (single container)
    InProcess,
    /// Redis-backed (multi-container)
    #[cfg(feature = "redis-events")]
    Redis(std::sync::Arc<RedisEventBus>),
}

/// Unified event bus that can work with either backend
#[derive(Debug, Clone)]
pub struct HybridEventBus {
    backend: EventBusBackend,
    /// Fallback local sender when Redis isn't configured
    local_sender: tokio_broadcast::Sender<LocalMessage>,
}

impl HybridEventBus {
    /// Create a new in-process event bus (no Redis)
    pub fn new_in_process() -> Self {
        let (local_sender, _) = tokio_broadcast::channel(1000);
        Self {
            backend: EventBusBackend::InProcess,
            local_sender,
        }
    }

    /// Create a new Redis-backed event bus
    #[cfg(feature = "redis-events")]
    pub async fn new_redis(redis_url: &str) -> anyhow::Result<Self> {
        let redis_bus = RedisEventBus::new(redis_url).await?;
        let (local_sender, _) = tokio_broadcast::channel(1000);

        Ok(Self {
            backend: EventBusBackend::Redis(std::sync::Arc::new(redis_bus)),
            local_sender,
        })
    }

    /// Subscribe to events
    pub fn subscribe(&self) -> tokio_broadcast::Receiver<LocalMessage> {
        match &self.backend {
            EventBusBackend::InProcess => self.local_sender.subscribe(),
            #[cfg(feature = "redis-events")]
            EventBusBackend::Redis(redis_bus) => redis_bus.subscribe(),
        }
    }

    /// Publish an event (routed by message category)
    pub async fn publish(&self, message: LocalMessage) {
        match &self.backend {
            EventBusBackend::InProcess => {
                let _ = self.local_sender.send(message);
            }
            #[cfg(feature = "redis-events")]
            EventBusBackend::Redis(redis_bus) => {
                redis_bus.publish(message).await;
            }
        }
    }

    /// Check if Redis is enabled
    pub fn is_redis_enabled(&self) -> bool {
        match &self.backend {
            EventBusBackend::InProcess => false,
            #[cfg(feature = "redis-events")]
            EventBusBackend::Redis(_) => true,
        }
    }

    /// Enqueue a worker-bound job into the job stream (XADD).
    /// Falls back to local broadcast for in-process mode.
    pub async fn enqueue_job(&self, message: LocalMessage) {
        match &self.backend {
            EventBusBackend::InProcess => {
                let _ = self.local_sender.send(message);
            }
            #[cfg(feature = "redis-events")]
            EventBusBackend::Redis(redis_bus) => {
                redis_bus.enqueue_job(message).await;
            }
        }
    }

    /// Create streams and consumer groups (idempotent).
    #[cfg(feature = "redis-events")]
    pub async fn ensure_streams(&self) -> anyhow::Result<()> {
        if let EventBusBackend::Redis(redis_bus) = &self.backend {
            redis_bus.ensure_streams().await?;
        }
        Ok(())
    }

    /// Start the pub/sub subscriber (only does something if Redis is configured)
    #[cfg(feature = "redis-events")]
    pub async fn start_redis_subscriber(&self) -> anyhow::Result<()> {
        if let EventBusBackend::Redis(redis_bus) = &self.backend {
            redis_bus.clone().start_subscriber().await?;
        }
        Ok(())
    }

    /// Start the result stream reader on the server side.
    /// Reads results from the result stream and forwards to local broadcast.
    #[cfg(feature = "redis-events")]
    pub async fn start_result_stream_reader(&self) -> anyhow::Result<()> {
        if let EventBusBackend::Redis(redis_bus) = &self.backend {
            redis_bus.clone().start_result_stream_reader().await?;
        }
        Ok(())
    }

    /// Get the Redis URL (for workers that need dedicated stream connections)
    #[cfg(feature = "redis-events")]
    pub fn redis_url(&self) -> Option<&str> {
        match &self.backend {
            EventBusBackend::InProcess => None,
            EventBusBackend::Redis(redis_bus) => Some(redis_bus.redis_url()),
        }
    }
}

impl Default for HybridEventBus {
    fn default() -> Self {
        Self::new_in_process()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_process_bus() {
        let bus = HybridEventBus::new_in_process();
        assert!(!bus.is_redis_enabled());
    }
}
