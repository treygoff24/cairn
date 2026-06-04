//! `cairn-storage` — the append-only event log: the daemon's spinal cord.
//!
//! # Scope: Wave 1.2 contract surface
//!
//! Publishes the [`EventLog`] trait and the monotonic [`EventId`]. The Wave 1.2
//! implementer builds a concrete log behind this trait against a SQLite-WAL (or
//! equivalent durable) backend — the backend lives behind the trait so its blast
//! radius is one crate. Required behavior (plan acceptance): atomic append,
//! strictly-increasing IDs that survive restart, deterministic ordered replay, a
//! credential-redaction pass on append, and a materialized-view skeleton.
//!
//! The Phase 1 golden fixture freezes at the Wave 1.3 boundary — do not reshape
//! this trait after that without a contract bump.

use std::{fs, path::Path, time::Duration};

use cairn_protocol::{CURRENT_PROTOCOL_VERSION, DaemonEvent, DaemonEventEnvelope};
use cairn_types::DaemonEventKind;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

const SCHEMA: &str = include_str!("../migrations/0001_event_log.sql");
const REDACTED: &str = "[REDACTED]";

/// Monotonic, log-assigned identifier for an appended event. Strictly increasing in
/// append order and never reused; assigned by the log, not the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EventId(pub u64);

impl EventId {
    /// Returns the raw numeric event id.
    #[must_use]
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

/// Append-only, crash-durable log of [`DaemonEvent`]s with monotonic IDs and
/// deterministic replay. Backend-agnostic via the associated [`EventLog::Error`].
pub trait EventLog {
    /// Backend-specific failure (I/O, corruption, lock contention).
    type Error: std::error::Error + Send + Sync + 'static;

    /// Durably append one event and return its assigned monotonic [`EventId`].
    /// Must be atomic: either the event is persisted with its ID or the call fails.
    fn append(&mut self, event: &DaemonEvent) -> Result<EventId, Self::Error>;

    /// Replay persisted events in ascending [`EventId`] order, exclusive of `after`
    /// (`None` replays from the beginning).
    fn replay(&self, after: Option<EventId>) -> Result<Vec<(EventId, DaemonEvent)>, Self::Error>;

    /// The highest [`EventId`] issued so far, or `None` if the log is empty.
    fn latest_id(&self) -> Result<Option<EventId>, Self::Error>;
}

/// Errors raised by the durable SQLite event log.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// Filesystem setup failed while opening the database path.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// SQLite failed to open, initialize, append, or replay the event log.
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// A daemon event could not be serialized or decoded.
    #[error("event serialization failed: {0}")]
    Serde(#[from] serde_json::Error),
    /// Caller supplied an event id too large for SQLite's signed rowid domain.
    #[error("event id {0} is outside SQLite's rowid range")]
    EventIdOutOfRange(u64),
    /// SQLite returned a rowid that cannot be a Cairn event id.
    #[error("SQLite returned invalid event rowid {0}")]
    InvalidRowId(i64),
    /// Persisted protocol versions disagreed or are not supported by this build.
    #[error("stored protocol version {stored} does not match envelope protocol version {envelope}")]
    ProtocolVersionMismatch { stored: u32, envelope: u32 },
}

/// SQLite-WAL implementation of [`EventLog`].
///
/// SQLite owns write serialization across multiple processes, `AUTOINCREMENT`
/// preserves monotonic IDs across restarts, and each append is wrapped in one
/// transaction so event id assignment and payload persistence commit together.
pub struct SqliteEventLog {
    conn: Connection,
}

impl SqliteEventLog {
    /// Opens or creates a durable event log database at `path`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let path = path.as_ref();
        create_parent_directory(path)?;

        let conn = Connection::open(path)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = FULL;")?;
        conn.execute_batch(SCHEMA)?;

        Ok(Self { conn })
    }

    /// Exposes the backing connection for crate-local tests.
    #[cfg(test)]
    fn connection(&self) -> &Connection {
        &self.conn
    }
}

impl EventLog for SqliteEventLog {
    type Error = StorageError;

