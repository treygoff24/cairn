//! `cairn-daemon` — the single-instance daemon lifecycle.
//!
//! This crate owns the Phase 1 process-lifecycle substrate: one writer daemon per
//! worktree identity, fenced by an OS-level exclusive lock, a SQLite-backed lease
//! DB, a heartbeat file, and a monotonic daemon generation ID. It deliberately
//! does **not** claim hook enforcement or stale-edit arbitration; those arrive
//! when the later ledgers and adapter capabilities are wired in. The public API
//! here is the reusable surface that the CLI, daemon client, and harness simulator
//! can call to start-or-attach without duplicating split-brain logic.

use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, BufRead, BufReader, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cairn_config::{CairnConfig, ConfigError};
use cairn_daemon_client::{
    DaemonClientHello, DaemonClientRequest, DaemonClientResponse, DaemonIdentity,
    DaemonStatusReport,
};
use cairn_identity::{IdentityError, WorktreeIdentity};
use cairn_protocol::{Confidence, DaemonDecision};
use cairn_types::{ConfigHash, DaemonEventKind, ProtocolVersion, WorktreeId};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const LEASE_SCHEMA_VERSION: u32 = 1;
const LEASE_RECORD_ID: i64 = 1;
const LEASE_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS daemon_lease (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    payload TEXT NOT NULL
);
";
const DEFAULT_STALE_AFTER: Duration = Duration::from_secs(5);
const DEFAULT_ATTACH_WAIT: Duration = Duration::from_millis(250);
const ATTACH_POLL_INTERVAL: Duration = Duration::from_millis(5);

static NEXT_TOKEN_COUNTER: AtomicU64 = AtomicU64::new(1);
static PROCESS_WRITER_LOCKS: OnceLock<Mutex<BTreeSet<PathBuf>>> = OnceLock::new();

/// Monotonic daemon generation for one [`DaemonScope`].
///
/// A new writer generation is minted every time a process obtains the exclusive
/// writer lease, including restarts after a graceful shutdown and recovery after
/// an abandoned/stale lease.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DaemonGenerationId(pub u64);

impl DaemonGenerationId {
    /// Returns the raw generation number.
    #[must_use]
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

/// Non-secret token proving which daemon generation owns the writer lease.
///
/// The token is a fencing value, not an authentication credential. It lets a
/// daemon notice that its lease record was replaced and shut down rather than
/// continuing as a split-brain writer.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LeaseToken(String);

impl LeaseToken {
    fn fresh(now_unix_ns: i64) -> Self {
        let counter = NEXT_TOKEN_COUNTER.fetch_add(1, Ordering::Relaxed);
        Self(format!("{}-{now_unix_ns}-{counter}", process::id()))
    }

    /// Borrows the token string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Identity partition for one per-worktree daemon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonScope {
    canonical_root: PathBuf,
    worktree_id: WorktreeId,
    config_hash: ConfigHash,
    protocol_version: ProtocolVersion,
    socket_key: Option<String>,
}

impl DaemonScope {
    /// Builds a daemon scope from the frozen identity components.
    #[must_use]
    pub fn new(
        canonical_root: PathBuf,
        worktree_id: WorktreeId,
        config_hash: ConfigHash,
        protocol_version: ProtocolVersion,
    ) -> Self {
        Self {
            canonical_root,
            worktree_id,
            config_hash,
            protocol_version,
            socket_key: None,
        }
    }

    /// Converts a computed worktree identity into a daemon scope.
    #[must_use]
    pub fn from_identity(identity: &WorktreeIdentity) -> Self {
        let mut scope = Self::new(
            identity.canonical_root.clone(),
            identity.worktree_id.clone(),
            identity.config_hash.clone(),
            identity.protocol_version,
        );
        scope.socket_key = Some(
            DaemonIdentity::from_worktree(identity.clone())
                .socket_key()
                .to_owned(),
        );
        scope
    }

    /// Canonical root path covered by this daemon.
    #[must_use]
    pub fn canonical_root(&self) -> &Path {
        &self.canonical_root
    }

    /// Worktree ID partitioning the daemon and event log.
    #[must_use]
    pub fn worktree_id(&self) -> &WorktreeId {
        &self.worktree_id
    }

    /// Effective config hash included in the daemon identity.
    #[must_use]
    pub fn config_hash(&self) -> &ConfigHash {
        &self.config_hash
    }

    /// Wire protocol version this daemon speaks.
    #[must_use]
    pub fn protocol_version(&self) -> ProtocolVersion {
        self.protocol_version
    }

    fn storage_key(&self) -> String {
        format!(
            "wt-{}-cfg-{}-p{}",
            sanitize_path_segment(self.worktree_id.as_str()),
            self.config_hash.as_hex(),
            self.protocol_version.0
        )
    }

    fn socket_key(&self) -> Option<&str> {
        self.socket_key.as_deref()
    }
}

impl From<&WorktreeIdentity> for DaemonScope {
    fn from(identity: &WorktreeIdentity) -> Self {
        Self::from_identity(identity)
    }
}

/// Runtime paths used by one daemon scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonPaths {
    runtime_dir: PathBuf,
    lock_file: PathBuf,
    lease_db: PathBuf,
    heartbeat_file: PathBuf,
    socket_file: PathBuf,
}

impl DaemonPaths {
    /// Derives the runtime directory and file paths for one daemon scope.
    #[must_use]
    pub fn for_scope(runtime_root: impl AsRef<Path>, scope: &DaemonScope) -> Self {
        let runtime_root = runtime_root.as_ref();
        let runtime_dir = runtime_root.join(scope.storage_key());
        let socket_file = scope
            .socket_key()
            .map(|socket_key| runtime_root.join(format!("c-{socket_key}.sock")))
            .unwrap_or_else(|| runtime_dir.join("daemon.sock"));
        Self {
            lock_file: runtime_dir.join("daemon.lock"),
            lease_db: runtime_dir.join("daemon-lease.sqlite3"),
            heartbeat_file: runtime_dir.join("heartbeat.json"),
            socket_file,
            runtime_dir,
        }
    }

    /// Directory containing all runtime artifacts for this daemon scope.
    #[must_use]
    pub fn runtime_dir(&self) -> &Path {
        &self.runtime_dir
    }

    /// OS-level exclusive lock file. The lock is advisory and held by the writer.
    #[must_use]
    pub fn lock_file(&self) -> &Path {
        &self.lock_file
    }

    /// Durable SQLite lease database path.
    #[must_use]
    pub fn lease_db(&self) -> &Path {
        &self.lease_db
    }

    /// Heartbeat record written by the active daemon generation.
    #[must_use]
    pub fn heartbeat_file(&self) -> &Path {
        &self.heartbeat_file
    }

    /// Socket path where clients attach to the active daemon generation.
    #[must_use]
    pub fn socket_file(&self) -> &Path {
        &self.socket_file
    }
}

/// Returns Cairn's default runtime root for daemon lifecycle artifacts.
///
/// The root is intentionally outside the project tree so the daemon does not
/// dirty the user's worktree.
#[must_use]
pub fn default_runtime_root() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("cairn")
}

/// Start-or-attach lifecycle coordinator for one daemon scope.
#[derive(Debug, Clone)]
pub struct DaemonLifecycle {
    scope: DaemonScope,
    paths: DaemonPaths,
    stale_after: Duration,
    attach_wait: Duration,
}

