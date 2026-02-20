#![allow(dead_code)]
//! Worker registry for tracking distributed scan workers
//!
//! The registry maintains a list of online workers, their paths, and health status.
//! Workers send periodic heartbeats; workers that miss heartbeats are marked as unhealthy.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::core::messaging::{HybridEventBus, Message};

/// How often workers should send heartbeats
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);

/// How long before a worker is considered dead (3 missed heartbeats)
pub const WORKER_TIMEOUT: Duration = Duration::from_secs(100);

/// Information about a registered worker
#[derive(Debug, Clone)]
pub struct WorkerInfo {
    /// Worker's unique ID
    pub worker_id: String,
    /// Paths this worker can scan
    pub paths: Vec<String>,
    /// When the worker came online
    pub online_since: Instant,
    /// Last heartbeat received
    pub last_heartbeat: Instant,
    /// Number of scans completed
    pub scans_completed: u64,
    /// Total files found
    pub files_found: u64,
    /// Worker uptime in seconds (from worker's perspective)
    pub uptime_seconds: u64,
    /// Whether the worker is considered healthy
    pub healthy: bool,
}

impl WorkerInfo {
    /// Create a new worker info from an online message
    pub fn new(worker_id: String, paths: Vec<String>) -> Self {
        let now = Instant::now();
        Self {
            worker_id,
            paths,
            online_since: now,
            last_heartbeat: now,
            scans_completed: 0,
            files_found: 0,
            uptime_seconds: 0,
            healthy: true,
        }
    }

    /// Update from a heartbeat
    pub fn update_heartbeat(&mut self, scans_completed: u64, files_found: u64, uptime_seconds: u64) {
        self.last_heartbeat = Instant::now();
        self.scans_completed = scans_completed;
        self.files_found = files_found;
        self.uptime_seconds = uptime_seconds;
        self.healthy = true;
    }

    /// Check if the worker has timed out
    pub fn is_timed_out(&self) -> bool {
        self.last_heartbeat.elapsed() > WORKER_TIMEOUT
    }

    /// Get time since last heartbeat
    pub fn time_since_heartbeat(&self) -> Duration {
        self.last_heartbeat.elapsed()
    }
}

/// Registry of all known workers
#[derive(Debug, Default)]
pub struct WorkerRegistry {
    workers: HashMap<String, WorkerInfo>,
}

impl WorkerRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            workers: HashMap::new(),
        }
    }

    /// Register a worker as online
    pub fn register(&mut self, worker_id: &str, paths: Vec<String>) {
        info!("Worker registered: {} with paths: {:?}", worker_id, paths);
        self.workers.insert(
            worker_id.to_string(),
            WorkerInfo::new(worker_id.to_string(), paths),
        );
    }

    /// Unregister a worker (went offline)
    pub fn unregister(&mut self, worker_id: &str) {
        if self.workers.remove(worker_id).is_some() {
            info!("Worker unregistered: {}", worker_id);
        }
    }

    /// Update a worker's heartbeat
    pub fn heartbeat(&mut self, worker_id: &str, paths: Vec<String>, scans_completed: u64, files_found: u64, uptime_seconds: u64) {
        if let Some(worker) = self.workers.get_mut(worker_id) {
            worker.update_heartbeat(scans_completed, files_found, uptime_seconds);
            // Update paths in case they changed
            worker.paths = paths;
            debug!("Heartbeat from worker {}: scans={}, files={}", worker_id, scans_completed, files_found);
        } else {
            // Worker wasn't registered, register now
            debug!("Heartbeat from unknown worker {}, registering", worker_id);
            let mut info = WorkerInfo::new(worker_id.to_string(), paths);
            info.update_heartbeat(scans_completed, files_found, uptime_seconds);
            self.workers.insert(worker_id.to_string(), info);
        }
    }

    /// Check for timed out workers and mark them unhealthy
    pub fn check_health(&mut self) -> Vec<String> {
        let mut timed_out = Vec::new();

        for (worker_id, worker) in self.workers.iter_mut() {
            if worker.is_timed_out() {
                if worker.healthy {
                    warn!("Worker {} has timed out (last heartbeat: {:?} ago)",
                          worker_id, worker.time_since_heartbeat());
                    worker.healthy = false;
                    timed_out.push(worker_id.clone());
                }
            }
        }

        timed_out
    }

    /// Get all online workers
    pub fn get_all(&self) -> Vec<&WorkerInfo> {
        self.workers.values().collect()
    }

    /// Get only healthy workers
    pub fn get_healthy(&self) -> Vec<&WorkerInfo> {
        self.workers.values().filter(|w| w.healthy).collect()
    }

    /// Get workers that can handle a specific path
    pub fn get_workers_for_path(&self, path: &str) -> Vec<&WorkerInfo> {
        self.workers.values()
            .filter(|w| w.healthy && w.paths.iter().any(|p| path.starts_with(p) || p.starts_with(path)))
            .collect()
    }

    /// Check if there are any healthy workers
    pub fn has_healthy_workers(&self) -> bool {
        self.workers.values().any(|w| w.healthy)
    }

    /// Get count of workers
    pub fn count(&self) -> usize {
        self.workers.len()
    }

    /// Get count of healthy workers
    pub fn healthy_count(&self) -> usize {
        self.workers.values().filter(|w| w.healthy).count()
    }
}

