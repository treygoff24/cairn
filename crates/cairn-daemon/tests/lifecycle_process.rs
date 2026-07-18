#![cfg(unix)]

use std::collections::BTreeSet;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use cairn_daemon::{
    DaemonHealth, DaemonLifecycle, DaemonLifecycleError, DaemonScope, DaemonStartReason,
    DaemonStartup,
};
use cairn_daemon_client::{DaemonClientConfig, DaemonIdentity, LocalDaemonClient};
use cairn_daemon_client::{DaemonClientRequest, DaemonClientResponse};
use cairn_identity::WorktreeIdentity;
use cairn_types::{ConfigHash, DaemonGenerationId, ProtocolVersion, WorktreeId};
use rusqlite::Connection;
use tempfile::TempDir;

const TEST_HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const TEST_HASH_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const CHILD_MODE: &str = "CAIRN_LIFECYCLE_CHILD_MODE";
const CHILD_RUNTIME_ROOT: &str = "CAIRN_LIFECYCLE_RUNTIME_ROOT";
const CHILD_READY: &str = "CAIRN_LIFECYCLE_READY";
const CHILD_RELEASE: &str = "CAIRN_LIFECYCLE_RELEASE";
const CHILD_RESULT: &str = "CAIRN_LIFECYCLE_RESULT";

#[test]
fn lease_recovery_matrix_covers_live_shutdown_orphan_stale_and_missing() {
    let temp = short_tempdir("cairn-lease-matrix");

    let graceful_root = temp.path().join("graceful");
    let graceful = lifecycle(&graceful_root, Duration::from_secs(60));
    let mut first = expect_started(
        graceful
            .start_or_attach()
            .expect("fresh daemon should start"),
    );
    assert_eq!(first.generation_id(), DaemonGenerationId(1));
    assert_eq!(first.start_reason(), DaemonStartReason::Fresh);
    first.shutdown().expect("fresh daemon should shut down");
    let mut restarted = expect_started(
        graceful
            .start_or_attach()
            .expect("shutdown daemon should restart"),
    );
    assert_eq!(restarted.generation_id(), DaemonGenerationId(2));
    assert_eq!(
        restarted.start_reason(),
        DaemonStartReason::RestartedAfterShutdown
    );
    restarted
        .shutdown()
        .expect("restarted daemon should shut down");

    let live_root = temp.path().join("live");
    let live_ready = temp.path().join("live-ready");
    let live_release = temp.path().join("live-release");
    let live_result = temp.path().join("live-result");
    let live_child =
        spawn_lifecycle_child("hold", &live_root, &live_ready, &live_release, &live_result);
    fs::write(&live_ready, b"go").expect("live child should be released");
    wait_for_path(&live_result, Duration::from_secs(10));
    let attached = lifecycle(&live_root, Duration::from_secs(60))
        .start_or_attach()
        .expect("second process should attach to live daemon");
    assert!(attached.is_attached());
    assert_eq!(attached.status().generation_id, DaemonGenerationId(1));
    assert_eq!(attached.status().health, DaemonHealth::Healthy);
    fs::write(&live_release, b"stop").expect("live owner should be released");
    assert_child_succeeded(live_child);

    let orphan_root = temp.path().join("orphan");
    crash_generation(&orphan_root, temp.path().join("orphan-ready"));
    let orphan_lifecycle = lifecycle(&orphan_root, Duration::from_secs(60));
    let mut orphan = expect_started(
        orphan_lifecycle
            .start_or_attach()
            .expect("fresh unlocked heartbeat should recover"),
    );
    assert_eq!(orphan.generation_id(), DaemonGenerationId(2));
    assert_eq!(
        orphan.start_reason(),
        DaemonStartReason::RecoveredOrphanedLease
    );
    orphan.shutdown().expect("orphan recovery should shut down");

    let stale_root = temp.path().join("stale");
    crash_generation(&stale_root, temp.path().join("stale-ready"));
    let stale_lifecycle = lifecycle(&stale_root, Duration::ZERO);
    let mut stale = expect_started(
        stale_lifecycle
            .start_or_attach()
            .expect("expired unlocked heartbeat should recover"),
    );
    assert_eq!(stale.generation_id(), DaemonGenerationId(2));
    assert_eq!(stale.start_reason(), DaemonStartReason::RecoveredStaleLease);
    stale.shutdown().expect("stale recovery should shut down");

    let missing_root = temp.path().join("missing");
    crash_generation(&missing_root, temp.path().join("missing-ready"));
    let missing_lifecycle = lifecycle(&missing_root, Duration::from_secs(60));
    fs::rename(
        missing_lifecycle.paths().heartbeat_file(),
        missing_lifecycle
            .paths()
            .heartbeat_file()
            .with_extension("absent"),
    )
    .expect("heartbeat should be hidden to simulate a crash before heartbeat persistence");
    let mut missing = expect_started(
        missing_lifecycle
            .start_or_attach()
            .expect("missing heartbeat should recover"),
    );
    assert_eq!(missing.generation_id(), DaemonGenerationId(2));
    assert_eq!(
        missing.start_reason(),
        DaemonStartReason::RecoveredStaleLease
    );
    missing
        .shutdown()
        .expect("missing recovery should shut down");
}