impl DaemonLifecycle {
    /// Builds a lifecycle coordinator from an already-computed daemon scope.
    #[must_use]
    pub fn new(scope: DaemonScope, runtime_root: impl AsRef<Path>) -> Self {
        let paths = DaemonPaths::for_scope(runtime_root, &scope);
        Self {
            scope,
            paths,
            stale_after: DEFAULT_STALE_AFTER,
            attach_wait: DEFAULT_ATTACH_WAIT,
        }
    }

    /// Builds a lifecycle coordinator by resolving config and worktree identity.
    pub fn for_project(
        root: impl AsRef<Path>,
        config: &CairnConfig,
        runtime_root: impl AsRef<Path>,
    ) -> Result<Self, DaemonLifecycleError> {
        let config_hash = config.config_hash()?;
        let identity =
            WorktreeIdentity::compute(root, config_hash, ProtocolVersion(config.protocol_version))?;
        Ok(Self::new(
            DaemonScope::from_identity(&identity),
            runtime_root,
        ))
    }

    /// Overrides timing knobs. Mainly useful for deterministic tests and doctor
    /// self-tests; production callers should normally use [`DaemonLifecycle::new`].
    #[must_use]
    pub fn with_timing(mut self, stale_after: Duration, attach_wait: Duration) -> Self {
        self.stale_after = stale_after;
        self.attach_wait = attach_wait;
        self
    }

    /// Daemon scope this lifecycle coordinates.
    #[must_use]
    pub fn scope(&self) -> &DaemonScope {
        &self.scope
    }

    /// Runtime paths this lifecycle uses.
    #[must_use]
    pub fn paths(&self) -> &DaemonPaths {
        &self.paths
    }

    /// Starts the daemon if this process wins the writer lease, otherwise attaches
    /// to the active generation.
    pub fn start_or_attach(&self) -> Result<DaemonStartup, DaemonLifecycleError> {
        create_runtime_dir(&self.paths)?;

        if let Some(startup) = self.try_start_with_available_lock()? {
            return Ok(startup);
        }

        self.wait_for_start_or_attach()
    }

    /// Reads current daemon lifecycle status without acquiring the writer lease.
    pub fn status(&self) -> Result<Option<DaemonStatus>, DaemonLifecycleError> {
        read_lease_record(&self.paths)?
            .map(|record| status_from_record(&self.paths, &record, self.stale_after))
            .transpose()
    }

    fn try_acquire_writer_lock(&self) -> Result<Option<File>, DaemonLifecycleError> {
        let mut process_locks = process_writer_locks()
            .lock()
            .map_err(|_| DaemonLifecycleError::ProcessLockTablePoisoned)?;
        if process_locks.contains(self.paths.lock_file()) {
            return Ok(None);
        }

        let lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.paths.lock_file())
            .map_err(|source| DaemonLifecycleError::OpenLock {
                path: self.paths.lock_file().to_path_buf(),
                source,
            })?;

        match lock_file.try_lock() {
            Ok(()) => {
                process_locks.insert(self.paths.lock_file().to_path_buf());
                Ok(Some(lock_file))
            }
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Error(source)) => Err(DaemonLifecycleError::AcquireLock {
                path: self.paths.lock_file().to_path_buf(),
                source,
            }),
        }
    }

    fn try_start_with_available_lock(&self) -> Result<Option<DaemonStartup>, DaemonLifecycleError> {
        let Some(lock_file) = self.try_acquire_writer_lock()? else {
            return Ok(None);
        };

        let startup = self.start_with_writer_lock(lock_file);
        if startup.is_err() {
            let _ = release_process_writer_lock(self.paths.lock_file());
        }
        startup.map(Some)
    }

    fn start_with_writer_lock(
        &self,
        lock_file: File,
    ) -> Result<DaemonStartup, DaemonLifecycleError> {
        let previous_record = read_lease_record(&self.paths)?;
        let now_unix_ns = now_unix_ns()?;
        let generation_id = next_generation(previous_record.as_ref());
        let lease_token = LeaseToken::fresh(now_unix_ns);
        let start_reason =
            start_reason_for(previous_record.as_ref(), &self.paths, self.stale_after)?;

        let record = LeaseRecord::active(
            &self.scope,
            &self.paths,
            generation_id,
            lease_token.clone(),
            process::id(),
            now_unix_ns,
            start_reason,
        );
        write_lease_record(&self.paths, &record)?;
        write_heartbeat(&self.paths, &record, now_unix_ns)?;

        Ok(DaemonStartup::Started(RunningDaemon {
            scope: self.scope.clone(),
            paths: self.paths.clone(),
            lock_file,
            generation_id,
            lease_token,
            writer_pid: process::id(),
            start_reason,
            started_at_unix_ns: now_unix_ns,
            last_heartbeat_unix_ns: Some(now_unix_ns),
            shutdown_at_unix_ns: None,
            shutdown: false,
        }))
    }

    fn wait_for_start_or_attach(&self) -> Result<DaemonStartup, DaemonLifecycleError> {
        let wait_started = std::time::Instant::now();
        let mut last_status = None;

        loop {
            if let Some(status) = self.status()? {
                if status_is_attachable(&status) {
                    return Ok(DaemonStartup::Attached(DaemonAttachment { status }));
                }
                last_status = Some(status);
            }

            if wait_started.elapsed() >= self.attach_wait {
                if let Some(status) = last_status {
                    return Err(DaemonLifecycleError::LeaseHeldWithUnhealthyStatus {
                        lock_path: self.paths.lock_file().to_path_buf(),
                        health: status.health,
                        waited: self.attach_wait,
                    });
                }

                return Err(DaemonLifecycleError::LeaseHeldWithoutRecord {
                    lock_path: self.paths.lock_file().to_path_buf(),
                    waited: self.attach_wait,
                });
            }

            let remaining = self.attach_wait.saturating_sub(wait_started.elapsed());
            thread::sleep(std::cmp::min(ATTACH_POLL_INTERVAL, remaining));

            if let Some(startup) = self.try_start_with_available_lock()? {
                return Ok(startup);
            }
        }
    }
}

fn status_is_attachable(status: &DaemonStatus) -> bool {
    status.state == DaemonLeaseState::Active && status.health == DaemonHealth::Healthy
}

/// Result of [`DaemonLifecycle::start_or_attach`].
#[derive(Debug)]
pub enum DaemonStartup {
    /// This process owns the writer lease and should run the daemon loop.
    Started(RunningDaemon),
    /// Another generation owns the writer lease; attach to it instead.
    Attached(DaemonAttachment),
}

impl DaemonStartup {
    /// Whether this caller became the daemon writer.
    #[must_use]
    pub fn is_started(&self) -> bool {
        matches!(self, Self::Started(_))
    }

    /// Whether this caller attached to an already-running daemon.
    #[must_use]
    pub fn is_attached(&self) -> bool {
        matches!(self, Self::Attached(_))
    }

    /// Current status for the daemon generation this caller should use.
    #[must_use]
    pub fn status(&self) -> DaemonStatus {
        match self {
            Self::Started(daemon) => daemon.status(),
            Self::Attached(attachment) => attachment.status.clone(),
        }
    }
}

/// Handle held by the process that owns the writer lease.
#[derive(Debug)]
pub struct RunningDaemon {
    scope: DaemonScope,
    paths: DaemonPaths,
    lock_file: File,
    generation_id: DaemonGenerationId,
    lease_token: LeaseToken,
    writer_pid: u32,
    start_reason: DaemonStartReason,
    started_at_unix_ns: i64,
    last_heartbeat_unix_ns: Option<i64>,
    shutdown_at_unix_ns: Option<i64>,
    shutdown: bool,
}

impl RunningDaemon {
    /// Daemon scope this writer owns.
    #[must_use]
    pub fn scope(&self) -> &DaemonScope {
        &self.scope
    }