    fn append(&mut self, event: &DaemonEvent) -> Result<EventId, Self::Error> {
        let payload = serialize_redacted_event(event)?;
        let tx = self.conn.transaction()?;

        tx.execute(
            "INSERT INTO events (protocol_version, payload) VALUES (?1, ?2)",
            params![CURRENT_PROTOCOL_VERSION.0, payload],
        )?;
        let event_id = event_id_from_rowid(tx.last_insert_rowid())?;
        tx.commit()?;

        Ok(event_id)
    }

    fn replay(&self, after: Option<EventId>) -> Result<Vec<(EventId, DaemonEvent)>, Self::Error> {
        let after = after.map(event_id_to_rowid).transpose()?;
        let mut stmt = self.conn.prepare(
            "SELECT event_id, protocol_version, payload
             FROM events
             WHERE (?1 IS NULL OR event_id > ?1)
             ORDER BY event_id ASC",
        )?;
        let mut rows = stmt.query(params![after])?;
        let mut events = Vec::new();

        while let Some(row) = rows.next()? {
            let event_id = event_id_from_rowid(row.get::<_, i64>(0)?)?;
            let protocol_version = row.get::<_, u32>(1)?;
            let payload = row.get::<_, String>(2)?;
            let event = deserialize_persisted_event(protocol_version, &payload)?;
            events.push((event_id, event));
        }

        Ok(events)
    }

    fn latest_id(&self) -> Result<Option<EventId>, Self::Error> {
        let rowid = self
            .conn
            .query_row("SELECT MAX(event_id) FROM events", [], |row| {
                row.get::<_, Option<i64>>(0)
            })?;

        rowid.map(event_id_from_rowid).transpose()
    }
}

/// A replay target backed by materialized state.
pub trait MaterializedView {
    /// Apply one already-ordered event to the view.
    fn apply(&mut self, event_id: EventId, event: &DaemonEvent);
}

/// Replays the event log into a materialized view skeleton.
pub fn replay_into<L, V>(log: &L, view: &mut V, after: Option<EventId>) -> Result<(), L::Error>
where
    L: EventLog + ?Sized,
    V: MaterializedView + ?Sized,
{
    for (event_id, event) in log.replay(after)? {
        view.apply(event_id, &event);
    }

    Ok(())
}

/// Minimal materialized-view skeleton used until product ledgers land.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventLogView {
    latest_event_id: Option<EventId>,
    applied_events: usize,
    last_event_kind: Option<DaemonEventKind>,
}

impl EventLogView {
    /// Highest event id applied to this view.
    #[must_use]
    pub fn latest_event_id(&self) -> Option<EventId> {
        self.latest_event_id
    }

    /// Number of events applied to this view.
    #[must_use]
    pub fn applied_events(&self) -> usize {
        self.applied_events
    }

    /// Kind of the most recently applied event.
    #[must_use]
    pub fn last_event_kind(&self) -> Option<DaemonEventKind> {
        self.last_event_kind
    }
}

impl MaterializedView for EventLogView {
    fn apply(&mut self, event_id: EventId, event: &DaemonEvent) {
        self.latest_event_id = Some(event_id);
        self.applied_events += 1;
        self.last_event_kind = Some(event.kind());
    }
}

fn create_parent_directory(path: &Path) -> Result<(), StorageError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }

    Ok(())
}

fn event_id_to_rowid(event_id: EventId) -> Result<i64, StorageError> {
    i64::try_from(event_id.0).map_err(|_| StorageError::EventIdOutOfRange(event_id.0))
}

fn event_id_from_rowid(rowid: i64) -> Result<EventId, StorageError> {
    if rowid <= 0 {
        return Err(StorageError::InvalidRowId(rowid));
    }

    Ok(EventId(rowid as u64))
}

fn serialize_redacted_event(event: &DaemonEvent) -> Result<String, StorageError> {
    let mut value = serde_json::to_value(DaemonEventEnvelope::current(event.clone()))?;
    redact_json_strings(&mut value);

    serde_json::to_string(&value).map_err(StorageError::from)
}

