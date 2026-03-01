//! Redis-backed event bus for distributed deployments
//!
//! This module provides a Redis pub/sub implementation of the event bus
//! that enables cross-container messaging in multi-instance deployments.

#[cfg(feature = "redis-events")]
use anyhow::{Context, Result};
#[cfg(feature = "redis-events")]
use futures_util::StreamExt;
#[cfg(feature = "redis-events")]
use redis::aio::ConnectionManager;
#[cfg(feature = "redis-events")]
use redis::AsyncCommands;
#[cfg(feature = "redis-events")]
use tokio::sync::broadcast;
#[cfg(feature = "redis-events")]
use tracing::{debug, error, info, trace, warn};

#[cfg(feature = "redis-events")]
use crate::core::messaging::Message;

/// Redis channel name for pir9 events
#[cfg(feature = "redis-events")]
const REDIS_CHANNEL: &str = "pir9:events";

/// Redis-backed event bus
///
/// This event bus publishes messages to both a local broadcast channel
/// and a Redis pub/sub channel, enabling cross-container communication.
#[cfg(feature = "redis-events")]
pub struct RedisEventBus {
    /// Local broadcast sender for in-process subscribers
    local_sender: broadcast::Sender<Message>,
    /// Redis connection manager for publishing
    redis_conn: ConnectionManager,
    /// Redis URL for creating pubsub connections
    redis_url: String,
    /// Instance ID to prevent echo (receiving our own messages)
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

        // Generate a unique instance ID
        let instance_id = uuid::Uuid::new_v4().to_string();

        info!("Redis event bus connected, instance_id={}", instance_id);

        Ok(Self {
            local_sender,
            redis_conn,
            redis_url: redis_url.to_string(),
            instance_id,
        })
    }

    /// Subscribe to local events (within this process)
    pub fn subscribe(&self) -> broadcast::Receiver<Message> {
        self.local_sender.subscribe()
    }

    /// Publish an event to both local and Redis channels
    pub async fn publish(&self, message: Message) {
        // Publish locally first
        let _ = self.local_sender.send(message.clone());

        // Wrap message with instance ID for Redis
        let envelope = RedisMessageEnvelope {
            instance_id: self.instance_id.clone(),
            message,
        };

        // Publish to Redis (fire and forget, with logging on error)
        let mut conn = self.redis_conn.clone();
        let json = match serde_json::to_string(&envelope) {
            Ok(j) => j,
            Err(e) => {
                error!("Failed to serialize message for Redis: {}", e);
                return;
            }
        };

        if let Err(e) = conn.publish::<_, _, ()>(REDIS_CHANNEL, &json).await {
            warn!("Failed to publish to Redis: {}", e);
        } else {
            trace!("Published message to Redis channel {}", REDIS_CHANNEL);
        }
    }

    /// Start the Redis subscriber loop
    ///
    /// This should be spawned as a background task. It subscribes to the Redis
    /// channel and forwards received messages to the local broadcast channel.
    pub async fn start_subscriber(self: std::sync::Arc<Self>) -> Result<()> {
        let client = redis::Client::open(self.redis_url.as_str())
            .context("Failed to create Redis client for subscriber")?;

        // Get async connection and convert to pubsub
        // Note: get_async_connection is deprecated in favor of get_multiplexed_async_connection,
        // but pubsub requires a dedicated connection that supports into_pubsub().
        #[allow(deprecated)]
        let conn = client
            .get_async_connection()
            .await
            .context("Failed to create Redis connection")?;
        let mut pubsub_conn = conn.into_pubsub();

        pubsub_conn
            .subscribe(REDIS_CHANNEL)
            .await
            .context("Failed to subscribe to Redis channel")?;

        info!("Redis subscriber started on channel {}", REDIS_CHANNEL);

        let mut msg_stream = pubsub_conn.on_message();

        while let Some(msg) = msg_stream.next().await {
            let payload: String = match msg.get_payload() {
                Ok(p) => p,
                Err(e) => {
                    error!("Failed to get Redis message payload: {}", e);
                    continue;
                }
            };

            // Deserialize the envelope
            let envelope: RedisMessageEnvelope = match serde_json::from_str(&payload) {
                Ok(e) => e,
                Err(e) => {
                    error!("Failed to deserialize Redis message: {}", e);
                    continue;
                }
            };

            // Skip messages from this instance (prevent echo)
            if envelope.instance_id == self.instance_id {
//                debug!("Skipping message from self");
                continue;
            }

            // Forward to local subscribers
            trace!(
                "Received message from instance {}, forwarding locally",
                envelope.instance_id
            );
            let _ = self.local_sender.send(envelope.message);
        }

        warn!("Redis subscriber loop ended");
        Ok(())
    }
}

/// Wrapper for messages sent through Redis
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

    /// Publish an event
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

    /// Start the Redis subscriber (only does something if Redis is configured)
    #[cfg(feature = "redis-events")]
    pub async fn start_redis_subscriber(&self) -> anyhow::Result<()> {
        if let EventBusBackend::Redis(redis_bus) = &self.backend {
            redis_bus.clone().start_subscriber().await?;
        }
        Ok(())
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