    /// Runtime paths this writer uses.
    #[must_use]
    pub fn paths(&self) -> &DaemonPaths {
        &self.paths
    }

    /// Generation ID for this writer.
    #[must_use]
    pub fn generation_id(&self) -> DaemonGenerationId {
        self.generation_id
    }

    /// Lease token for this writer generation.
    #[must_use]
    pub fn lease_token(&self) -> &LeaseToken {
        &self.lease_token
    }

    /// Why this daemon generation started.
    #[must_use]
    pub fn start_reason(&self) -> DaemonStartReason {
        self.start_reason
    }

    /// Current in-memory status for this writer generation.
    #[must_use]
    pub fn status(&self) -> DaemonStatus {
        DaemonStatus {
            generation_id: self.generation_id,
            lease_token: self.lease_token.clone(),
            writer_pid: self.writer_pid,
            state: if self.shutdown {
                DaemonLeaseState::Shutdown
            } else {
                DaemonLeaseState::Active
            },
            health: if self.shutdown {
                DaemonHealth::Shutdown
            } else {
                DaemonHealth::Healthy
            },
            start_reason: self.start_reason,
            started_at_unix_ns: Some(self.started_at_unix_ns),
            last_heartbeat_unix_ns: self.last_heartbeat_unix_ns,
            shutdown_at_unix_ns: self.shutdown_at_unix_ns,
            paths: self.paths.clone(),
        }
    }

    /// Refreshes the heartbeat and lease record for this writer.
    pub fn heartbeat(&mut self) -> Result<DaemonStatus, DaemonLifecycleError> {
        self.ensure_active_record()?;
        let now_unix_ns = now_unix_ns()?;
        let mut record = read_required_lease_record(&self.paths)?;
        record.last_heartbeat_unix_ns = Some(now_unix_ns);
        write_lease_record(&self.paths, &record)?;
        write_heartbeat(&self.paths, &record, now_unix_ns)?;
        self.last_heartbeat_unix_ns = Some(now_unix_ns);
        Ok(self.status())
    }

    /// Marks the daemon generation as gracefully shut down and releases the writer
    /// lock. After this returns, another caller may start a new generation.
    pub fn shutdown(&mut self) -> Result<DaemonStatus, DaemonLifecycleError> {
        if self.shutdown {
            return Ok(self.status());
        }

        self.ensure_active_record()?;
        let now_unix_ns = now_unix_ns()?;
        let mut record = read_required_lease_record(&self.paths)?;
        record.state = DaemonLeaseState::Shutdown;
        record.shutdown_at_unix_ns = Some(now_unix_ns);
        write_lease_record(&self.paths, &record)?;
        remove_heartbeat_if_present(&self.paths)?;
        remove_socket_if_present(&self.paths)?;
        self.release_writer_lock()?;
        self.shutdown_at_unix_ns = Some(now_unix_ns);
        self.shutdown = true;
        Ok(self.status())
    }

    /// Binds the daemon's Unix socket and returns a JSON-line server compatible
    /// with `cairn-daemon-client`.
    ///
    /// The returned server owns this writer handle. Dropping it runs the same
    /// graceful shutdown path as dropping [`RunningDaemon`].
    pub fn into_socket_server(mut self) -> Result<DaemonSocketServer, DaemonLifecycleError> {
        self.heartbeat()?;
        bind_socket_server(self)
    }

    fn ensure_active_record(&self) -> Result<(), DaemonLifecycleError> {
        let record = read_required_lease_record(&self.paths)?;
        if record.generation_id == self.generation_id
            && record.lease_token == self.lease_token
            && record.state == DaemonLeaseState::Active
        {
            return Ok(());
        }

        Err(DaemonLifecycleError::LeaseLost {
            expected_generation: self.generation_id,
            expected_token: self.lease_token.clone(),
            found_generation: record.generation_id,
            found_token: record.lease_token,
            found_state: record.state,
        })
    }

    fn release_writer_lock(&self) -> Result<(), DaemonLifecycleError> {
        self.lock_file
            .unlock()
            .map_err(|source| DaemonLifecycleError::ReleaseLock {
                path: self.paths.lock_file().to_path_buf(),
                source,
            })?;
        release_process_writer_lock(self.paths.lock_file())?;
        Ok(())
    }
}

impl Drop for RunningDaemon {
    fn drop(&mut self) {
        if self.shutdown {
            return;
        }

        if self.shutdown().is_err() {
            let _ = self.release_writer_lock();
            self.shutdown = true;
        }
    }
}

/// Result of serving one daemon-client request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonServerOutcome {
    /// A status request was served.
    Status,
    /// An event request was observed and allowed.
    Event { event_kind: DaemonEventKind },
    /// The request was answered fail-open because it did not match this daemon.
    Degraded,
}

/// Bound Unix-socket server for one running daemon generation.
#[derive(Debug)]
pub struct DaemonSocketServer {
    daemon: RunningDaemon,
    #[cfg(unix)]
    listener: std::os::unix::net::UnixListener,
}

impl DaemonSocketServer {
    /// Runtime paths exposed by the underlying daemon.
    #[must_use]
    pub fn paths(&self) -> &DaemonPaths {
        self.daemon.paths()
    }

    /// Generation served by this socket.
    #[must_use]
    pub fn generation_id(&self) -> DaemonGenerationId {
        self.daemon.generation_id()
    }

    /// Current status for the owned daemon generation.
    #[must_use]
    pub fn status(&self) -> DaemonStatus {
        self.daemon.status()
    }

    /// Serves one daemon-client JSON-line request.
    pub fn serve_one(&mut self) -> Result<DaemonServerOutcome, DaemonLifecycleError> {
        serve_one_request(self)
    }

    /// Gracefully shuts down the underlying daemon generation.
    pub fn shutdown(&mut self) -> Result<DaemonStatus, DaemonLifecycleError> {
        self.daemon.shutdown()
    }
}

/// Attachment to an existing daemon generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonAttachment {
    /// Status for the daemon generation to attach to.
    pub status: DaemonStatus,
}

/// Status visible to CLI, clients, and harness fixtures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonStatus {
    /// Active or most recently recorded generation.
    pub generation_id: DaemonGenerationId,
    /// Writer lease token for the recorded generation.
    pub lease_token: LeaseToken,
    /// OS process ID recorded by the writer.
    pub writer_pid: u32,
    /// Lease lifecycle state.
    pub state: DaemonLeaseState,
    /// Heartbeat-derived health.
    pub health: DaemonHealth,
    /// Why this generation started.
    pub start_reason: DaemonStartReason,
    /// Generation start timestamp, if loaded from the lease record.
    pub started_at_unix_ns: Option<i64>,
    /// Last heartbeat timestamp, if present.
    pub last_heartbeat_unix_ns: Option<i64>,
    /// Graceful shutdown timestamp, if present.
    pub shutdown_at_unix_ns: Option<i64>,
    /// Paths for attach/doctor output.
    pub paths: DaemonPaths,
}

/// Stored lifecycle state for a daemon lease.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DaemonLeaseState {
    /// A daemon generation owns or claimed the writer lease.
    Active,
    /// The daemon generation released the writer lease gracefully.
    Shutdown,
}

/// Why a daemon writer generation was minted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DaemonStartReason {
    /// No prior lease record existed.
    Fresh,
    /// A prior generation shut down cleanly.
    RestartedAfterShutdown,
    /// A prior active generation left a stale or missing heartbeat and no OS lock.
    RecoveredStaleLease,
    /// A prior active generation had a fresh heartbeat record but no OS lock.
    RecoveredOrphanedLease,
}