#[test]
fn held_lock_with_mismatched_heartbeat_stays_unavailable() {
    let temp = short_tempdir("cairn-held-mismatch");
    let runtime_root = temp.path().join("runtime");
    let ready = temp.path().join("ready");
    let release = temp.path().join("release");
    let result = temp.path().join("result");
    let child = spawn_lifecycle_child("hold", &runtime_root, &ready, &release, &result);
    fs::write(&ready, b"go").expect("owner should start");
    wait_for_path(&result, Duration::from_secs(10));

    let contender = lifecycle(&runtime_root, Duration::from_secs(60))
        .with_timing(Duration::from_secs(60), Duration::from_millis(50));
    let mut heartbeat: serde_json::Value = serde_json::from_slice(
        &fs::read(contender.paths().heartbeat_file()).expect("read heartbeat"),
    )
    .expect("decode heartbeat");
    heartbeat["lease_token"] = serde_json::Value::String("mismatched-token".to_owned());
    fs::write(
        contender.paths().heartbeat_file(),
        serde_json::to_vec(&heartbeat).expect("encode mismatched heartbeat"),
    )
    .expect("replace heartbeat");
    let error = contender
        .start_or_attach()
        .expect_err("held lock with mismatched heartbeat must stay unavailable");
    assert!(matches!(
        error,
        DaemonLifecycleError::LeaseHeldWithUnhealthyStatus {
            health: DaemonHealth::HeartbeatMismatch,
            ..
        }
    ));

    fs::write(&release, b"stop").expect("owner should stop");
    assert_child_succeeded(child);
}

#[test]
fn malformed_lease_and_heartbeat_json_are_explicit_errors() {
    let temp = short_tempdir("cairn-malformed-state");

    let lease_root = temp.path().join("lease");
    let lease_lifecycle = lifecycle(&lease_root, Duration::from_secs(60));
    let mut owner = expect_started(
        lease_lifecycle
            .start_or_attach()
            .expect("lease fixture should start"),
    );
    owner.shutdown().expect("lease fixture should stop");
    Connection::open(lease_lifecycle.paths().lease_db())
        .expect("open lease database")
        .execute("UPDATE daemon_lease SET payload = ?1", ["{not-valid-json"])
        .expect("corrupt lease JSON");
    assert!(matches!(
        lease_lifecycle.status(),
        Err(DaemonLifecycleError::Json { .. })
    ));

    let heartbeat_root = temp.path().join("heartbeat");
    let ready = temp.path().join("heartbeat-ready");
    let release = temp.path().join("heartbeat-release");
    let result = temp.path().join("heartbeat-result");
    let child = spawn_lifecycle_child("hold", &heartbeat_root, &ready, &release, &result);
    fs::write(&ready, b"go").expect("heartbeat owner should start");
    wait_for_path(&result, Duration::from_secs(10));
    let heartbeat_lifecycle = lifecycle(&heartbeat_root, Duration::from_secs(60));
    fs::write(heartbeat_lifecycle.paths().heartbeat_file(), b"{").expect("corrupt heartbeat JSON");
    assert!(matches!(
        heartbeat_lifecycle.start_or_attach(),
        Err(DaemonLifecycleError::Json { .. })
    ));
    fs::write(&release, b"stop").expect("heartbeat owner should stop");
    assert_child_succeeded(child);
}