fn deserialize_persisted_event(
    stored_protocol_version: u32,
    payload: &str,
) -> Result<DaemonEvent, StorageError> {
    let envelope = serde_json::from_str::<DaemonEventEnvelope>(payload)?;
    validate_protocol_version(stored_protocol_version, envelope.protocol_version.0)?;
    Ok(envelope.event)
}

fn validate_protocol_version(stored: u32, envelope: u32) -> Result<(), StorageError> {
    if stored == CURRENT_PROTOCOL_VERSION.0 && envelope == CURRENT_PROTOCOL_VERSION.0 {
        return Ok(());
    }

    Err(StorageError::ProtocolVersionMismatch { stored, envelope })
}

fn redact_json_strings(value: &mut serde_json::Value) {
    redact_json_value(value, false);
}

fn redact_json_value(value: &mut serde_json::Value, secret_context: bool) {
    match value {
        serde_json::Value::String(text) => {
            *text = if secret_context {
                REDACTED.to_owned()
            } else {
                redact_text(text)
            };
        }
        serde_json::Value::Array(items) => {
            for item in items {
                redact_json_value(item, secret_context);
            }
        }
        serde_json::Value::Object(fields) => {
            for (field_name, value) in fields {
                let field_is_secret = secret_context || is_secret_field_name(field_name);
                redact_json_value(value, field_is_secret);
            }
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {}
    }
}

fn redact_text(text: &str) -> String {
    let mut redacted = String::with_capacity(text.len());
    let mut token = String::new();

    for ch in text.chars() {
        if is_credential_token_char(ch) {
            token.push(ch);
        } else {
            push_redacted_token(&mut redacted, &token);
            token.clear();
            redacted.push(ch);
        }
    }

    push_redacted_token(&mut redacted, &token);
    redacted
}

fn push_redacted_token(output: &mut String, token: &str) {
    if token.is_empty() {
        return;
    }

    if looks_like_credential(token) {
        output.push_str(REDACTED);
    } else {
        output.push_str(token);
    }
}

fn is_credential_token_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.')
}

fn looks_like_credential(token: &str) -> bool {
    looks_like_aws_access_key(token)
        || has_secret_prefix(token, "sk_live_", 16)
        || has_secret_prefix(token, "sk_test_", 16)
        || has_secret_prefix(token, "ghp_", 12)
        || has_secret_prefix(token, "github_pat_", 20)
        || has_secret_prefix(token, "eyJ", 24)
        || has_secret_prefix(token, "xoxb-", 12)
        || has_secret_prefix(token, "xoxp-", 12)
        || has_secret_prefix(token, "AIza", 24)
        || has_secret_prefix(token, "ya29.", 12)
        || has_secret_prefix(token, "glpat-", 12)
}

fn looks_like_aws_access_key(token: &str) -> bool {
    let has_aws_prefix = token.starts_with("AKIA") || token.starts_with("ASIA");
    has_aws_prefix && token.len() >= 20 && token.chars().all(|ch| ch.is_ascii_alphanumeric())
}

fn has_secret_prefix(token: &str, prefix: &str, min_len: usize) -> bool {
    token.starts_with(prefix) && token.len() >= min_len
}

fn is_secret_field_name(field_name: &str) -> bool {
    let normalized = normalize_field_name(field_name);

    matches!(
        normalized.as_str(),
        "auth"
            | "access_key"
            | "accesskey"
            | "api_key"
            | "apikey"
            | "authorization"
            | "auth_token"
            | "authtoken"
            | "bearer"
            | "bearertoken"
            | "client_secret"
            | "clientsecret"
            | "credential"
            | "credentials"
            | "password"
            | "passwd"
            | "passphrase"
            | "private_key"
            | "privatekey"
            | "refresh_token"
            | "refreshtoken"
            | "secret"
            | "secret_key"
            | "secretkey"
            | "token"
    ) || normalized.ends_with("_api_key")
        || normalized.ends_with("_apikey")
        || normalized.ends_with("_access_key")
        || normalized.ends_with("_auth_token")
        || normalized.ends_with("_client_secret")
        || normalized.ends_with("_private_key")
        || normalized.ends_with("_refresh_token")
        || normalized.ends_with("_secret")
        || normalized.ends_with("_secret_key")
        || normalized.ends_with("_token")
}