/// Heartbeat-derived daemon health.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonHealth {
    /// Active lease and fresh matching heartbeat.
    Healthy,
    /// Active lease with no heartbeat file.
    MissingHeartbeat,
    /// Active lease whose heartbeat file belongs to another generation/token.
    HeartbeatMismatch,
    /// Active lease with a matching heartbeat older than the configured threshold.
    StaleHeartbeat { age: Duration },
    /// Lease was gracefully shut down.
    Shutdown,
}

/// Errors produced by daemon lifecycle coordination.
#[derive(Debug, Error)]
pub enum DaemonLifecycleError {
    /// Config hashing failed while building a project lifecycle.
    #[error("config error: {0}")]
    Config(#[from] ConfigError),

    /// Worktree identity computation failed.
    #[error("identity error: {0}")]
    Identity(#[from] IdentityError),

    /// Runtime directory creation failed.
    #[error("failed to create daemon runtime directory `{path}`: {source}")]
    CreateRuntimeDir { path: PathBuf, source: io::Error },

    /// Lock file open failed.
    #[error("failed to open daemon lock file `{path}`: {source}")]
    OpenLock { path: PathBuf, source: io::Error },

    /// Writer lock acquisition failed for reasons other than contention.
    #[error("failed to acquire daemon writer lock `{path}`: {source}")]
    AcquireLock { path: PathBuf, source: io::Error },

    /// Writer lock release failed.
    #[error("failed to release daemon writer lock `{path}`: {source}")]
    ReleaseLock { path: PathBuf, source: io::Error },

    /// Unix socket setup failed.
    #[error("failed to bind daemon socket `{path}`: {source}")]
    BindSocket { path: PathBuf, source: io::Error },

    /// A daemon client connection could not be accepted.
    #[error("failed to accept daemon socket connection `{path}`: {source}")]
    AcceptSocket { path: PathBuf, source: io::Error },

    /// Daemon socket I/O failed.
    #[error("daemon socket I/O at `{path}` failed: {source}")]
    SocketIo { path: PathBuf, source: io::Error },

    /// The current platform cannot serve the Phase 1 Unix socket protocol.
    #[error("Unix-domain daemon sockets are unsupported on this platform")]
    UnsupportedSocketPlatform,

    /// Another process held the lock but did not publish a lease record in time.
    #[error("daemon lock `{lock_path}` stayed held without a lease record after {waited:?}")]
    LeaseHeldWithoutRecord {
        lock_path: PathBuf,
        waited: Duration,
    },

    /// Another process held the lock but only published an unhealthy lifecycle status.
    #[error(
        "daemon lock `{lock_path}` stayed held with unhealthy status {health:?} after {waited:?}"
    )]
    LeaseHeldWithUnhealthyStatus {
        lock_path: PathBuf,
        health: DaemonHealth,
        waited: Duration,
    },

    /// In-process lock table was poisoned by a panic.
    #[error("daemon in-process writer lock table is poisoned")]
    ProcessLockTablePoisoned,

    /// Lease record was required but missing.
    #[error("daemon lease record `{path}` is missing")]
    MissingLeaseRecord { path: PathBuf },

    /// File I/O failed.
    #[error("daemon file I/O at `{path}` failed: {source}")]
    FileIo { path: PathBuf, source: io::Error },

    /// JSON serialization/deserialization failed.
    #[error("daemon JSON at `{path}` failed: {source}")]
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },

    /// SQLite lease database failed.
    #[error("daemon lease DB `{path}` failed: {source}")]
    Sqlite {
        path: PathBuf,
        source: rusqlite::Error,
    },

    /// This writer no longer owns the active lease record.
    #[error(
        "daemon lease lost: expected generation {expected_generation:?} token {expected_token:?}; found generation {found_generation:?} token {found_token:?} state {found_state:?}"
    )]
    LeaseLost {
        expected_generation: DaemonGenerationId,
        expected_token: LeaseToken,
        found_generation: DaemonGenerationId,
        found_token: LeaseToken,
        found_state: DaemonLeaseState,
    },

    /// System clock could not be represented as Unix nanoseconds.
    #[error("system clock is before the Unix epoch")]
    ClockBeforeUnixEpoch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LeaseRecord {
    schema_version: u32,
    scope_key: String,
    generation_id: DaemonGenerationId,
    lease_token: LeaseToken,
    writer_pid: u32,
    socket_path: PathBuf,
    started_at_unix_ns: i64,
    last_heartbeat_unix_ns: Option<i64>,
    shutdown_at_unix_ns: Option<i64>,
    start_reason: DaemonStartReason,
    state: DaemonLeaseState,
}