#[test]
fn identity_partitions_reach_the_matching_daemon_end_to_end() {
    let temp = short_tempdir("cairn-partitions");
    let runtime_root = temp.path().join("runtime");
    let first_root = temp.path().join("worktree-a");
    let second_root = temp.path().join("worktree-b");
    fs::create_dir_all(&first_root).expect("first worktree should be created");
    fs::create_dir_all(&second_root).expect("second worktree should be created");

    let identities = [
        identity(&first_root, TEST_HASH_A, ProtocolVersion(1)),
        identity(&first_root, TEST_HASH_B, ProtocolVersion(1)),
        identity(&first_root, TEST_HASH_A, ProtocolVersion(2)),
        identity(&second_root, TEST_HASH_A, ProtocolVersion(1)),
    ];
    let mut clients = Vec::new();
    let mut servers = Vec::new();

    for identity in identities {
        let lifecycle = DaemonLifecycle::new(DaemonScope::from_identity(&identity), &runtime_root)
            .with_timing(Duration::from_secs(5), Duration::from_millis(500));
        let daemon = expect_started(
            lifecycle
                .start_or_attach()
                .expect("partition daemon should start"),
        );
        let mut config = DaemonClientConfig::new(&runtime_root);
        config.request_timeout = Duration::from_secs(1);
        clients.push(
            LocalDaemonClient::from_identity(DaemonIdentity::from_worktree(identity), config)
                .expect("partition client should build"),
        );
        servers.push(
            daemon
                .into_socket_server()
                .expect("partition socket should bind"),
        );
    }

    assert_eq!(
        clients
            .iter()
            .map(|client| client.socket().path().to_path_buf())
            .collect::<BTreeSet<_>>()
            .len(),
        4,
        "worktree, config hash, and protocol version must each partition sockets"
    );

    let expected_storage = servers
        .iter()
        .map(|server| server.paths().runtime_dir().to_path_buf())
        .collect::<BTreeSet<_>>();
    assert_eq!(expected_storage.len(), 4);
    let joins = servers
        .into_iter()
        .map(|mut server| {
            thread::spawn(move || {
                server.serve_one().expect("status should be served");
                server.shutdown().expect("partition daemon should stop");
            })
        })
        .collect::<Vec<_>>();
    let reports = clients
        .iter()
        .map(|client| client.request_status().expect("status should round trip"))
        .collect::<Vec<_>>();
    for join in joins {
        join.join().expect("partition server should finish");
    }

    assert_eq!(
        reports
            .iter()
            .filter_map(|report| report.storage_path.clone())
            .collect::<BTreeSet<_>>(),
        expected_storage
    );
    assert!(
        reports
            .iter()
            .all(|report| report.generation_id.as_deref() == Some("1"))
    );
}

#[test]
fn forged_client_protocol_version_gets_explicit_degraded_notice() {
    use std::os::unix::net::UnixStream;

    let temp = short_tempdir("cairn-protocol-mismatch");
    let runtime_root = temp.path().join("runtime");
    let worktree_root = temp.path().join("worktree");
    fs::create_dir_all(&worktree_root).expect("worktree should be created");
    let identity = identity(&worktree_root, TEST_HASH_A, ProtocolVersion(1));
    let lifecycle = DaemonLifecycle::new(DaemonScope::from_identity(&identity), &runtime_root)
        .with_timing(Duration::from_secs(5), Duration::from_millis(500));
    let server = expect_started(lifecycle.start_or_attach().expect("daemon should start"))
        .into_socket_server()
        .expect("daemon socket should bind");
    let socket_path = server.paths().socket_file().to_path_buf();
    let join = thread::spawn(move || {
        let mut server = server;
        let outcome = server.serve_one().expect("forged request should be served");
        server.shutdown().expect("daemon should stop");
        outcome
    });

    let mut hello = DaemonIdentity::from_worktree(identity).hello();
    hello.protocol_version = ProtocolVersion(2);
    let request = DaemonClientRequest::Status { hello };
    let mut stream = UnixStream::connect(socket_path).expect("connect forged client");
    serde_json::to_writer(&mut stream, &request).expect("write forged request");
    stream.write_all(b"\n").expect("finish forged request");
    let mut response = String::new();
    BufReader::new(stream)
        .read_line(&mut response)
        .expect("read degraded response");

    assert_eq!(
        serde_json::from_str::<DaemonClientResponse>(&response).expect("decode response"),
        DaemonClientResponse::Degraded {
            reason: "client protocol version does not match daemon scope".to_owned(),
        }
    );
    assert_eq!(
        join.join().expect("server should finish"),
        cairn_daemon::DaemonServerOutcome::Degraded
    );
}