fn normalize_field_name(field_name: &str) -> String {
    field_name
        .chars()
        .map(|ch| match ch {
            '-' | ' ' => '_',
            _ => ch.to_ascii_lowercase(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, thread};

    use cairn_protocol::{
        AdapterHeartbeat, AdapterKind, AdapterRef, CURRENT_PROTOCOL_VERSION, DegradedState,
        ToolIntent,
    };
    use cairn_types::{AdapterCapabilities, Timestamp, WorktreeId};
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn append_replay_and_latest_id_are_ordered() {
        let dir = TempDir::new().expect("tempdir");
        let mut log = SqliteEventLog::open(dir.path().join("events.db")).expect("open log");

        let first = log.append(&heartbeat(1)).expect("append first");
        let second = log.append(&heartbeat(2)).expect("append second");
        let third = log.append(&heartbeat(3)).expect("append third");

        assert_eq!([first.0, second.0, third.0], [1, 2, 3]);
        assert_eq!(log.latest_id().expect("latest id"), Some(third));

        let all_ids = replayed_ids(&log, None);
        let after_first = replayed_ids(&log, Some(first));

        assert_eq!(all_ids, vec![first, second, third]);
        assert_eq!(after_first, vec![second, third]);
    }

    #[test]
    fn event_ids_survive_restart() {
        let dir = TempDir::new().expect("tempdir");
        let db_path = dir.path().join("events.db");

        let first = {
            let mut log = SqliteEventLog::open(&db_path).expect("open log");
            log.append(&heartbeat(1)).expect("append first")
        };

        let mut reopened = SqliteEventLog::open(&db_path).expect("reopen log");
        let second = reopened
            .append(&heartbeat(2))
            .expect("append after restart");

        assert_eq!(first, EventId(1));
        assert_eq!(second, EventId(2));
        assert_eq!(reopened.latest_id().expect("latest id"), Some(second));
    }

    #[test]
    fn sqlite_log_uses_wal_journal_mode() {
        let dir = TempDir::new().expect("tempdir");
        let log = SqliteEventLog::open(dir.path().join("events.db")).expect("open log");

        let journal_mode: String = log
            .connection()
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .expect("journal mode");

        assert_eq!(journal_mode.to_ascii_lowercase(), "wal");
    }

    #[test]
    fn failed_append_does_not_persist_partial_event_or_consume_id() {
        let dir = TempDir::new().expect("tempdir");
        let mut log = SqliteEventLog::open(dir.path().join("events.db")).expect("open log");

        log.connection()
            .execute_batch(
                "CREATE TRIGGER fail_next_insert
                 BEFORE INSERT ON events
                 BEGIN
                     SELECT RAISE(ABORT, 'simulated append failure');
                 END;",
            )
            .expect("install failing trigger");

        let err = log
            .append(&heartbeat(1))
            .expect_err("append should fail before commit");
        assert!(matches!(err, StorageError::Sqlite(_)));

        let persisted_count: u64 = log
            .connection()
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .expect("event count");
        assert_eq!(persisted_count, 0);

        log.connection()
            .execute_batch("DROP TRIGGER fail_next_insert;")
            .expect("drop failing trigger");

        let event_id = log.append(&heartbeat(2)).expect("append after failure");
        assert_eq!(event_id, EventId(1));
    }

    #[test]
    fn persisted_events_are_append_only() {
        let dir = TempDir::new().expect("tempdir");
        let mut log = SqliteEventLog::open(dir.path().join("events.db")).expect("open log");
        let event_id = log.append(&heartbeat(1)).expect("append");
        let rowid = event_id_to_rowid(event_id).expect("rowid");

        let update_result = log.connection().execute(
            "UPDATE events SET payload = ?1 WHERE event_id = ?2",
            rusqlite::params!["{}", rowid],
        );
        let delete_result = log.connection().execute(
            "DELETE FROM events WHERE event_id = ?1",
            rusqlite::params![rowid],
        );

        assert!(update_result.is_err(), "event payloads must not be mutable");
        assert!(delete_result.is_err(), "event rows must not be deletable");
        assert_eq!(replayed_ids(&log, None), vec![event_id]);
    }

    #[test]
    fn append_stores_redacted_payload() {
        let dir = TempDir::new().expect("tempdir");
        let mut log = SqliteEventLog::open(dir.path().join("events.db")).expect("open log");
        let event = heartbeat_with_degraded_reason(
            1,
            "adapter queued token ghp_123456789abcdef before reconnect",
        );

        log.append(&event).expect("append");

        let (protocol_version, payload): (u32, String) = log
            .connection()
            .query_row(
                "SELECT protocol_version, payload FROM events WHERE event_id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("stored payload");
        let decoded =
            deserialize_persisted_event(protocol_version, &payload).expect("payload decodes");

        assert_eq!(protocol_version, CURRENT_PROTOCOL_VERSION.0);
        assert!(!payload.contains("ghp_123456789abcdef"));
        assert_eq!(
            decoded,
            heartbeat_with_degraded_reason(
                1,
                &format!("adapter queued token {REDACTED} before reconnect")
            )
        );
    }

    #[test]
    fn redactor_catches_obvious_credentials_without_touching_keys() {
        let mut value = serde_json::json!({
            "ghp_field": "token ghp_123456789abcdef",
            "aws": "AKIAABCDEFGHIJKLMNOP",
            "stripe": "sk_live_1234567890abcdef",
            "jwt": "eyJhbGciOiJIUzI1NiIsInR5cCI",
            "safe": "not-a-secret",
        });

        redact_json_strings(&mut value);

        assert!(value.get("ghp_field").is_some());
        assert_eq!(value["safe"], "not-a-secret");
        assert_eq!(value["ghp_field"], format!("token {REDACTED}"));
        assert_eq!(value["aws"], REDACTED);
        assert_eq!(value["stripe"], REDACTED);
        assert_eq!(value["jwt"], REDACTED);
    }

    #[test]
    fn redactor_catches_structured_secret_fields_without_touching_keys() {
        let mut value = serde_json::json!({
            "api_key": "ordinary-looking-value",
            "credentials": {
                "nested": "also-ordinary-looking"
            },
            "token_usage": {
                "input_tokens": 12,
                "label": "keep diagnostics readable"
            },
            "safe": "not-a-secret",
        });

        redact_json_strings(&mut value);

        assert!(value.get("api_key").is_some());
        assert!(value.get("credentials").is_some());
        assert_eq!(value["api_key"], REDACTED);
        assert_eq!(value["credentials"]["nested"], REDACTED);
        assert_eq!(value["token_usage"]["input_tokens"], 12);
        assert_eq!(value["token_usage"]["label"], "keep diagnostics readable");
        assert_eq!(value["safe"], "not-a-secret");
    }

    #[test]
    fn append_redacts_secret_named_validated_shape_without_revalidating_payload() {
        let dir = TempDir::new().expect("tempdir");
        let mut log = SqliteEventLog::open(dir.path().join("events.db")).expect("open log");
        let event = tool_intent_with_secret_named_file_version();

        log.append(&event)
            .expect("append should not revalidate redacted JSON");

        let (protocol_version, payload): (u32, String) = log
            .connection()
            .query_row(
                "SELECT protocol_version, payload FROM events WHERE event_id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("stored payload");
        let replayed = log.replay(None).expect("redacted payload should replay");

        assert_eq!(protocol_version, CURRENT_PROTOCOL_VERSION.0);
        assert!(
            !payload.contains("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
        assert!(payload.contains(REDACTED));
        assert_eq!(replayed.len(), 1);
    }

    #[test]
    fn materialized_view_replays_from_log() {
        let dir = TempDir::new().expect("tempdir");
        let mut log = SqliteEventLog::open(dir.path().join("events.db")).expect("open log");

        log.append(&heartbeat(1)).expect("append first");
        let second = log.append(&heartbeat(2)).expect("append second");

        let mut view = EventLogView::default();
        replay_into(&log, &mut view, None).expect("replay into view");

        assert_eq!(view.applied_events(), 2);
        assert_eq!(view.latest_event_id(), Some(second));
        assert_eq!(
            view.last_event_kind(),
            Some(DaemonEventKind::AdapterHeartbeat)
        );
    }

    #[test]
    fn concurrent_appends_are_serialized_by_sqlite() {
        let dir = TempDir::new().expect("tempdir");
        let db_path = Arc::new(dir.path().join("events.db"));
        SqliteEventLog::open(db_path.as_ref()).expect("initialize schema");

        let threads = 6;
        let appends_per_thread = 25;
        let mut handles = Vec::new();

        for thread_index in 0..threads {
            let db_path = Arc::clone(&db_path);
            handles.push(thread::spawn(move || {
                let mut log = SqliteEventLog::open(db_path.as_ref()).expect("open thread log");
                let mut ids = Vec::new();

                for event_index in 0..appends_per_thread {
                    let timestamp = i64::from(thread_index * appends_per_thread + event_index);
                    ids.push(log.append(&heartbeat(timestamp)).expect("append event"));
                }

                ids
            }));
        }

        let mut ids = Vec::new();
        for handle in handles {
            ids.extend(handle.join().expect("thread joined"));
        }
        ids.sort_unstable();

        let expected_count = threads * appends_per_thread;
        let expected_ids = (1..=expected_count as u64).map(EventId).collect::<Vec<_>>();
        let replayed_ids = replayed_ids(
            &SqliteEventLog::open(db_path.as_ref()).expect("open replay log"),
            None,
        );

        assert_eq!(ids, expected_ids);
        assert_eq!(replayed_ids, expected_ids);
    }

    fn replayed_ids(log: &SqliteEventLog, after: Option<EventId>) -> Vec<EventId> {
        log.replay(after)
            .expect("replay")
            .into_iter()
            .map(|(event_id, _)| event_id)
            .collect()
    }

    fn heartbeat(timestamp: i64) -> DaemonEvent {
        heartbeat_with_degraded_reason(timestamp, "")
    }

    fn heartbeat_with_degraded_reason(timestamp: i64, degraded_reason: &str) -> DaemonEvent {
        DaemonEvent::AdapterHeartbeat(AdapterHeartbeat {
            agent_session_id: None,
            worktree_id: WorktreeId::new("storage-test-worktree"),
            harness: AdapterRef {
                adapter_id: "storage-test".to_owned(),
                adapter_kind: AdapterKind::HarnessSim,
            },
            capabilities: AdapterCapabilities::default(),
            sent_at: Timestamp(timestamp),
            daemon_generation_id: None,
            token_usage: None,
            queued_event_count: 0,
            degraded: (!degraded_reason.is_empty()).then(|| DegradedState {
                reason: degraded_reason.to_owned(),
                fail_open: true,
                since: Timestamp(timestamp),
            }),
        })
    }

    fn tool_intent_with_secret_named_file_version() -> DaemonEvent {
        DaemonEvent::ToolIntent(ToolIntent {
            agent_session_id: cairn_types::SessionId::new("storage-session"),
            worktree_id: WorktreeId::new("storage-test-worktree"),
            harness: AdapterRef {
                adapter_id: "storage-test".to_owned(),
                adapter_kind: AdapterKind::HarnessSim,
            },
            tool_call_id: "tool-1".to_owned(),
            tool_name: "Read".to_owned(),
            input: serde_json::json!({
                "api_key": {
                    "file_id": "file-1",
                    "path": "src/lib.rs",
                    "content_hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "size": 12,
                    "mtime_observed": "1",
                    "executable_bit": false,
                    "symlink_target": null,
                    "repo_epoch_id": "epoch-1",
                    "source_class": "source"
                }
            }),
            repo_epoch_id: None,
            occurred_at: Timestamp(1),
            token_usage: None,
        })
    }
}