impl LeaseRecord {
    fn active(
        scope: &DaemonScope,
        paths: &DaemonPaths,
        generation_id: DaemonGenerationId,
        lease_token: LeaseToken,
        writer_pid: u32,
        now_unix_ns: i64,
        start_reason: DaemonStartReason,
    ) -> Self {
        Self {
            schema_version: LEASE_SCHEMA_VERSION,
            scope_key: scope.storage_key(),
            generation_id,
            lease_token,
            writer_pid,
            socket_path: paths.socket_file().to_path_buf(),
            started_at_unix_ns: now_unix_ns,
            last_heartbeat_unix_ns: Some(now_unix_ns),
            shutdown_at_unix_ns: None,
            start_reason,
            state: DaemonLeaseState::Active,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HeartbeatRecord {
    schema_version: u32,
    scope_key: String,
    generation_id: DaemonGenerationId,
    lease_token: LeaseToken,
    writer_pid: u32,
    beat_at_unix_ns: i64,
}

fn create_runtime_dir(paths: &DaemonPaths) -> Result<(), DaemonLifecycleError> {
    fs::create_dir_all(paths.runtime_dir()).map_err(|source| {
        DaemonLifecycleError::CreateRuntimeDir {
            path: paths.runtime_dir().to_path_buf(),
            source,
        }
    })
}

fn process_writer_locks() -> &'static Mutex<BTreeSet<PathBuf>> {
    PROCESS_WRITER_LOCKS.get_or_init(|| Mutex::new(BTreeSet::new()))
}

fn release_process_writer_lock(path: &Path) -> Result<(), DaemonLifecycleError> {
    let mut process_locks = process_writer_locks()
        .lock()
        .map_err(|_| DaemonLifecycleError::ProcessLockTablePoisoned)?;
    process_locks.remove(path);
    Ok(())
}

fn start_reason_for(
    previous_record: Option<&LeaseRecord>,
    paths: &DaemonPaths,
    stale_after: Duration,
) -> Result<DaemonStartReason, DaemonLifecycleError> {
    let Some(record) = previous_record else {
        return Ok(DaemonStartReason::Fresh);
    };

    if record.state == DaemonLeaseState::Shutdown {
        return Ok(DaemonStartReason::RestartedAfterShutdown);
    }

    let status = status_from_record(paths, record, stale_after)?;
    match status.health {
        DaemonHealth::Healthy => Ok(DaemonStartReason::RecoveredOrphanedLease),
        DaemonHealth::MissingHeartbeat
        | DaemonHealth::HeartbeatMismatch
        | DaemonHealth::StaleHeartbeat { .. } => Ok(DaemonStartReason::RecoveredStaleLease),
        DaemonHealth::Shutdown => Ok(DaemonStartReason::RestartedAfterShutdown),
    }
}

fn next_generation(previous_record: Option<&LeaseRecord>) -> DaemonGenerationId {
    let next = previous_record
        .map(|record| record.generation_id.0.saturating_add(1))
        .unwrap_or(1);
    DaemonGenerationId(next)
}

fn status_from_record(
    paths: &DaemonPaths,
    record: &LeaseRecord,
    stale_after: Duration,
) -> Result<DaemonStatus, DaemonLifecycleError> {
    let health = lease_health(paths, record, stale_after)?;
    Ok(DaemonStatus {
        generation_id: record.generation_id,
        lease_token: record.lease_token.clone(),
        writer_pid: record.writer_pid,
        state: record.state,
        health,
        start_reason: record.start_reason,
        started_at_unix_ns: Some(record.started_at_unix_ns),
        last_heartbeat_unix_ns: record.last_heartbeat_unix_ns,
        shutdown_at_unix_ns: record.shutdown_at_unix_ns,
        paths: paths.clone(),
    })
}

fn lease_health(
    paths: &DaemonPaths,
    record: &LeaseRecord,
    stale_after: Duration,
) -> Result<DaemonHealth, DaemonLifecycleError> {
    if record.state == DaemonLeaseState::Shutdown {
        return Ok(DaemonHealth::Shutdown);
    }

    let Some(heartbeat) = read_heartbeat_record(paths)? else {
        return Ok(DaemonHealth::MissingHeartbeat);
    };

    if !heartbeat_matches(record, &heartbeat) {
        return Ok(DaemonHealth::HeartbeatMismatch);
    }

    let now = now_unix_ns()?;
    let age = heartbeat_age(now, heartbeat.beat_at_unix_ns);
    if age > stale_after {
        return Ok(DaemonHealth::StaleHeartbeat { age });
    }

    Ok(DaemonHealth::Healthy)
}

fn heartbeat_matches(record: &LeaseRecord, heartbeat: &HeartbeatRecord) -> bool {
    record.scope_key == heartbeat.scope_key
        && record.generation_id == heartbeat.generation_id
        && record.lease_token == heartbeat.lease_token
        && record.writer_pid == heartbeat.writer_pid
}

fn heartbeat_age(now_unix_ns: i64, beat_unix_ns: i64) -> Duration {
    if now_unix_ns <= beat_unix_ns {
        return Duration::ZERO;
    }

    let age_ns = (now_unix_ns - beat_unix_ns) as u64;
    Duration::from_nanos(age_ns)
}

fn read_required_lease_record(paths: &DaemonPaths) -> Result<LeaseRecord, DaemonLifecycleError> {
    read_lease_record(paths)?.ok_or_else(|| DaemonLifecycleError::MissingLeaseRecord {
        path: paths.lease_db().to_path_buf(),
    })
}

fn read_lease_record(paths: &DaemonPaths) -> Result<Option<LeaseRecord>, DaemonLifecycleError> {
    if !paths.lease_db().exists() {
        return Ok(None);
    }

    let conn = open_lease_db(paths)?;
    let payload = conn
        .query_row(
            "SELECT payload FROM daemon_lease WHERE id = ?1",
            params![LEASE_RECORD_ID],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|source| DaemonLifecycleError::Sqlite {
            path: paths.lease_db().to_path_buf(),
            source,
        })?;

    payload
        .map(|payload| serde_json::from_str(&payload))
        .transpose()
        .map_err(|source| DaemonLifecycleError::Json {
            path: paths.lease_db().to_path_buf(),
            source,
        })
}

fn write_lease_record(
    paths: &DaemonPaths,
    record: &LeaseRecord,
) -> Result<(), DaemonLifecycleError> {
    let conn = open_lease_db(paths)?;
    let payload = serde_json::to_string(record).map_err(|source| DaemonLifecycleError::Json {
        path: paths.lease_db().to_path_buf(),
        source,
    })?;
    conn.execute(
        "INSERT INTO daemon_lease (id, payload)
         VALUES (?1, ?2)
         ON CONFLICT(id) DO UPDATE SET payload = excluded.payload",
        params![LEASE_RECORD_ID, payload],
    )
    .map_err(|source| DaemonLifecycleError::Sqlite {
        path: paths.lease_db().to_path_buf(),
        source,
    })?;

    Ok(())
}

fn open_lease_db(paths: &DaemonPaths) -> Result<Connection, DaemonLifecycleError> {
    create_parent_directory(paths.lease_db())?;
    let conn =
        Connection::open(paths.lease_db()).map_err(|source| DaemonLifecycleError::Sqlite {
            path: paths.lease_db().to_path_buf(),
            source,
        })?;
    conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = FULL;")
        .and_then(|()| conn.execute_batch(LEASE_SCHEMA))
        .map_err(|source| DaemonLifecycleError::Sqlite {
            path: paths.lease_db().to_path_buf(),
            source,
        })?;

    Ok(conn)
}

fn create_parent_directory(path: &Path) -> Result<(), DaemonLifecycleError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|source| DaemonLifecycleError::FileIo {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    Ok(())
}

fn read_heartbeat_record(
    paths: &DaemonPaths,
) -> Result<Option<HeartbeatRecord>, DaemonLifecycleError> {
    read_json_file(paths.heartbeat_file())
}

fn write_heartbeat(
    paths: &DaemonPaths,
    record: &LeaseRecord,
    beat_at_unix_ns: i64,
) -> Result<(), DaemonLifecycleError> {
    let heartbeat = HeartbeatRecord {
        schema_version: LEASE_SCHEMA_VERSION,
        scope_key: record.scope_key.clone(),
        generation_id: record.generation_id,
        lease_token: record.lease_token.clone(),
        writer_pid: record.writer_pid,
        beat_at_unix_ns,
    };
    write_json_file_atomically(paths.heartbeat_file(), &heartbeat)
}

fn remove_heartbeat_if_present(paths: &DaemonPaths) -> Result<(), DaemonLifecycleError> {
    match fs::remove_file(paths.heartbeat_file()) {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == ErrorKind::NotFound => Ok(()),
        Err(source) => Err(DaemonLifecycleError::FileIo {
            path: paths.heartbeat_file().to_path_buf(),
            source,
        }),
    }
}

fn remove_socket_if_present(paths: &DaemonPaths) -> Result<(), DaemonLifecycleError> {
    match fs::remove_file(paths.socket_file()) {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == ErrorKind::NotFound => Ok(()),
        Err(source) => Err(DaemonLifecycleError::SocketIo {
            path: paths.socket_file().to_path_buf(),
            source,
        }),
    }
}

#[cfg(unix)]
fn bind_socket_server(daemon: RunningDaemon) -> Result<DaemonSocketServer, DaemonLifecycleError> {
    create_socket_parent(daemon.paths.socket_file())?;
    remove_socket_if_present(&daemon.paths)?;
    let listener =
        std::os::unix::net::UnixListener::bind(daemon.paths.socket_file()).map_err(|source| {
            DaemonLifecycleError::BindSocket {
                path: daemon.paths.socket_file().to_path_buf(),
                source,
            }
        })?;

    Ok(DaemonSocketServer { daemon, listener })
}

#[cfg(not(unix))]
fn bind_socket_server(_daemon: RunningDaemon) -> Result<DaemonSocketServer, DaemonLifecycleError> {
    Err(DaemonLifecycleError::UnsupportedSocketPlatform)
}

fn create_socket_parent(path: &Path) -> Result<(), DaemonLifecycleError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|source| DaemonLifecycleError::SocketIo {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    Ok(())
}

#[cfg(unix)]
fn serve_one_request(
    server: &mut DaemonSocketServer,
) -> Result<DaemonServerOutcome, DaemonLifecycleError> {
    let (stream, _) =
        server
            .listener
            .accept()
            .map_err(|source| DaemonLifecycleError::AcceptSocket {
                path: server.daemon.paths.socket_file().to_path_buf(),
                source,
            })?;
    handle_client_stream(&mut server.daemon, stream)
}

#[cfg(not(unix))]
fn serve_one_request(
    _server: &mut DaemonSocketServer,
) -> Result<DaemonServerOutcome, DaemonLifecycleError> {
    Err(DaemonLifecycleError::UnsupportedSocketPlatform)
}

#[cfg(unix)]
fn handle_client_stream(
    daemon: &mut RunningDaemon,
    stream: std::os::unix::net::UnixStream,
) -> Result<DaemonServerOutcome, DaemonLifecycleError> {
    let socket_path = daemon.paths.socket_file().to_path_buf();
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    let bytes_read =
        reader
            .read_line(&mut request_line)
            .map_err(|source| DaemonLifecycleError::SocketIo {
                path: socket_path.clone(),
                source,
            })?;
    if bytes_read == 0 {
        return Err(DaemonLifecycleError::SocketIo {
            path: socket_path,
            source: io::Error::new(ErrorKind::UnexpectedEof, "client closed without request"),
        });
    }

    let request: DaemonClientRequest =
        serde_json::from_str(&request_line).map_err(|source| DaemonLifecycleError::Json {
            path: socket_path.clone(),
            source,
        })?;
    let (response, outcome) = response_for_request(daemon, request)?;
    let mut stream = reader.into_inner();
    write_client_response(&mut stream, &response, &socket_path)?;
    Ok(outcome)
}

fn response_for_request(
    daemon: &mut RunningDaemon,
    request: DaemonClientRequest,
) -> Result<(DaemonClientResponse, DaemonServerOutcome), DaemonLifecycleError> {
    let status = daemon.heartbeat()?;
    match request {
        DaemonClientRequest::Status { hello } => {
            if let Err(reason) = validate_client_hello(daemon.scope(), &hello) {
                return Ok(degraded_response(reason));
            }

            Ok((
                DaemonClientResponse::Status(Box::new(status_report_from_status(&status))),
                DaemonServerOutcome::Status,
            ))
        }
        DaemonClientRequest::Event { hello, event } => {
            if let Err(reason) = validate_client_hello(daemon.scope(), &hello) {
                return Ok(degraded_response(reason));
            }

            let event_kind = event.kind();
            Ok((
                DaemonClientResponse::Decision(Box::new(DaemonDecision::allow(
                    Confidence::Verified,
                ))),
                DaemonServerOutcome::Event { event_kind },
            ))
        }
    }
}

fn validate_client_hello(scope: &DaemonScope, hello: &DaemonClientHello) -> Result<(), String> {
    if hello.canonical_root != scope.canonical_root {
        return Err(format!(
            "client canonical root `{}` does not match daemon root `{}`",
            hello.canonical_root.display(),
            scope.canonical_root.display()
        ));
    }
    if hello.worktree_id != scope.worktree_id.as_str() {
        return Err("client worktree id does not match daemon scope".to_owned());
    }
    if hello.config_hash != scope.config_hash.as_hex() {
        return Err("client config hash does not match daemon scope".to_owned());
    }
    if hello.protocol_version != scope.protocol_version.0 {
        return Err("client protocol version does not match daemon scope".to_owned());
    }
    if let Some(socket_key) = scope.socket_key()
        && hello.identity_key != socket_key
    {
        return Err("client daemon identity key does not match daemon socket".to_owned());
    }

    Ok(())
}

fn degraded_response(reason: String) -> (DaemonClientResponse, DaemonServerOutcome) {
    (
        DaemonClientResponse::Degraded { reason },
        DaemonServerOutcome::Degraded,
    )
}

fn status_report_from_status(status: &DaemonStatus) -> DaemonStatusReport {
    DaemonStatusReport {
        generation_id: Some(status.generation_id.as_u64().to_string()),
        storage_path: Some(status.paths.runtime_dir().to_path_buf()),
        degraded_reason: degraded_reason_from_status(status),
    }
}

fn degraded_reason_from_status(status: &DaemonStatus) -> Option<String> {
    match &status.health {
        DaemonHealth::Healthy => None,
        DaemonHealth::MissingHeartbeat => Some("daemon heartbeat is missing".to_owned()),
        DaemonHealth::HeartbeatMismatch => Some("daemon heartbeat does not match lease".to_owned()),
        DaemonHealth::StaleHeartbeat { age } => {
            Some(format!("daemon heartbeat is stale by {age:?}"))
        }
        DaemonHealth::Shutdown => Some("daemon generation is shut down".to_owned()),
    }
}

#[cfg(unix)]
fn write_client_response(
    stream: &mut std::os::unix::net::UnixStream,
    response: &DaemonClientResponse,
    socket_path: &Path,
) -> Result<(), DaemonLifecycleError> {
    serde_json::to_writer(&mut *stream, response).map_err(|source| DaemonLifecycleError::Json {
        path: socket_path.to_path_buf(),
        source,
    })?;
    stream
        .write_all(b"\n")
        .and_then(|()| stream.flush())
        .map_err(|source| DaemonLifecycleError::SocketIo {
            path: socket_path.to_path_buf(),
            source,
        })
}

fn read_json_file<T: for<'de> Deserialize<'de>>(
    path: &Path,
) -> Result<Option<T>, DaemonLifecycleError> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(source) if source.kind() == ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(DaemonLifecycleError::FileIo {
                path: path.to_path_buf(),
                source,
            });
        }
    };

    serde_json::from_reader(file)
        .map(Some)
        .map_err(|source| DaemonLifecycleError::Json {
            path: path.to_path_buf(),
            source,
        })
}

