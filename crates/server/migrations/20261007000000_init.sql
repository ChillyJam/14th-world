-- Full world state, written periodically. On startup the newest row is loaded.
CREATE TABLE snapshots (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    tick       INTEGER NOT NULL,
    version    INTEGER NOT NULL,
    data       BLOB    NOT NULL,
    created_at TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Append-only history of notable happenings (meetings, new eras, ...).
CREATE TABLE events (
    id      INTEGER PRIMARY KEY AUTOINCREMENT,
    tick    INTEGER NOT NULL,
    kind    TEXT    NOT NULL,
    payload TEXT    NOT NULL -- JSON
);

CREATE INDEX events_tick ON events (tick);
CREATE INDEX events_kind ON events (kind, tick);
