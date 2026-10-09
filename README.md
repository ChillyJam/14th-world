# 14th World

A long-running 2D world simulation. A handful of people with no knowledge
wander a world with day/night cycles and animals. As they meet, they form
relationships and learn from each other, and their collective knowledge
slowly carries them from the Primitive era towards the Modern day.

The world is scattered with materials: trees, rocks and flint from the start,
plus berries, mushrooms and roots to forage,
then clay, copper, tin and iron ore that people can only work once they reach
the right era. People gather from deposits they pass. Trees and foraged plants grow back, but
everything dug from the ground eventually runs out.

The server is meant to run unattended for months. All state lives in SQLite,
so the world survives restarts and redeploys. The server also serves a web
page that renders the world live in any browser.

## Architecture

```
┌──────────────── server (Rust, tokio + axum) ────────────────┐
│                                                             │
│  engine ── World::step() at TICK_RATE_HZ ──► watch channel ─┼──► /ws  (JSON frames)       
│    │                                                        │       │
│    └── every SNAPSHOT_INTERVAL_SECS ──► SQLite (WAL)        │       │
│          snapshots + append-only event log                  │       │
└─────────────────────────────────────────────────────────────┘       │
                                                                      ▼
                          browser: GET /  (canvas 2D renderer, one HTML file)
```

| Crate | What it does |
| --- | --- |
| [`crates/sim`](crates/sim) | The simulation itself. Pure and deterministic: no I/O, no clocks, seeded RNG. The same seed and number of steps always give the same world. |
| [`crates/protocol`](crates/protocol) | Messages between server and browser, as JSON. A catalog of materials, species and eras is sent on connect so the page never duplicates the rules. |
| [`crates/server`](crates/server) | Runs the tick loop, persists to SQLite ([sqlx](https://docs.rs/sqlx)) and streams frames over WebSocket ([axum](https://docs.rs/axum)). Also serves the web client from [`assets/index.html`](crates/server/assets/index.html), compiled into the binary. |

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
silently corrupted. Add a migration path before deploying such a change: keep
the old layout in a module (see `v1` in `crates/sim/src/world.rs`) and convert
it in `World::from_snapshot`.

## Getting started

### Prerequisites

- [Rust](https://rustup.rs) (stable)

### Run locally

```sh
make run                              # build and start the server
```

Then open <http://localhost:8080>. Add `PROFILE=dev` for a quicker unoptimised
build. On Windows, use `make.cmd` instead; it needs nothing but Rust and works
from both Command Prompt and PowerShell:

```powershell
.\make run                            # build and start the server
.\make run -Profile dev               # quicker unoptimised build
```

Without `make`: `cargo run --bin world-server`.

Click a person, animal or material deposit to see its stats. For people that's
age, knowledge, whether they're asleep, what they're carrying and who they're
close to. For deposits it's how much is left and whether it can be gathered
yet. Trees are drawn as dark green dots and everything else as squares, and
used-up deposits fade out. Press Esc or click empty ground to close the panel.

The world is stored in `data/world.db`. Stop the server with Ctrl+C and it
saves before exiting. Closing the page leaves the world running, and the page
reconnects by itself if the server restarts.

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

CI runs all of the above on every pull request.

## Roadmap

The scaffold gives you movement, day/night, meetings, relationships,
knowledge, eras and materials. Next steps:

- Births, ageing and death, so the population can grow
- Needs (food, shelter) and animals as a food source
- Terrain and settlements
- Using gathered materials: tools, shelter and crafting
- Discoveries as a tech tree instead of a single knowledge number
- A spatial index for encounters once populations grow (currently O(n²))
- Delta-encoded frames and viewport culling for large worlds
- A person's history (who they met and when) in the stats panel
