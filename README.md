# 14th World

A long-running 2D world simulation. A handful of people with no knowledge
wander a world with day/night cycles and animals. As they meet, they form
relationships and learn from each other, and their collective knowledge
slowly carries them from the Primitive era towards the Modern day.

The server is meant to run unattended for months. All state lives in SQLite,
so the world survives restarts and redeploys. A native desktop client
connects to the server and renders the world live.

## Architecture

```
┌──────────────── server (Rust, tokio + axum) ────────────────┐
│                                                             │
│  engine ── World::step() at TICK_RATE_HZ ──► watch channel ─┼──► /ws  (binary postcard frames)
│    │                                                        │       │
│    └── every SNAPSHOT_INTERVAL_SECS ──► SQLite (WAL)        │       │
│          snapshots + append-only event log                  │       │
└─────────────────────────────────────────────────────────────┘       │
                                                                      ▼
                          desktop: world-client (Rust, egui) ── 2D renderer
```

| Crate | What it does |
| --- | --- |
| [`crates/sim`](crates/sim) | The simulation itself. Pure and deterministic: no I/O, no clocks, seeded RNG. The same seed and number of steps always give the same world. |
| [`crates/protocol`](crates/protocol) | Messages between server and client, shared by both so they can't drift apart. Encoded with [postcard](https://docs.rs/postcard). |
| [`crates/server`](crates/server) | Runs the tick loop, persists to SQLite ([sqlx](https://docs.rs/sqlx)) and streams frames over WebSocket ([axum](https://docs.rs/axum)). Headless. |
| [`crates/client`](crates/client) | Native desktop client for Windows, macOS and Linux, built with [egui](https://www.egui.rs). Connects to a server and draws the world. |

### Time

One tick is one in-world minute (1,440 ticks per day). At the default
`TICK_RATE_HZ=10`, an in-world day passes every 2.4 real minutes and a year
roughly every 14.6 hours. People sleep from about 20:00 to 04:00.

### Persistence

- **`snapshots`**: the full serialized `World`, written every
  `SNAPSHOT_INTERVAL_SECS` and on graceful shutdown. Only the newest
  `SNAPSHOTS_TO_KEEP` are kept. The newest one is loaded on startup.
- **`events`**: a permanent, append-only history (people meeting, new eras, …)
  with JSON payloads.

Events are written in the same transaction as the snapshot that follows them.
After a crash the world resumes from the last snapshot, and because the
simulation is deterministic it regenerates exactly the events that were lost.

Snapshots carry a format version (`sim::SNAPSHOT_VERSION`). If you change the
shape of `World`, bump it. The server refuses to load a snapshot with a
different version rather than misreading it, so a long-running world is never
silently corrupted. Add a migration path before deploying such a change.

## Getting started

### Prerequisites

- [Rust](https://rustup.rs) (stable)

### Run locally

```sh
make build                            # build the server and client
make run                              # build, then start the server and a client
```

`make run` keeps the server in the foreground and opens the client window
alongside it. Use `make run-server` or `make run-client` to start just one,
and add `PROFILE=dev` for quicker unoptimised builds. On Windows, `make` needs
a POSIX shell such as Git Bash on `PATH`.

Without `make`, run each binary in its own terminal:

```sh
cargo run --bin world-server          # terminal 1, listens on port 8080
cargo run --bin world-client          # terminal 2, opens a window
```

The world is stored in `data/world.db`. Stop the server with Ctrl+C and it
saves before exiting. Closing the client leaves the world running, and the
client reconnects by itself if the server restarts.

### Connect to another server

Pass the server as an argument, or set `WORLD_SERVER`. Either a bare
`host:port` or a full WebSocket URL works:

```sh
cargo run --release --bin world-client -- my-server.example:8080
cargo run --release --bin world-client -- wss://world.example.com/ws
```

With no argument the client connects to `127.0.0.1:8080`. CI publishes
release builds of the client for Windows, macOS and Linux as workflow
artifacts.

### Run in Docker

```sh
docker compose up -d --build
```

The world is kept in the `world-data` volume. The container restarts
automatically and gets 30 seconds to write its final snapshot when stopped.

## Configuration

The server reads these environment variables (see [`.env.example`](.env.example)):

| Variable | Default | Meaning |
| --- | --- | --- |
| `BIND_ADDR` | `0.0.0.0:8080` | HTTP listen address |
| `DATABASE_URL` | `sqlite://data/world.db` | SQLite database; created if missing |
| `TICK_RATE_HZ` | `10` | Simulation speed (in-world minutes per real second) |
| `SNAPSHOT_INTERVAL_SECS` | `60` | How often the world is saved |
| `SNAPSHOTS_TO_KEEP` | `24` | Older snapshots are pruned |
| `WORLD_SEED` | current time | Seed for a brand-new world |
| `RUST_LOG` | `info,sqlx=warn` | Log filter |

## Development

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

CI runs all of the above, plus release builds of the client on Windows, macOS
and Linux, on every pull request.

## Roadmap

The scaffold gives you movement, day/night, meetings, relationships,
knowledge and eras. Next steps:

- Births, ageing and death, so the population can grow
- Needs (food, shelter) and animals as a food source
- Terrain, resources and settlements
- Discoveries as a tech tree instead of a single knowledge number
- A spatial index for encounters once populations grow (currently O(n²))
- Delta-encoded frames and viewport culling for large worlds
- Click to inspect a person: relationships, history, knowledge