fn write_json_file_atomically<T: Serialize>(
    path: &Path,
    value: &T,
) -> Result<(), DaemonLifecycleError> {
    create_parent_directory(path)?;
    let temp_path = temp_path_for(path);
    let mut file = File::create(&temp_path).map_err(|source| DaemonLifecycleError::FileIo {
        path: temp_path.clone(),
        source,
    })?;

    serde_json::to_writer_pretty(&mut file, value).map_err(|source| {
        DaemonLifecycleError::Json {
            path: temp_path.clone(),
            source,
        }
    })?;
    file.write_all(b"\n")
        .map_err(|source| DaemonLifecycleError::FileIo {
            path: temp_path.clone(),
            source,
        })?;
    file.sync_all()
        .map_err(|source| DaemonLifecycleError::FileIo {
            path: temp_path.clone(),
            source,
        })?;
    drop(file);

    fs::rename(&temp_path, path).map_err(|source| DaemonLifecycleError::FileIo {
        path: path.to_path_buf(),
        source,
    })
}

fn temp_path_for(path: &Path) -> PathBuf {
    let counter = NEXT_TOKEN_COUNTER.fetch_add(1, Ordering::Relaxed);
    let extension = format!("tmp-{}-{counter}", process::id());
    path.with_extension(extension)
}

fn now_unix_ns() -> Result<i64, DaemonLifecycleError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| DaemonLifecycleError::ClockBeforeUnixEpoch)?;
    i64::try_from(duration.as_nanos()).map_err(|_| DaemonLifecycleError::ClockBeforeUnixEpoch)
}

