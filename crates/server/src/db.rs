use std::str::FromStr;

use anyhow::{Context, Result};
use sim::{Event, World, SNAPSHOT_VERSION};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Row, SqlitePool};

#[derive(Clone)]
pub struct Db {
    pool: SqlitePool,
}

impl Db {
    pub async fn connect(url: &str) -> Result<Self> {
        let options = SqliteConnectOptions::from_str(url)
            .with_context(|| format!("invalid DATABASE_URL {url:?}"))?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal);

        if let Some(dir) = options.get_filename().parent() {
            if !dir.as_os_str().is_empty() {
                std::fs::create_dir_all(dir)
                    .with_context(|| format!("creating database directory {}", dir.display()))?;
            }
        }

        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await
            .context("opening database")?;
        sqlx::migrate!()
            .run(&pool)
            .await
            .context("running migrations")?;
        Ok(Self { pool })
    }

    /// The most recent snapshot, or `None` for a fresh database.
    pub async fn load_latest_world(&self) -> Result<Option<World>> {
        let row = sqlx::query("SELECT data FROM snapshots ORDER BY id DESC LIMIT 1")
            .fetch_optional(&self.pool)
            .await?;
        row.map(|row| {
            let data: Vec<u8> = row.get("data");
            World::from_snapshot(&data).context("decoding latest snapshot")
        })
        .transpose()
    }

    /// Atomically append `events` and a new snapshot of `world`, then prune old
    /// snapshots. The event log therefore never runs ahead of the stored state:
    /// after a crash the simulation resumes from the snapshot and, being
    /// deterministic, regenerates exactly the events that were lost.
    pub async fn save(&self, world: &World, events: &[(u64, Event)], keep: u32) -> Result<()> {
        let snapshot = world.to_snapshot()?;
        let mut tx = self.pool.begin().await?;

        for (tick, event) in events {
            sqlx::query("INSERT INTO events (tick, kind, payload) VALUES (?, ?, ?)")
                .bind(*tick as i64)
                .bind(event.kind())
                .bind(serde_json::to_string(event)?)
                .execute(&mut *tx)
                .await?;
        }

        sqlx::query("INSERT INTO snapshots (tick, version, data) VALUES (?, ?, ?)")
            .bind(world.time.tick as i64)
            .bind(SNAPSHOT_VERSION as i64)
            .bind(snapshot)
            .execute(&mut *tx)
            .await?;

        sqlx::query(
            "DELETE FROM snapshots WHERE id NOT IN \
             (SELECT id FROM snapshots ORDER BY id DESC LIMIT ?)",
        )
        .bind(keep as i64)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::WorldConfig;

    async fn temp_db() -> (tempfile::TempDir, Db) {
        let dir = tempfile::tempdir().unwrap();
        let url = format!("sqlite://{}", dir.path().join("nested/world.db").display());
        let db = Db::connect(&url).await.unwrap();
        (dir, db)
    }

    #[tokio::test]
    async fn empty_database_has_no_world() {
        let (_dir, db) = temp_db().await;
        assert!(db.load_latest_world().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn saves_and_restores_latest_world() {
        let (_dir, db) = temp_db().await;
        let mut world = World::new(5, WorldConfig::default());
        let mut events = Vec::new();
        for _ in 0..3 {
            for _ in 0..sim::TICKS_PER_DAY {
                events.extend(world.step().into_iter().map(|e| (world.time.tick, e)));
            }
            db.save(&world, &events, 2).await.unwrap();
            events.clear();
        }

        assert_eq!(db.load_latest_world().await.unwrap(), Some(world));

        let snapshots: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM snapshots")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(snapshots, 2, "older snapshots are pruned");
    }
}
