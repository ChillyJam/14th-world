mod config;
mod db;
mod engine;
mod web;

use anyhow::Context;
use protocol::{Catalog, ServerMsg, PROTOCOL_VERSION};
use sim::{World, WorldConfig};
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::config::Config;
use crate::db::Db;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,sqlx=warn".into()),
        )
        .init();

    let config = Config::from_env()?;
    let db = Db::connect(&config.database_url).await?;

    let world = match db.load_latest_world().await? {
        Some(world) => {
            info!(
                tick = world.time.tick,
                year = world.time.year(),
                era = world.era.name(),
                "resuming saved world"
            );
            world
        }
        None => {
            info!(seed = config.seed, "creating a new world");
            let world = World::new(config.seed, WorldConfig::default());
            db.save(&world, &[], config.snapshots_to_keep).await?;
            world
        }
    };

    let welcome = protocol::encode(&ServerMsg::Welcome {
        protocol_version: PROTOCOL_VERSION,
        world_width: world.config.width,
        world_height: world.config.height,
        catalog: Catalog::new(),
    });
    let (frames_tx, frames_rx) = watch::channel(engine::frame(&world));
    let shutdown = CancellationToken::new();

    let engine = tokio::spawn(engine::run(
        world,
        db,
        config.clone(),
        frames_tx,
        shutdown.clone(),
    ));

    let app = web::router(web::AppState {
        welcome,
        frames: frames_rx,
        shutdown: shutdown.clone(),
    });
    let listener = TcpListener::bind(config.bind)
        .await
        .with_context(|| format!("binding {}", config.bind))?;
    info!(addr = %config.bind, "listening");

    let signal = shutdown.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            wait_for_signal().await;
            info!("shutting down");
            signal.cancel();
        })
        .await?;

    shutdown.cancel();
    engine.await.context("simulation task panicked")??;
    Ok(())
}

async fn wait_for_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("installing Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("installing SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}