fn sanitize_path_segment(raw: &str) -> String {
    let mut sanitized = String::with_capacity(raw.len());
    for byte in raw.bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' => {
                sanitized.push(char::from(byte));
            }
            _ => sanitized.push_str(&format!("_{byte:02x}")),
        }
    }

    if sanitized.is_empty() {
        "empty".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::{Arc, Barrier};

    use cairn_daemon_client::{DaemonClient, DaemonClientConfig, LocalDaemonClient};
    use cairn_protocol::{AdapterHeartbeat, AdapterKind, AdapterRef};
    use cairn_types::{AdapterCapabilities, Timestamp};

    use super::*;

    const TEST_HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn concurrent_launches_elect_exactly_one_writer() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let lifecycle = Arc::new(test_lifecycle(temp_dir.path()));
        let barrier = Arc::new(Barrier::new(16));
        let mut joins = Vec::new();

        for _ in 0..16 {
            let lifecycle = Arc::clone(&lifecycle);
            let barrier = Arc::clone(&barrier);
            joins.push(std::thread::spawn(move || {
                barrier.wait();
                lifecycle.start_or_attach().expect("start or attach")
            }));
        }

        let mut outcomes = Vec::new();
        for join in joins {
            outcomes.push(join.join().expect("thread finished"));
        }

        let started_count = outcomes
            .iter()
            .filter(|outcome| outcome.is_started())
            .count();
        assert_eq!(started_count, 1);

        let generations = outcomes
            .iter()
            .map(|outcome| outcome.status().generation_id)
            .collect::<BTreeSet<_>>();
        assert_eq!(generations.len(), 1);

        for outcome in &mut outcomes {
            if let DaemonStartup::Started(daemon) = outcome {
                daemon.shutdown().expect("shutdown");
            }
        }
    }

    #[test]
    fn fresh_active_lease_attaches_without_claiming_writer() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let lifecycle = test_lifecycle(temp_dir.path());
        let mut owner = match lifecycle.start_or_attach().expect("start") {
            DaemonStartup::Started(owner) => owner,
            DaemonStartup::Attached(_) => panic!("first caller should own writer lease"),
        };

        let attached = lifecycle.start_or_attach().expect("attach");
        assert!(attached.is_attached());
        assert_eq!(attached.status().generation_id, owner.generation_id());
        assert_eq!(attached.status().health, DaemonHealth::Healthy);

        owner.shutdown().expect("shutdown");
    }

    #[test]
    fn started_status_carries_lease_timestamps() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let lifecycle = test_lifecycle(temp_dir.path());
        let mut owner = match lifecycle.start_or_attach().expect("start") {
            DaemonStartup::Started(owner) => owner,
            DaemonStartup::Attached(_) => panic!("first caller should own writer lease"),
        };

        let started_status = owner.status();
        assert_eq!(started_status.generation_id, owner.generation_id());
        assert_eq!(started_status.health, DaemonHealth::Healthy);
        assert!(started_status.started_at_unix_ns.is_some());
        assert!(started_status.last_heartbeat_unix_ns.is_some());
        assert!(started_status.shutdown_at_unix_ns.is_none());
        assert!(lifecycle.paths().lease_db().exists());
        assert!(lifecycle.paths().heartbeat_file().exists());

        let shutdown_status = owner.shutdown().expect("shutdown");
        assert!(shutdown_status.shutdown_at_unix_ns.is_some());
        assert!(!lifecycle.paths().heartbeat_file().exists());
    }

    #[test]
    fn stale_lease_is_recovered_with_new_generation() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let lifecycle = test_lifecycle(temp_dir.path());
        create_runtime_dir(lifecycle.paths()).expect("runtime dir");

        let stale_beat = now_unix_ns().expect("clock") - 10_000_000_000;
        let record = test_record(
            lifecycle.scope(),
            lifecycle.paths(),
            DaemonGenerationId(7),
            stale_beat,
        );
        write_lease_record(lifecycle.paths(), &record).expect("lease record");
        write_heartbeat(lifecycle.paths(), &record, stale_beat).expect("heartbeat");

        let mut recovered = match lifecycle.start_or_attach().expect("recover") {
            DaemonStartup::Started(owner) => owner,
            DaemonStartup::Attached(_) => panic!("stale unlocked lease should be recovered"),
        };

        assert_eq!(recovered.generation_id(), DaemonGenerationId(8));
        assert_eq!(
            recovered.start_reason(),
            DaemonStartReason::RecoveredStaleLease
        );

        recovered.shutdown().expect("shutdown");
    }

    #[test]
    fn missing_heartbeat_lease_is_recovered_with_new_generation() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let lifecycle = test_lifecycle(temp_dir.path());
        create_runtime_dir(lifecycle.paths()).expect("runtime dir");

        let beat_at = now_unix_ns().expect("clock");
        let record = test_record(
            lifecycle.scope(),
            lifecycle.paths(),
            DaemonGenerationId(3),
            beat_at,
        );
        write_lease_record(lifecycle.paths(), &record).expect("lease record");

        let mut recovered = match lifecycle.start_or_attach().expect("recover") {
            DaemonStartup::Started(owner) => owner,
            DaemonStartup::Attached(_) => panic!("unlocked missing heartbeat should recover"),
        };

        assert_eq!(recovered.generation_id(), DaemonGenerationId(4));
        assert_eq!(
            recovered.start_reason(),
            DaemonStartReason::RecoveredStaleLease
        );

        recovered.shutdown().expect("shutdown");
    }

    #[test]
    fn heartbeat_mismatch_is_reported_in_status() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let lifecycle = test_lifecycle(temp_dir.path());
        create_runtime_dir(lifecycle.paths()).expect("runtime dir");

        let beat_at = now_unix_ns().expect("clock");
        let record = test_record(
            lifecycle.scope(),
            lifecycle.paths(),
            DaemonGenerationId(5),
            beat_at,
        );
        write_lease_record(lifecycle.paths(), &record).expect("lease record");
        write_heartbeat(lifecycle.paths(), &record, beat_at).expect("heartbeat");

        let mut mismatched_record = record;
        mismatched_record.lease_token = LeaseToken("different-token".to_string());
        write_lease_record(lifecycle.paths(), &mismatched_record).expect("mismatched lease record");

        let status = lifecycle
            .status()
            .expect("status should read")
            .expect("lease record should exist");

        assert_eq!(status.health, DaemonHealth::HeartbeatMismatch);
    }

    #[test]
    fn active_writer_reports_lost_lease_when_record_is_replaced() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let lifecycle = test_lifecycle(temp_dir.path());
        let mut owner = match lifecycle.start_or_attach().expect("start") {
            DaemonStartup::Started(owner) => owner,
            DaemonStartup::Attached(_) => panic!("first caller should own writer lease"),
        };

        let replacement = test_record(
            lifecycle.scope(),
            lifecycle.paths(),
            DaemonGenerationId(owner.generation_id().as_u64() + 1),
            now_unix_ns().expect("clock"),
        );
        write_lease_record(lifecycle.paths(), &replacement).expect("replacement lease");

        let error = owner
            .heartbeat()
            .expect_err("writer should notice replaced lease");

        assert!(matches!(
            error,
            DaemonLifecycleError::LeaseLost {
                expected_generation,
                found_generation,
                ..
            } if expected_generation == DaemonGenerationId(1)
                && found_generation == DaemonGenerationId(2)
        ));
    }

    #[test]
    fn graceful_shutdown_allows_next_generation() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let lifecycle = test_lifecycle(temp_dir.path());
        let mut first = match lifecycle.start_or_attach().expect("start first") {
            DaemonStartup::Started(owner) => owner,
            DaemonStartup::Attached(_) => panic!("first caller should own writer lease"),
        };

        let first_generation = first.generation_id();
        let shutdown_status = first.shutdown().expect("shutdown");
        assert_eq!(shutdown_status.state, DaemonLeaseState::Shutdown);
        assert_eq!(shutdown_status.health, DaemonHealth::Shutdown);

        let mut second = match lifecycle.start_or_attach().expect("start second") {
            DaemonStartup::Started(owner) => owner,
            DaemonStartup::Attached(_) => panic!("shutdown lease should allow restart"),
        };

        assert_eq!(
            second.generation_id(),
            DaemonGenerationId(first_generation.as_u64() + 1)
        );
        assert_eq!(
            second.start_reason(),
            DaemonStartReason::RestartedAfterShutdown
        );

        second.shutdown().expect("shutdown");
    }

    #[test]
    fn heartbeat_refreshes_status() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let lifecycle = test_lifecycle(temp_dir.path());
        let mut owner = match lifecycle.start_or_attach().expect("start") {
            DaemonStartup::Started(owner) => owner,
            DaemonStartup::Attached(_) => panic!("first caller should own writer lease"),
        };

        let status = owner.heartbeat().expect("heartbeat");
        assert_eq!(status.health, DaemonHealth::Healthy);
        assert_eq!(status.generation_id, owner.generation_id());
        assert!(lifecycle.paths().heartbeat_file().exists());

        owner.shutdown().expect("shutdown");
    }

    #[cfg(unix)]
    #[test]
    fn identity_scoped_socket_path_matches_daemon_client_locator() {
        let temp_dir = short_socket_tempdir();
        let runtime_root = temp_dir.path().join("runtime");
        let worktree_root = temp_dir.path().join("worktree");
        let identity = test_worktree_identity(&worktree_root);
        let lifecycle = DaemonLifecycle::new(DaemonScope::from_identity(&identity), &runtime_root);
        let client = test_client(identity, &runtime_root);

        assert_eq!(lifecycle.paths().socket_file(), client.socket().path());
    }

    #[cfg(unix)]
    #[test]
    fn socket_server_replies_to_daemon_client_status_request() {
        let temp_dir = short_socket_tempdir();
        let runtime_root = temp_dir.path().join("runtime");
        let worktree_root = temp_dir.path().join("worktree");
        let identity = test_worktree_identity(&worktree_root);
        let lifecycle = DaemonLifecycle::new(DaemonScope::from_identity(&identity), &runtime_root)
            .with_timing(Duration::from_secs(5), Duration::from_millis(500));
        let socket_path = lifecycle.paths().socket_file().to_path_buf();
        let runtime_dir = lifecycle.paths().runtime_dir().to_path_buf();
        let server = start_test_server(&lifecycle);
        let generation_id = server.generation_id();
        let join = std::thread::spawn(move || {
            let mut server = server;
            let outcome = server.serve_one().expect("serve status");
            let shutdown = server.shutdown().expect("shutdown");
            (outcome, shutdown)
        });

        let client = test_client(identity, &runtime_root);
        let report = client.request_status().expect("status report");
        let (outcome, shutdown) = join.join().expect("server thread finished");

        assert_eq!(outcome, DaemonServerOutcome::Status);
        assert_eq!(
            report.generation_id,
            Some(generation_id.as_u64().to_string())
        );
        assert_eq!(report.storage_path, Some(runtime_dir));
        assert_eq!(report.degraded_reason, None);
        assert_eq!(shutdown.health, DaemonHealth::Shutdown);
        assert!(!socket_path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn socket_server_returns_allow_decision_for_events() {
        let temp_dir = short_socket_tempdir();
        let runtime_root = temp_dir.path().join("runtime");
        let worktree_root = temp_dir.path().join("worktree");
        let identity = test_worktree_identity(&worktree_root);
        let lifecycle = DaemonLifecycle::new(DaemonScope::from_identity(&identity), &runtime_root)
            .with_timing(Duration::from_secs(5), Duration::from_millis(500));
        let server = start_test_server(&lifecycle);
        let join = std::thread::spawn(move || {
            let mut server = server;
            let outcome = server.serve_one().expect("serve event");
            server.shutdown().expect("shutdown");
            outcome
        });

        let client = test_client(identity, &runtime_root);
        let decision = client
            .send_event(&adapter_heartbeat_event(&client))
            .expect("event decision");
        let outcome = join.join().expect("server thread finished");

        assert_eq!(decision, DaemonDecision::allow(Confidence::Verified));
        assert_eq!(
            outcome,
            DaemonServerOutcome::Event {
                event_kind: DaemonEventKind::AdapterHeartbeat
            }
        );
    }

    fn test_lifecycle(runtime_root: &Path) -> DaemonLifecycle {
        let scope = DaemonScope::new(
            runtime_root.to_path_buf(),
            WorktreeId::new("test-worktree"),
            ConfigHash::from_hex(TEST_HASH),
            ProtocolVersion(1),
        );
        DaemonLifecycle::new(scope, runtime_root)
            .with_timing(Duration::from_secs(5), Duration::from_millis(500))
    }

    #[cfg(unix)]
    fn short_socket_tempdir() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("cd")
            .tempdir_in("/tmp")
            .expect("short temp dir")
    }

    fn test_worktree_identity(worktree_root: &Path) -> WorktreeIdentity {
        fs::create_dir_all(worktree_root).expect("worktree root");
        WorktreeIdentity::compute(
            worktree_root,
            ConfigHash::from_hex(TEST_HASH),
            ProtocolVersion(1),
        )
        .expect("worktree identity")
    }

    fn test_client(identity: WorktreeIdentity, runtime_root: &Path) -> LocalDaemonClient {
        let mut config = DaemonClientConfig::new(runtime_root);
        config.request_timeout = Duration::from_millis(500);

        LocalDaemonClient::from_identity(DaemonIdentity::from_worktree(identity), config)
            .expect("daemon client")
    }

    #[cfg(unix)]
    fn start_test_server(lifecycle: &DaemonLifecycle) -> DaemonSocketServer {
        match lifecycle.start_or_attach().expect("start daemon") {
            DaemonStartup::Started(daemon) => daemon.into_socket_server().expect("socket server"),
            DaemonStartup::Attached(_) => panic!("test server should own writer lease"),
        }
    }

    fn adapter_heartbeat_event(client: &LocalDaemonClient) -> cairn_protocol::DaemonEvent {
        cairn_protocol::DaemonEvent::AdapterHeartbeat(AdapterHeartbeat {
            session_id: None,
            worktree_id: client.identity().worktree().worktree_id.clone(),
            adapter: AdapterRef {
                adapter_id: "daemon-test".to_owned(),
                adapter_kind: AdapterKind::HarnessSim,
            },
            protocol_version: client.identity().worktree().protocol_version,
            capabilities: AdapterCapabilities::default(),
            sent_at: Timestamp(1),
            daemon_generation_id: None,
            token_usage: None,
            queued_event_count: 0,
            degraded: None,
        })
    }

    fn test_record(
        scope: &DaemonScope,
        paths: &DaemonPaths,
        generation_id: DaemonGenerationId,
        beat_at_unix_ns: i64,
    ) -> LeaseRecord {
        let token = LeaseToken("stale-token".to_string());
        LeaseRecord {
            schema_version: LEASE_SCHEMA_VERSION,
            scope_key: scope.storage_key(),
            generation_id,
            lease_token: token,
            writer_pid: 1,
            socket_path: paths.socket_file().to_path_buf(),
            started_at_unix_ns: beat_at_unix_ns,
            last_heartbeat_unix_ns: Some(beat_at_unix_ns),
            shutdown_at_unix_ns: None,
            start_reason: DaemonStartReason::Fresh,
            state: DaemonLeaseState::Active,
        }
    }
}