/// Service that manages the worker registry
pub struct WorkerRegistryService {
    registry: Arc<RwLock<WorkerRegistry>>,
    event_bus: HybridEventBus,
}

impl WorkerRegistryService {
    /// Create a new worker registry service
    pub fn new(event_bus: HybridEventBus) -> Self {
        Self {
            registry: Arc::new(RwLock::new(WorkerRegistry::new())),
            event_bus,
        }
    }

    /// Get a reference to the registry
    pub fn registry(&self) -> Arc<RwLock<WorkerRegistry>> {
        self.registry.clone()
    }

    /// Start the registry service
    ///
    /// This subscribes to worker events and runs a health check loop.
    pub async fn run(self: Arc<Self>) -> anyhow::Result<()> {
        info!("Starting worker registry service");

        let registry = self.registry.clone();
        let mut receiver = self.event_bus.subscribe();

        // Spawn health check task
        let health_registry = registry.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                let mut reg = health_registry.write().await;
                let timed_out = reg.check_health();
                if !timed_out.is_empty() {
                    warn!("Workers timed out: {:?}", timed_out);
                }
                drop(reg);
            }
        });

        // Process worker events
        loop {
            match receiver.recv().await {
                Ok(message) => {
                    match message {
                        Message::WorkerOnline { worker_id, paths } => {
                            let mut reg = registry.write().await;
                            reg.register(&worker_id, paths);
                        }
                        Message::WorkerOffline { worker_id } => {
                            let mut reg = registry.write().await;
                            reg.unregister(&worker_id);
                        }
                        Message::WorkerHeartbeat { worker_id, paths, scans_completed, files_found, uptime_seconds } => {
                            let mut reg = registry.write().await;
                            reg.heartbeat(&worker_id, paths, scans_completed, files_found, uptime_seconds);
                        }
                        _ => {}
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    warn!("Worker registry lagged by {} messages", n);
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    info!("Event bus closed, shutting down worker registry");
                    break;
                }
            }
        }

        Ok(())
    }
}

/// API response for worker status
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerStatus {
    pub worker_id: String,
    pub paths: Vec<String>,
    pub healthy: bool,
    pub scans_completed: u64,
    pub files_found: u64,
    pub uptime_seconds: u64,
    pub seconds_since_heartbeat: u64,
}

impl From<&WorkerInfo> for WorkerStatus {
    fn from(info: &WorkerInfo) -> Self {
        Self {
            worker_id: info.worker_id.clone(),
            paths: info.paths.clone(),
            healthy: info.healthy,
            scans_completed: info.scans_completed,
            files_found: info.files_found,
            uptime_seconds: info.uptime_seconds,
            seconds_since_heartbeat: info.time_since_heartbeat().as_secs(),
        }
    }
}
