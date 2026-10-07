use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{ensure, Context, Result};

/// Runtime settings, read from environment variables (see `.env.example`).
#[derive(Clone, Debug)]
pub struct Config {
    pub bind: SocketAddr,
    pub database_url: String,
    pub static_dir: PathBuf,
    /// Simulation ticks per real second. One tick is one in-world minute.
    pub tick_rate_hz: f64,
    pub snapshot_interval: Duration,
    pub snapshots_to_keep: u32,
    /// Only used when creating a brand-new world.
    pub seed: u64,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let config = Self {
            bind: var("BIND_ADDR", SocketAddr::from(([0, 0, 0, 0], 8080)))?,
            database_url: var("DATABASE_URL", "sqlite://data/world.db".to_string())?,
            static_dir: var("STATIC_DIR", PathBuf::from("crates/client/dist"))?,
            tick_rate_hz: var("TICK_RATE_HZ", 10.0)?,
            snapshot_interval: Duration::from_secs(var("SNAPSHOT_INTERVAL_SECS", 60u64)?),
            snapshots_to_keep: var("SNAPSHOTS_TO_KEEP", 24u32)?,
            seed: match env::var("WORLD_SEED") {
                Ok(s) => s
                    .parse()
                    .context("WORLD_SEED must be an unsigned integer")?,
                Err(_) => SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_nanos() as u64)
                    .unwrap_or_default(),
            },
        };
        ensure!(
            config.tick_rate_hz > 0.0 && config.tick_rate_hz.is_finite(),
            "TICK_RATE_HZ must be positive"
        );
        ensure!(
            !config.snapshot_interval.is_zero(),
            "SNAPSHOT_INTERVAL_SECS must be positive"
        );
        ensure!(
            config.snapshots_to_keep > 0,
            "SNAPSHOTS_TO_KEEP must be at least 1"
        );
        Ok(config)
    }
}

fn var<T>(name: &str, default: T) -> Result<T>
where
    T: FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    match env::var(name) {
        Ok(raw) => raw
            .parse()
            .with_context(|| format!("invalid value for {name}: {raw:?}")),
        Err(_) => Ok(default),
    }
}
