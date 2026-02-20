//! Command-line argument parsing for Pir9
//!
//! Supports different run modes for distributed deployments:
//! - `all`: Full application (web server + scheduler + local scanning) - default
//! - `server`: Web server + scheduler, publishes scan requests to Redis
//! - `worker`: Scan worker only, subscribes to Redis for scan requests

use clap::{Parser, ValueEnum};

/// Pir9 - Smart PVR for TV and anime
#[derive(Parser, Debug)]
#[command(name = "pir9")]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Run mode for the application
    #[arg(long, value_enum, default_value_t = RunMode::All)]
    pub mode: RunMode,

    /// Redis URL for distributed mode (required for server/worker modes)
    #[arg(long, env = "PIR9_REDIS_URL")]
    pub redis_url: Option<String>,

    /// Paths this worker should handle (worker mode only)
    /// Can be specified multiple times: --worker-path /media/tv --worker-path /media/anime
    #[arg(long = "worker-path", env = "PIR9_WORKER_PATHS")]
    pub worker_paths: Vec<String>,

    /// Worker ID (auto-generated if not specified)
    #[arg(long, env = "PIR9_WORKER_ID")]
    pub worker_id: Option<String>,

    /// Server port (overrides config file)
    #[arg(short, long, env = "PIR9_PORT")]
    pub port: Option<u16>,
}

/// Application run mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RunMode {
    /// Full application: web server + scheduler + local scanning (default)
    All,
    /// Server mode: web server + scheduler, uses Redis for distributed scanning
    Server,
    /// Worker mode: scan worker only, connects to Redis for commands
    Worker,
}

impl Args {
    /// Validate the arguments based on the run mode
    pub fn validate(&self) -> Result<(), String> {
        match self.mode {
            RunMode::All => {
                // All mode works with or without Redis
                Ok(())
            }
            RunMode::Server => {
                if self.redis_url.is_none() {
                    return Err("--redis-url is required for server mode".to_string());
                }
                Ok(())
            }
            RunMode::Worker => {
                if self.redis_url.is_none() {
                    return Err("--redis-url is required for worker mode".to_string());
                }
                if self.worker_paths.is_empty() {
                    return Err("--worker-path is required for worker mode (specify paths this worker should scan)".to_string());
                }
                Ok(())
            }
        }
    }

    /// Check if this mode requires Redis
    pub fn requires_redis(&self) -> bool {
        matches!(self.mode, RunMode::Server | RunMode::Worker)
    }

    /// Check if this mode should run the web server
    pub fn should_run_web_server(&self) -> bool {
        matches!(self.mode, RunMode::All | RunMode::Server)
    }

    /// Check if this mode should run the scheduler
    pub fn should_run_scheduler(&self) -> bool {
        matches!(self.mode, RunMode::All | RunMode::Server)
    }

    /// Check if this mode should run as a scan worker
    pub fn should_run_worker(&self) -> bool {
        matches!(self.mode, RunMode::Worker)
    }

    /// Check if scanning should be done locally (vs distributed via Redis)
    pub fn should_scan_locally(&self) -> bool {
        matches!(self.mode, RunMode::All)
    }
}
