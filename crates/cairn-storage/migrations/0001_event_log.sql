CREATE TABLE IF NOT EXISTS events (
    event_id INTEGER PRIMARY KEY AUTOINCREMENT,
    protocol_version INTEGER NOT NULL DEFAULT 1,
    payload TEXT NOT NULL,
    created_at_unix_seconds INTEGER NOT NULL DEFAULT (unixepoch())
);

-- Append-only is enforced at the storage boundary, not just by the Rust API.
CREATE TRIGGER IF NOT EXISTS events_append_only_no_update
BEFORE UPDATE ON events
BEGIN
    SELECT RAISE(ABORT, 'events are append-only');
END;

CREATE TRIGGER IF NOT EXISTS events_append_only_no_delete
BEFORE DELETE ON events
BEGIN
    SELECT RAISE(ABORT, 'events are append-only');
END;

CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at_unix_seconds INTEGER NOT NULL DEFAULT (unixepoch())
);

INSERT OR IGNORE INTO schema_migrations (version) VALUES (1);