#[test]
fn lifecycle_subprocess_helper() {
    let Ok(mode) = std::env::var(CHILD_MODE) else {
        return;
    };
    let runtime_root = PathBuf::from(required_env(CHILD_RUNTIME_ROOT));
    let ready = PathBuf::from(required_env(CHILD_READY));
    let release = PathBuf::from(required_env(CHILD_RELEASE));
    let result = PathBuf::from(required_env(CHILD_RESULT));
    wait_for_path(&ready, Duration::from_secs(10));

    let startup = lifecycle(&runtime_root, Duration::from_secs(60))
        .start_or_attach()
        .expect("child should start or attach");
    let outcome = LaunchOutcome {
        started: startup.is_started(),
        generation: startup.status().generation_id.as_u64(),
    };
    fs::write(
        &result,
        format!("{}:{}", u8::from(outcome.started), outcome.generation),
    )
    .expect("child outcome should be written");

    match mode.as_str() {
        "crash" => std::process::exit(0),
        "hold" | "race" if startup.is_started() => {
            wait_for_path(&release, Duration::from_secs(10));
            let mut daemon = expect_started(startup);
            daemon.shutdown().expect("child daemon should shut down");
        }
        "hold" | "race" => {}
        other => panic!("unknown child mode: {other}"),
    }
}

#[derive(Debug, Clone, Copy)]
struct LaunchOutcome {
    started: bool,
    generation: u64,
}

fn short_tempdir(prefix: &str) -> TempDir {
    tempfile::Builder::new()
        .prefix(prefix)
        .tempdir_in("/tmp")
        .expect("short temp directory should be created")
}

fn config_hash(raw: &str) -> ConfigHash {
    ConfigHash::from_hex(raw)
}

fn lifecycle(runtime_root: &Path, stale_after: Duration) -> DaemonLifecycle {
    DaemonLifecycle::new(
        DaemonScope::new(
            runtime_root.to_path_buf(),
            WorktreeId::new("process-test-worktree"),
            config_hash(TEST_HASH_A),
            ProtocolVersion(1),
        ),
        runtime_root,
    )
    .with_timing(stale_after, Duration::from_secs(2))
}

fn identity(root: &Path, hash: &str, protocol: ProtocolVersion) -> WorktreeIdentity {
    WorktreeIdentity::compute(root, config_hash(hash), protocol)
        .expect("worktree identity should compute")
}

fn expect_started(startup: DaemonStartup) -> cairn_daemon::RunningDaemon {
    match startup {
        DaemonStartup::Started(daemon) => daemon,
        DaemonStartup::Attached(attachment) => {
            panic!("expected writer, attached to {:?}", attachment.status)
        }
    }
}

fn spawn_lifecycle_child(
    mode: &str,
    runtime_root: &Path,
    ready: &Path,
    release: &Path,
    result: &Path,
) -> LifecycleChild {
    LifecycleChild(Some(
        Command::new(std::env::current_exe().expect("test executable should resolve"))
            .args(["--exact", "lifecycle_subprocess_helper", "--nocapture"])
            .env(CHILD_MODE, mode)
            .env(CHILD_RUNTIME_ROOT, runtime_root)
            .env(CHILD_READY, ready)
            .env(CHILD_RELEASE, release)
            .env(CHILD_RESULT, result)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("lifecycle child should spawn"),
    ))
}

fn crash_generation(runtime_root: &Path, ready: PathBuf) {
    let release = ready.with_extension("unused-release");
    let result = ready.with_extension("result");
    let child = spawn_lifecycle_child("crash", runtime_root, &ready, &release, &result);
    fs::write(&ready, b"go").expect("crash child should be released");
    wait_for_path(&result, Duration::from_secs(10));
    assert_child_succeeded(child);
}

fn assert_child_succeeded(mut child: LifecycleChild) {
    let output = child
        .0
        .take()
        .expect("lifecycle child should be present")
        .wait_with_output()
        .expect("lifecycle child should be collected");
    assert!(
        output.status.success(),
        "lifecycle child failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

struct LifecycleChild(Option<Child>);

impl Drop for LifecycleChild {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn wait_for_path(path: &Path, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while !path.exists() {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {}",
            path.display()
        );
        thread::sleep(Duration::from_millis(2));
    }
}

fn required_env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("missing required child env {name}"))
}
