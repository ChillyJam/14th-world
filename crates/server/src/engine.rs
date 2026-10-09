use std::time::Duration;

use std::sync::{Arc, Mutex};

use protocol::{EventLog, ServerMsg, WorldView};
use sim::{Event, World};
use tokio::sync::watch;
use tokio::time::{interval, MissedTickBehavior};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::config::Config;
use crate::db::Db;

/// Owns the world: steps it on a fixed clock, publishes each frame to
/// connected clients and periodically persists it. Returns after a final save
/// once `shutdown` is cancelled.
pub async fn run(
    mut world: World,
    db: Db,
    config: Config,
    frames: watch::Sender<String>,
    log: Arc<Mutex<EventLog>>,
    shutdown: CancellationToken,
) -> anyhow::Result<()> {
    let mut ticker = interval(Duration::from_secs_f64(1.0 / config.tick_rate_hz));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut snapshot_timer = interval(config.snapshot_interval);
    snapshot_timer.tick().await; // the first tick fires immediately

    let mut pending: Vec<(u64, Event)> = Vec::new();

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => break,
            _ = ticker.tick() => {
                let events = world.step();
                for event in events.iter().cloned() {
                    match &event {
                        Event::EraReached { era } => {
                            info!(tick = world.time.tick, era = era.name(), "new era reached");
                        }
                        Event::Died { person, cause } => {
                            info!(tick = world.time.tick, person, ?cause, "someone died");
                        }
                        Event::Met { .. } | Event::Born { .. } | Event::FocusChanged { .. } => {}
                    }
                    pending.push((world.time.tick, event));
                }
                // Record before publishing, so a frame never announces log
                // entries that sockets cannot read yet.
                let mut log = log.lock().expect("log lock poisoned");
                log.record(&world, &events);
                frames.send_replace(frame(&world, log.next_seq()));
            }
            _ = snapshot_timer.tick() => persist(&db, &world, &mut pending, &config).await,
        }
    }

    // On shutdown a failed save is fatal, so it gets surfaced.
    db.save(&world, &pending, config.snapshots_to_keep).await?;
    info!(tick = world.time.tick, "world saved, simulation stopped");
    Ok(())
}

pub fn frame(world: &World, log_seq: u64) -> String {
    protocol::encode(&ServerMsg::Frame(WorldView::new(world, log_seq)))
}

/// A failed periodic save must not kill a long-running world: the events stay
/// pending and the next interval retries.
async fn persist(db: &Db, world: &World, pending: &mut Vec<(u64, Event)>, config: &Config) {
    match db.save(world, pending, config.snapshots_to_keep).await {
        Ok(()) => pending.clear(),
        Err(err) => error!(error = %err, "saving world failed; will retry"),
    }
}
