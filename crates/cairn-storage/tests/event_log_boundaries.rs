use std::{
    ffi::OsString,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use cairn_protocol::{
    AdapterHeartbeat, AdapterKind, AdapterRef, CURRENT_PROTOCOL_VERSION, DaemonEvent,
    DaemonEventEnvelope,
};
use cairn_storage::{EventLog, SqliteEventLog, StorageError};
use cairn_types::{AdapterCapabilities, EventId, ProtocolVersion, Timestamp, WorktreeId};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use tempfile::TempDir;

const CHILD_DB_PATH: &str = "CAIRN_EVENT_LOG_CHILD_DB";
const CHILD_WRITER_INDEX: &str = "CAIRN_EVENT_LOG_WRITER_INDEX";
const CHILD_ABORT_AFTER: &str = "CAIRN_EVENT_LOG_ABORT_AFTER";
const WRITER_COUNT: usize = 4;
const EVENTS_PER_WRITER: usize = 32;

#[test]
fn crash_with_torn_wal_tail_recovers_only_committed_events() {
    let dir = TempDir::new().expect("tempdir");
    let db_path = dir.path().join("events.db");
    SqliteEventLog::open(&db_path).expect("initialize event log");

    let crashed = run_child("committed_wal_writer", &db_path, None);
    assert!(!crashed.success(), "writer must terminate without cleanup");

    let wal_path = sqlite_sidecar_path(&db_path, "-wal");
    let mut wal = OpenOptions::new()
        .append(true)
        .open(&wal_path)
        .expect("crashed writer should leave a WAL");
    wal.write_all(&[0x43, 0x41, 0x49, 0x52, 0x4e, 0x01, 0x02])
        .expect("append partial WAL frame");
    wal.sync_all().expect("persist torn WAL tail");

    let mut recovered = SqliteEventLog::open(&db_path).expect("recover event log");
    assert_eq!(replayed_ids(&recovered), vec![EventId(1)]);
    assert_eq!(
        recovered
            .append(&heartbeat(2))
            .expect("append after recovery"),
        EventId(2)
    );
    assert_eq!(replayed_ids(&recovered), vec![EventId(1), EventId(2)]);
}

#[test]
fn committed_wal_writer() {
    let Ok(db_path) = std::env::var(CHILD_DB_PATH) else {
        return;
    };

    let mut log = SqliteEventLog::open(db_path).expect("open child event log");
    assert_eq!(log.append(&heartbeat(1)).expect("commit event"), EventId(1));
    std::process::abort();
}

#[test]
fn golden_v1_event_log_fixture_replays_byte_for_byte() {
    let fixture_text = include_str!("fixtures/event_log_v1.json").trim();
    let fixture: Value = serde_json::from_str(fixture_text).expect("parse golden fixture");
    let rows = fixture.as_array().expect("fixture rows");
    let dir = TempDir::new().expect("tempdir");
    let db_path = dir.path().join("events.db");
    let mut log = SqliteEventLog::open(&db_path).expect("open event log");

    for row in rows {
        let event_id = row["event_id"].as_u64().expect("fixture event id");
        let envelope: DaemonEventEnvelope =
            serde_json::from_value(row["payload"].clone()).expect("decode fixture envelope");
        assert_eq!(
            log.append(&envelope.event).expect("append fixture event"),
            EventId(event_id)
        );
    }
    drop(log);

    let conn = Connection::open(&db_path).expect("open fixture database");
    for row in rows {
        let event_id = row["event_id"].as_u64().expect("fixture event id");
        let (stored_version, stored_payload): (u32, String) = conn
            .query_row(
                "SELECT protocol_version, payload FROM events WHERE event_id = ?1",
                [event_id],
                |db_row| Ok((db_row.get(0)?, db_row.get(1)?)),
            )
            .expect("read stored fixture row");
        assert_eq!(stored_version, row["protocol_version"]);
        assert_eq!(
            stored_payload,
            serde_json::to_string(&row["payload"]).unwrap()
        );
    }
    drop(conn);

    let replayed = SqliteEventLog::open(&db_path)
        .expect("reopen fixture log")
        .replay(None)
        .expect("replay fixture")
        .into_iter()
        .map(|(event_id, event)| {
            json!({
                "event_id": event_id.0,
                "payload": DaemonEventEnvelope::current(event),
                "protocol_version": CURRENT_PROTOCOL_VERSION.0,
            })
        })
        .collect::<Value>();

    assert_eq!(serde_json::to_string(&replayed).unwrap(), fixture_text);
}

#[test]
fn replay_rejects_persisted_row_and_envelope_protocol_mismatch() {
    let dir = TempDir::new().expect("tempdir");
    let db_path = dir.path().join("events.db");
    SqliteEventLog::open(&db_path).expect("initialize event log");

    let envelope = DaemonEventEnvelope {
        protocol_version: ProtocolVersion(CURRENT_PROTOCOL_VERSION.0 + 1),
        event: heartbeat(1),
    };
    Connection::open(&db_path)
        .expect("open raw database")
        .execute(
            "INSERT INTO events (protocol_version, payload) VALUES (?1, ?2)",
            params![
                CURRENT_PROTOCOL_VERSION.0,
                serde_json::to_string(&envelope).expect("serialize mismatched envelope")
            ],
        )
        .expect("insert mismatched row");

    let error = SqliteEventLog::open(&db_path)
        .expect("open event log")
        .replay(None)
        .expect_err("protocol mismatch must fail replay");
    assert!(matches!(
        error,
        StorageError::ProtocolVersionMismatch { stored, envelope }
            if stored == CURRENT_PROTOCOL_VERSION.0
                && envelope == CURRENT_PROTOCOL_VERSION.0 + 1
    ));
}

#[test]
fn event_ids_are_unique_and_contiguous_across_processes() {
    let dir = TempDir::new().expect("tempdir");
    let db_path = dir.path().join("events.db");
    SqliteEventLog::open(&db_path).expect("initialize event log");

    let killed_after = EVENTS_PER_WRITER / 2;
    let children = (0..WRITER_COUNT)
        .map(|writer_index| {
            let mut command =
                Command::new(std::env::current_exe().expect("current test executable"));
            command
                .args(["--exact", "cross_process_append_worker", "--nocapture"])
                .env(CHILD_DB_PATH, &db_path)
                .env(CHILD_WRITER_INDEX, writer_index.to_string())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            if writer_index == 0 {
                command.env(CHILD_ABORT_AFTER, killed_after.to_string());
            }
            (
                writer_index,
                command.spawn().expect("spawn event-log writer"),
            )
        })
        .collect::<Vec<_>>();

    for (writer_index, child) in children {
        let output = child.wait_with_output().expect("wait for writer");
        if writer_index == 0 {
            assert!(!output.status.success(), "selected writer must be killed");
        } else {
            assert!(
                output.status.success(),
                "writer failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    let committed_before_restart = (WRITER_COUNT - 1) * EVENTS_PER_WRITER + killed_after;
    let mut restarted = SqliteEventLog::open(&db_path).expect("restart completed event log");
    assert_eq!(
        restarted
            .append(&heartbeat(i64::MAX))
            .expect("append after restart"),
        EventId((committed_before_restart + 1) as u64)
    );
    let ids = replayed_ids(&restarted);
    let expected = (1..=(committed_before_restart + 1) as u64)
        .map(EventId)
        .collect::<Vec<_>>();
    assert_eq!(ids, expected);
    assert_eq!(
        replayed_ids(&restarted),
        ids,
        "replay order must be deterministic"
    );
}

#[test]
fn cross_process_append_worker() {
    let (Ok(db_path), Ok(writer_index)) = (
        std::env::var(CHILD_DB_PATH),
        std::env::var(CHILD_WRITER_INDEX),
    ) else {
        return;
    };
    let writer_index = writer_index.parse::<usize>().expect("writer index");
    let abort_after = std::env::var(CHILD_ABORT_AFTER)
        .ok()
        .map(|value| value.parse::<usize>().expect("abort count"));
    let mut log = SqliteEventLog::open(db_path).expect("open writer event log");

    for event_index in 0..EVENTS_PER_WRITER {
        let timestamp = (writer_index * EVENTS_PER_WRITER + event_index) as i64;
        log.append(&heartbeat(timestamp)).expect("append event");
        if abort_after == Some(event_index + 1) {
            std::process::abort();
        }
    }
}

fn run_child(
    test_name: &str,
    db_path: &Path,
    writer_index: Option<usize>,
) -> std::process::ExitStatus {
    let mut command = Command::new(std::env::current_exe().expect("current test executable"));
    command
        .args(["--exact", test_name, "--nocapture"])
        .env(CHILD_DB_PATH, db_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(writer_index) = writer_index {
        command.env(CHILD_WRITER_INDEX, writer_index.to_string());
    }
    command.status().expect("run child test")
}

fn sqlite_sidecar_path(db_path: &Path, suffix: &str) -> PathBuf {
    let mut sidecar = OsString::from(db_path.as_os_str());
    sidecar.push(suffix);
    sidecar.into()
}

fn replayed_ids(log: &SqliteEventLog) -> Vec<EventId> {
    log.replay(None)
        .expect("replay events")
        .into_iter()
        .map(|(event_id, _)| event_id)
        .collect()
}

fn heartbeat(timestamp: i64) -> DaemonEvent {
    DaemonEvent::AdapterHeartbeat(AdapterHeartbeat {
        agent_session_id: None,
        worktree_id: WorktreeId::new("boundary-test-worktree"),
        harness: AdapterRef {
            adapter_id: "boundary-test".to_owned(),
            adapter_kind: AdapterKind::HarnessSim,
        },
        capabilities: AdapterCapabilities::default(),
        sent_at: Timestamp(timestamp),
        daemon_generation_id: None,
        token_usage: None,
        queued_event_count: 0,
        degraded: None,
    })
}
