//! `cairn-daemon-client` — the connect-or-launch client used by adapters and the CLI.
//!
//! Sync by design for Phase 1: adapters call this on latency-sensitive hook paths,
//! so this crate does not depend on an async runtime. Every public operation returns
//! a degraded [`DaemonClientError`] instead of panicking; callers fail open and keep
//! the host agent moving when the daemon is unavailable.

use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use cairn_identity::{IdentityError, WorktreeIdentity};
use cairn_protocol::{DaemonDecision, DaemonEvent, DegradedState};
use cairn_types::{ConfigHash, ProtocolVersion, Timestamp};
use serde::{Deserialize, Serialize};

/// Conservative Unix-domain socket path limit for macOS (`sockaddr_un.sun_path`).
///
/// Linux allows 108 bytes, but Cairn targets macOS hook paths first. Socket names
/// are BLAKE3-shortened so the default `/tmp/cairn` directory stays comfortably
/// under this ceiling. Rust's Unix listener requires the path to be strictly
/// shorter than macOS's `sun_path` capacity, leaving one byte for the terminator.
pub const MAX_UNIX_SOCKET_PATH_BYTES: usize = 103;

const SOCKET_HASH_DOMAIN: &[u8] = b"cairn.daemon.socket.v1";
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_millis(10);
const DEFAULT_LAUNCH_TIMEOUT: Duration = Duration::from_millis(500);
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_millis(25);
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(5);

/// Why a client call could not reach a healthy daemon. Never fatal to the host
/// agent: callers treat every variant as "proceed without the daemon's opinion".
#[derive(Debug, thiserror::Error)]
pub enum DaemonClientError {
    /// No daemon is reachable for this worktree and one could not be reached.
    #[error("daemon unavailable: {0}")]
    Unavailable(String),
    /// A launch was attempted but the daemon did not come up.
    #[error("daemon launch failed: {0}")]
    LaunchFailed(String),
    /// The daemon did not respond within the client's deadline.
    #[error("daemon request timed out")]
    Timeout,
}

impl DaemonClientError {
    /// Machine-readable error class for status surfaces and fail-open metrics.
    #[must_use]
    pub fn kind(&self) -> DaemonClientErrorKind {
        match self {
            Self::Unavailable(_) => DaemonClientErrorKind::Unavailable,
            Self::LaunchFailed(_) => DaemonClientErrorKind::LaunchFailed,
            Self::Timeout => DaemonClientErrorKind::Timeout,
        }
    }

    /// All daemon-client failures are degraded fail-open states for adapters.
    ///
    /// Strict-mode policy can choose a harder behavior above this crate, but the
    /// transport client itself never asks a host harness to block because Cairn is
    /// unavailable.
    #[must_use]
    pub fn is_fail_open(&self) -> bool {
        true
    }

    /// Human-readable reason without the enum display prefix.
    #[must_use]
    pub fn degradation_reason(&self) -> String {
        match self {
            Self::Unavailable(reason) | Self::LaunchFailed(reason) => reason.clone(),
            Self::Timeout => "daemon request timed out".to_owned(),
        }
    }

    /// Fully structured degraded state for callers that need machine-readable
    /// fail-open telemetry rather than only an error string.
    #[must_use]
    pub fn degradation(&self, since: Timestamp) -> DaemonClientDegradation {
        DaemonClientDegradation {
            kind: self.kind(),
            reason: self.degradation_reason(),
            fail_open: self.is_fail_open(),
            since,
        }
    }

    /// Converts this transport error into the protocol's explicit degraded state.
    #[must_use]
    pub fn degraded_state(&self, since: Timestamp) -> DegradedState {
        let degradation = self.degradation(since);
        DegradedState {
            reason: degradation.reason,
            fail_open: degradation.fail_open,
            since: degradation.since,
        }
    }

    /// Converts this degraded transport error into the protocol's explicit
    /// fail-open decision shape.
    #[must_use]
    pub fn degraded_decision(&self) -> DaemonDecision {
        let degradation = self.degradation(Timestamp::now());
        DaemonDecision::degraded_allow(degradation.reason, degradation.since)
    }
}

/// Machine-readable daemon-client failure class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DaemonClientErrorKind {
    Unavailable,
    LaunchFailed,
    Timeout,
}

/// Machine-readable fail-open state produced by the sync transport client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonClientDegradation {
    pub kind: DaemonClientErrorKind,
    pub reason: String,
    pub fail_open: bool,
    pub since: Timestamp,
}

/// A client that discovers (or launches) the single daemon for a worktree and ships
/// it events, receiving the daemon's decision in return.
pub trait DaemonClient {
    /// Connect to the running daemon for this worktree, launching it if absent.
    /// Fail-open: a degraded return is not fatal to the host agent.
    fn connect_or_launch(&self) -> Result<(), DaemonClientError>;

    /// Send one event and return the daemon's [`DaemonDecision`], or a degraded
    /// error the caller treats as "allow".
    fn send_event(&self, event: &DaemonEvent) -> Result<DaemonDecision, DaemonClientError>;
}

/// Daemon identity scoped by canonical worktree identity, config hash, and protocol
/// version. The short [`DaemonIdentity::socket_key`] is only a transport locator;
/// downstream state must still reason over the full [`WorktreeIdentity`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonIdentity {
    worktree: WorktreeIdentity,
    socket_key: String,
}

impl DaemonIdentity {
    /// Computes the daemon identity for a root using `cairn-identity`.
    pub fn compute(
        root: impl AsRef<Path>,
        config_hash: ConfigHash,
        protocol_version: ProtocolVersion,
    ) -> Result<Self, DaemonClientError> {
        WorktreeIdentity::compute(root, config_hash, protocol_version)
            .map(Self::from_worktree)
            .map_err(identity_error)
    }

    /// Wraps a precomputed worktree identity.
    #[must_use]
    pub fn from_worktree(worktree: WorktreeIdentity) -> Self {
        let socket_key = socket_key_for_identity(&worktree);
        Self {
            worktree,
            socket_key,
        }
    }

    /// Full identity tuple computed by `cairn-identity`.
    #[must_use]
    pub fn worktree(&self) -> &WorktreeIdentity {
        &self.worktree
    }

    /// Short, stable key used only in the socket filename.
    #[must_use]
    pub fn socket_key(&self) -> &str {
        &self.socket_key
    }

    /// Serializable identity view sent on the daemon wire.
    #[must_use]
    pub fn hello(&self) -> DaemonClientHello {
        DaemonClientHello {
            identity_key: self.socket_key.clone(),
            canonical_root: self.worktree.canonical_root.clone(),
            worktree_id: self.worktree.worktree_id.as_str().to_owned(),
            config_hash: self.worktree.config_hash.as_hex().to_owned(),
            protocol_version: self.worktree.protocol_version.0,
        }
    }
}

/// Short serializable identity header included in every client request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonClientHello {
    pub identity_key: String,
    pub canonical_root: PathBuf,
    pub worktree_id: String,
    pub config_hash: String,
    pub protocol_version: u32,
}

/// Unix socket path derived from a full daemon identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonSocket {
    path: PathBuf,
    identity_key: String,
}

impl DaemonSocket {
    /// Builds a socket path under `socket_dir` for `identity`.
    pub fn for_identity(
        identity: &DaemonIdentity,
        socket_dir: impl Into<PathBuf>,
    ) -> Result<Self, DaemonClientError> {
        let path = socket_path_for_key(socket_dir, identity.socket_key());
        validate_socket_path(&path)?;

        Ok(Self {
            path,
            identity_key: identity.socket_key().to_owned(),
        })
    }

    /// Filesystem path for the Unix socket.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Stable short key shared with [`DaemonIdentity`].
    #[must_use]
    pub fn identity_key(&self) -> &str {
        &self.identity_key
    }
}

/// Derives the daemon Unix-socket path for a socket root and identity key.
#[must_use]
pub fn socket_path_for_key(socket_dir: impl Into<PathBuf>, socket_key: &str) -> PathBuf {
    socket_dir.into().join(format!("c-{socket_key}.sock"))
}

/// Explicit timeout and socket-root settings for the sync client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonClientConfig {
    pub socket_dir: PathBuf,
    pub connect_timeout: Duration,
    pub launch_timeout: Duration,
    pub request_timeout: Duration,
    pub poll_interval: Duration,
}

impl DaemonClientConfig {
    /// Uses `socket_dir` with the default Phase 1 hook-path deadlines.
    #[must_use]
    pub fn new(socket_dir: impl Into<PathBuf>) -> Self {
        Self {
            socket_dir: socket_dir.into(),
            ..Self::default()
        }
    }
}

impl Default for DaemonClientConfig {
    fn default() -> Self {
        Self {
            socket_dir: default_socket_dir(),
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            launch_timeout: DEFAULT_LAUNCH_TIMEOUT,
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
            poll_interval: DEFAULT_POLL_INTERVAL,
        }
    }
}

/// Local reachability state suitable for `cairn status` / harness diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonClientStatus {
    pub identity: DaemonClientHello,
    pub socket: DaemonSocket,
    pub reachability: DaemonReachability,
}

/// Whether the daemon socket can currently be reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonReachability {
    Reachable,
    Degraded { reason: String },
}

/// Daemon-reported runtime state. The daemon crate fills this in once it owns the
/// lease, heartbeat, storage path, and generation ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonStatusReport {
    pub generation_id: Option<String>,
    pub storage_path: Option<PathBuf>,
    pub degraded_reason: Option<String>,
}

/// JSON-line request sent over the Phase 1 Unix-socket client protocol.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DaemonClientRequest {
    Status {
        hello: DaemonClientHello,
    },
    Event {
        hello: DaemonClientHello,
        event: Box<DaemonEvent>,
    },
}

/// JSON-line response returned by the daemon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DaemonClientResponse {
    Decision(Box<DaemonDecision>),
    Status(Box<DaemonStatusReport>),
    Degraded { reason: String },
}

/// Context passed to a launcher implementation.
pub struct DaemonLaunchContext<'a> {
    pub identity: &'a DaemonIdentity,
    pub socket: &'a DaemonSocket,
}

/// Starts a daemon process when no socket is reachable.
pub trait DaemonLauncher: Send + Sync {
    fn launch(&self, context: &DaemonLaunchContext<'_>) -> Result<(), DaemonClientError>;
}

/// Process-based launcher used by production callers.
///
/// By default this invokes the current executable as:
///
/// ```text
/// <current-exe> daemon serve --socket <path>
/// ```
///
/// Tests and harnesses can inject their own [`DaemonLauncher`] to avoid starting a
/// real daemon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandDaemonLauncher {
    program: PathBuf,
    base_args: Vec<OsString>,
}

impl CommandDaemonLauncher {
    /// Creates a launcher with a custom executable and no base arguments.
    #[must_use]
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            base_args: Vec::new(),
        }
    }

    /// Creates a launcher with custom executable and fixed leading arguments.
    #[must_use]
    pub fn with_args<I, S>(program: impl Into<PathBuf>, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        Self {
            program: program.into(),
            base_args: args.into_iter().map(Into::into).collect(),
        }
    }
}

impl Default for CommandDaemonLauncher {
    fn default() -> Self {
        let program = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("cairn"));
        Self::with_args(program, ["daemon", "serve"])
    }
}

impl DaemonLauncher for CommandDaemonLauncher {
    fn launch(&self, context: &DaemonLaunchContext<'_>) -> Result<(), DaemonClientError> {
        let mut command = Command::new(&self.program);
        command
            .args(&self.base_args)
            .arg("--socket")
            .arg(context.socket.path())
            .env("CAIRN_DAEMON_SOCKET", context.socket.path())
            .env(
                "CAIRN_WORKTREE_ROOT",
                &context.identity.worktree().canonical_root,
            )
            .env(
                "CAIRN_WORKTREE_ID",
                context.identity.worktree().worktree_id.as_str(),
            )
            .env(
                "CAIRN_CONFIG_HASH",
                context.identity.worktree().config_hash.as_hex(),
            )
            .env(
                "CAIRN_PROTOCOL_VERSION",
                context.identity.worktree().protocol_version.0.to_string(),
            )
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        command
            .spawn()
            .map(|_| ())
            .map_err(|source| DaemonClientError::LaunchFailed(source.to_string()))
    }
}

/// Concrete sync client for local per-worktree daemons.
pub struct LocalDaemonClient {
    identity: DaemonIdentity,
    socket: DaemonSocket,
    config: DaemonClientConfig,
    launcher: Arc<dyn DaemonLauncher>,
}

impl LocalDaemonClient {
    /// Computes identity from `root` and uses the default process launcher.
    pub fn for_worktree(
        root: impl AsRef<Path>,
        config_hash: ConfigHash,
        protocol_version: ProtocolVersion,
    ) -> Result<Self, DaemonClientError> {
        let identity = DaemonIdentity::compute(root, config_hash, protocol_version)?;
        Self::from_identity(identity, DaemonClientConfig::default())
    }

    /// Uses a precomputed identity and the default process launcher.
    pub fn from_identity(
        identity: DaemonIdentity,
        config: DaemonClientConfig,
    ) -> Result<Self, DaemonClientError> {
        Self::with_launcher(identity, config, Arc::new(CommandDaemonLauncher::default()))
    }

    /// Uses a precomputed identity and caller-supplied launcher.
    pub fn with_launcher(
        identity: DaemonIdentity,
        config: DaemonClientConfig,
        launcher: Arc<dyn DaemonLauncher>,
    ) -> Result<Self, DaemonClientError> {
        let socket = DaemonSocket::for_identity(&identity, &config.socket_dir)?;
        Ok(Self {
            identity,
            socket,
            config,
            launcher,
        })
    }

    /// Full identity used for socket discovery.
    #[must_use]
    pub fn identity(&self) -> &DaemonIdentity {
        &self.identity
    }

    /// Socket selected for this identity.
    #[must_use]
    pub fn socket(&self) -> &DaemonSocket {
        &self.socket
    }

    /// Local status probe that never launches a daemon.
    #[must_use]
    pub fn status(&self) -> DaemonClientStatus {
        let reachability = match self.connect_existing() {
            Ok(()) => DaemonReachability::Reachable,
            Err(error) => DaemonReachability::Degraded {
                reason: error.to_string(),
            },
        };

        DaemonClientStatus {
            identity: self.identity.hello(),
            socket: self.socket.clone(),
            reachability,
        }
    }

    /// Ask a reachable daemon for its runtime status. Does not launch.
    pub fn request_status(&self) -> Result<DaemonStatusReport, DaemonClientError> {
        let request = DaemonClientRequest::Status {
            hello: self.identity.hello(),
        };
        match self.send_request(&request)? {
            DaemonClientResponse::Status(status) => Ok(*status),
            DaemonClientResponse::Degraded { reason } => Ok(DaemonStatusReport {
                generation_id: None,
                storage_path: None,
                degraded_reason: Some(reason),
            }),
            DaemonClientResponse::Decision(_) => Err(DaemonClientError::Unavailable(
                "daemon returned a decision to a status request".to_owned(),
            )),
        }
    }

    fn connect_existing(&self) -> Result<(), DaemonClientError> {
        self.wait_for_existing_socket(
            deadline_after(self.config.connect_timeout),
            self.config.connect_timeout,
        )
        .map(|_| ())
    }

    #[cfg(unix)]
    fn wait_for_existing_socket(
        &self,
        deadline: Instant,
        timeout: Duration,
    ) -> Result<std::os::unix::net::UnixStream, DaemonClientError> {
        loop {
            match self.open_stream() {
                Ok(stream) => return Ok(stream),
                Err(error) if should_keep_polling(&error) && Instant::now() < deadline => {
                    sleep_until_next_poll(deadline, self.config.poll_interval);
                }
                Err(error) => return Err(connect_error(&self.socket, error)),
            }

            if Instant::now() >= deadline {
                return Err(DaemonClientError::Unavailable(format!(
                    "daemon socket `{}` was not reachable within {:?}",
                    self.socket.path().display(),
                    timeout
                )));
            }
        }
    }

    #[cfg(not(unix))]
    fn wait_for_existing_socket(
        &self,
        _deadline: Instant,
        _timeout: Duration,
    ) -> Result<(), DaemonClientError> {
        Err(DaemonClientError::Unavailable(
            "Unix-domain daemon sockets are unsupported on this platform".to_owned(),
        ))
    }

    #[cfg(unix)]
    fn open_stream(&self) -> io::Result<std::os::unix::net::UnixStream> {
        let stream = std::os::unix::net::UnixStream::connect(self.socket.path())?;
        stream.set_read_timeout(Some(self.config.request_timeout))?;
        stream.set_write_timeout(Some(self.config.request_timeout))?;
        Ok(stream)
    }

    fn launch_daemon(&self) -> Result<(), DaemonClientError> {
        if let Some(parent) = self.socket.path().parent() {
            fs::create_dir_all(parent).map_err(|source| {
                DaemonClientError::LaunchFailed(format!(
                    "failed to create socket directory `{}`: {source}",
                    parent.display()
                ))
            })?;
        }

        let context = DaemonLaunchContext {
            identity: &self.identity,
            socket: &self.socket,
        };
        self.launcher.launch(&context)
    }

    fn wait_for_launch(&self) -> Result<(), DaemonClientError> {
        let deadline = deadline_after(self.config.launch_timeout);
        match self.wait_for_existing_socket(deadline, self.config.launch_timeout) {
            Ok(_) => Ok(()),
            Err(DaemonClientError::Unavailable(_)) => {
                Err(DaemonClientError::LaunchFailed(format!(
                    "daemon did not become reachable at `{}` within {:?}",
                    self.socket.path().display(),
                    self.config.launch_timeout
                )))
            }
            Err(error) => Err(error),
        }
    }

    #[cfg(unix)]
    fn send_request(
        &self,
        request: &DaemonClientRequest,
    ) -> Result<DaemonClientResponse, DaemonClientError> {
        let mut stream = self
            .open_stream()
            .map_err(|source| connect_error(&self.socket, source))?;
        write_request(&mut stream, request, &self.socket)?;
        read_response(stream, &self.socket)
    }

    #[cfg(not(unix))]
    fn send_request(
        &self,
        _request: &DaemonClientRequest,
    ) -> Result<DaemonClientResponse, DaemonClientError> {
        Err(DaemonClientError::Unavailable(
            "Unix-domain daemon sockets are unsupported on this platform".to_owned(),
        ))
    }
}

impl fmt::Debug for LocalDaemonClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalDaemonClient")
            .field("identity", &self.identity)
            .field("socket", &self.socket)
            .field("config", &self.config)
            .field("launcher", &"<daemon launcher>")
            .finish()
    }
}

impl DaemonClient for LocalDaemonClient {
    fn connect_or_launch(&self) -> Result<(), DaemonClientError> {
        match self.connect_existing() {
            Ok(()) => Ok(()),
            Err(DaemonClientError::Unavailable(_)) => {
                self.launch_daemon()?;
                self.wait_for_launch()
            }
            Err(error) => Err(error),
        }
    }

    fn send_event(&self, event: &DaemonEvent) -> Result<DaemonDecision, DaemonClientError> {
        match self.send_event_request(event) {
            Ok(decision) => return Ok(decision),
            Err(DaemonClientError::Unavailable(_)) => self.connect_or_launch()?,
            Err(error) => return Err(error),
        }

        self.send_event_request(event)
    }
}

impl LocalDaemonClient {
    fn send_event_request(&self, event: &DaemonEvent) -> Result<DaemonDecision, DaemonClientError> {
        let request = DaemonClientRequest::Event {
            hello: self.identity.hello(),
            event: Box::new(event.clone()),
        };

        match self.send_request(&request)? {
            DaemonClientResponse::Decision(decision) => Ok(*decision),
            DaemonClientResponse::Degraded { reason } => {
                Ok(DaemonDecision::degraded_allow(reason, Timestamp::now()))
            }
            DaemonClientResponse::Status(_) => Err(DaemonClientError::Unavailable(
                "daemon returned status to an event request".to_owned(),
            )),
        }
    }
}

#[cfg(unix)]
fn write_request(
    stream: &mut std::os::unix::net::UnixStream,
    request: &DaemonClientRequest,
    socket: &DaemonSocket,
) -> Result<(), DaemonClientError> {
    let mut payload = serde_json::to_vec(request).map_err(|source| {
        DaemonClientError::Unavailable(format!(
            "failed to encode daemon request for `{}`: {source}",
            socket.path().display()
        ))
    })?;
    payload.push(b'\n');
    stream
        .write_all(&payload)
        .and_then(|()| stream.flush())
        .map_err(|source| request_io_error("write daemon request", socket, source))
}

#[cfg(unix)]
fn read_response(
    stream: std::os::unix::net::UnixStream,
    socket: &DaemonSocket,
) -> Result<DaemonClientResponse, DaemonClientError> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let bytes_read = reader
        .read_line(&mut line)
        .map_err(|source| request_io_error("read daemon response", socket, source))?;

    if bytes_read == 0 {
        return Err(DaemonClientError::Unavailable(format!(
            "daemon closed `{}` without a response",
            socket.path().display()
        )));
    }

    serde_json::from_str(&line).map_err(|source| {
        DaemonClientError::Unavailable(format!(
            "failed to decode daemon response from `{}`: {source}",
            socket.path().display()
        ))
    })
}

fn socket_key_for_identity(identity: &WorktreeIdentity) -> String {
    let mut hasher = blake3::Hasher::new();
    update_hash_segment(&mut hasher, SOCKET_HASH_DOMAIN);
    update_hash_segment(&mut hasher, identity.worktree_id.as_str().as_bytes());
    update_hash_segment(&mut hasher, identity.config_hash.as_hex().as_bytes());
    update_hash_segment(&mut hasher, &identity.protocol_version.0.to_le_bytes());
    update_hash_segment(&mut hasher, &path_bytes(&identity.canonical_root));

    if let Some(git_common_dir) = identity.git_common_dir.as_deref() {
        update_hash_segment(&mut hasher, &path_bytes(git_common_dir));
    } else {
        update_hash_segment(&mut hasher, b"<no-git-common-dir>");
    }

    hasher.finalize().to_hex().chars().take(32).collect()
}

fn update_hash_segment(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&(bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

fn validate_socket_path(path: &Path) -> Result<(), DaemonClientError> {
    let byte_len = socket_path_bytes(path);
    if byte_len <= MAX_UNIX_SOCKET_PATH_BYTES {
        return Ok(());
    }

    Err(DaemonClientError::Unavailable(format!(
        "daemon socket path `{}` is {byte_len} bytes; max supported is {MAX_UNIX_SOCKET_PATH_BYTES}",
        path.display()
    )))
}

fn identity_error(error: IdentityError) -> DaemonClientError {
    DaemonClientError::Unavailable(format!("failed to compute project identity: {error}"))
}

#[cfg(unix)]
fn connect_error(socket: &DaemonSocket, source: io::Error) -> DaemonClientError {
    if is_timeout(&source) {
        return DaemonClientError::Timeout;
    }

    DaemonClientError::Unavailable(format!(
        "failed to connect to daemon socket `{}`: {source}",
        socket.path().display()
    ))
}

#[cfg(unix)]
fn request_io_error(context: &str, socket: &DaemonSocket, source: io::Error) -> DaemonClientError {
    if is_timeout(&source) {
        return DaemonClientError::Timeout;
    }

    DaemonClientError::Unavailable(format!(
        "{context} on `{}` failed: {source}",
        socket.path().display()
    ))
}

#[cfg(unix)]
fn should_keep_polling(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound
            | io::ErrorKind::ConnectionRefused
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::Interrupted
            | io::ErrorKind::WouldBlock
    )
}

#[cfg(unix)]
fn is_timeout(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
    )
}

fn sleep_until_next_poll(deadline: Instant, poll_interval: Duration) {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return;
    }

    thread::sleep(remaining.min(poll_interval));
}

fn deadline_after(duration: Duration) -> Instant {
    Instant::now()
        .checked_add(duration)
        .unwrap_or_else(Instant::now)
}

#[cfg(unix)]
fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;

    path.as_os_str().as_bytes().to_vec()
}

#[cfg(not(unix))]
fn path_bytes(path: &Path) -> Vec<u8> {
    path.to_string_lossy().as_bytes().to_vec()
}

#[cfg(unix)]
fn socket_path_bytes(path: &Path) -> usize {
    use std::os::unix::ffi::OsStrExt;

    path.as_os_str().as_bytes().len()
}

#[cfg(not(unix))]
fn socket_path_bytes(path: &Path) -> usize {
    path.to_string_lossy().len()
}

#[cfg(unix)]
pub fn default_socket_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("cairn")
}

#[cfg(not(unix))]
pub fn default_socket_dir() -> PathBuf {
    std::env::temp_dir().join("cairn")
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairn_protocol::{AdapterHeartbeat, AdapterKind, AdapterRef, Confidence};
    use cairn_types::{AdapterCapabilities, Timestamp};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn config_hash(hex_digit: char) -> ConfigHash {
        ConfigHash::from_hex(std::iter::repeat_n(hex_digit, 64).collect::<String>())
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let label_hint = label.bytes().fold(0_u16, |acc, byte| {
            acc.wrapping_mul(31).wrapping_add(u16::from(byte))
        });
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);

        PathBuf::from("/tmp").join(format!(
            "c{label_hint:x}-{:x}-{counter:x}-{nanos:x}",
            std::process::id()
        ))
    }

    fn identity(root: &Path, hex_digit: char) -> DaemonIdentity {
        fs::create_dir_all(root).expect("test root should be created");
        DaemonIdentity::compute(root, config_hash(hex_digit), ProtocolVersion(1))
            .expect("identity should compute for temp root")
    }

    fn heartbeat_event(client: &LocalDaemonClient) -> DaemonEvent {
        DaemonEvent::AdapterHeartbeat(AdapterHeartbeat {
            agent_session_id: None,
            worktree_id: client.identity().worktree().worktree_id.clone(),
            harness: AdapterRef {
                adapter_id: "test-harness".to_owned(),
                adapter_kind: AdapterKind::HarnessSim,
            },
            capabilities: AdapterCapabilities::default(),
            sent_at: Timestamp(1),
            daemon_generation_id: None,
            token_usage: None,
            queued_event_count: 0,
            degraded: None,
        })
    }

    #[derive(Default)]
    struct CountingLauncher {
        launches: Arc<AtomicUsize>,
    }

    impl DaemonLauncher for CountingLauncher {
        fn launch(&self, _context: &DaemonLaunchContext<'_>) -> Result<(), DaemonClientError> {
            self.launches.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[cfg(unix)]
    struct ReadinessSocketLauncher {
        launches: Arc<AtomicUsize>,
    }

    #[cfg(unix)]
    impl DaemonLauncher for ReadinessSocketLauncher {
        fn launch(&self, context: &DaemonLaunchContext<'_>) -> Result<(), DaemonClientError> {
            use std::os::unix::net::UnixListener;

            self.launches.fetch_add(1, Ordering::SeqCst);
            let listener = UnixListener::bind(context.socket.path())
                .map_err(|source| DaemonClientError::LaunchFailed(source.to_string()))?;

            let _server = thread::spawn(move || {
                let (_stream, _) = listener.accept().expect("readiness probe should connect");
            });

            Ok(())
        }
    }

    #[cfg(unix)]
    struct EventServerLauncher {
        launches: Arc<AtomicUsize>,
        request_sender: std::sync::Mutex<Option<std::sync::mpsc::Sender<DaemonClientRequest>>>,
    }

    #[cfg(unix)]
    impl DaemonLauncher for EventServerLauncher {
        fn launch(&self, context: &DaemonLaunchContext<'_>) -> Result<(), DaemonClientError> {
            use std::os::unix::net::UnixListener;

            self.launches.fetch_add(1, Ordering::SeqCst);
            let request_sender = self
                .request_sender
                .lock()
                .expect("request sender mutex should not be poisoned")
                .take()
                .expect("event server launcher should be used once");
            let listener = UnixListener::bind(context.socket.path())
                .map_err(|source| DaemonClientError::LaunchFailed(source.to_string()))?;

            let _server = thread::spawn(move || {
                let (_readiness_stream, _) = listener
                    .accept()
                    .expect("launch readiness probe should connect");
                let (stream, _) = listener.accept().expect("event request should connect");
                let mut reader = BufReader::new(stream);
                let mut request_line = String::new();
                reader
                    .read_line(&mut request_line)
                    .expect("event request should read");
                let request: DaemonClientRequest =
                    serde_json::from_str(&request_line).expect("event request should decode");

                let mut stream = reader.into_inner();
                serde_json::to_writer(
                    &mut stream,
                    &DaemonClientResponse::Decision(Box::new(DaemonDecision::allow(
                        Confidence::Verified,
                    ))),
                )
                .expect("decision response should encode");
                stream
                    .write_all(b"\n")
                    .expect("decision newline should write");
                request_sender
                    .send(request)
                    .expect("captured request should send to test");
            });

            Ok(())
        }
    }

    #[cfg(unix)]
    fn accept_one_connection(path: &Path) -> thread::JoinHandle<()> {
        use std::os::unix::net::UnixListener;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("socket parent should be created");
        }
        let listener = UnixListener::bind(path).expect("listener should bind");

        thread::spawn(move || {
            let (_stream, _) = listener.accept().expect("client should connect");
        })
    }

    #[cfg(unix)]
    fn fast_config(socket_dir: impl Into<PathBuf>) -> DaemonClientConfig {
        DaemonClientConfig {
            socket_dir: socket_dir.into(),
            connect_timeout: Duration::from_millis(10),
            launch_timeout: Duration::from_millis(100),
            request_timeout: Duration::from_millis(25),
            poll_interval: Duration::from_millis(1),
        }
    }

    #[test]
    fn socket_key_includes_config_hash() {
        let root = unique_temp_dir("identity-key");
        let first = identity(&root, 'a');
        let second = identity(&root, 'b');

        assert_ne!(first.socket_key(), second.socket_key());

        fs::remove_dir_all(root).expect("temp root should clean up");
    }

    #[test]
    fn socket_key_includes_protocol_version() {
        let root = unique_temp_dir("protocol-key");
        fs::create_dir_all(&root).expect("test root should be created");
        let hash = config_hash('a');
        let first = DaemonIdentity::compute(&root, hash.clone(), ProtocolVersion(1))
            .expect("first protocol identity should compute");
        let second = DaemonIdentity::compute(&root, hash, ProtocolVersion(2))
            .expect("second protocol identity should compute");

        assert_ne!(first.socket_key(), second.socket_key());

        fs::remove_dir_all(root).expect("temp root should clean up");
    }

    #[test]
    fn hello_carries_full_project_identity() {
        let root = unique_temp_dir("hello");
        let identity = identity(&root, 'c');
        let hello = identity.hello();
        let worktree = identity.worktree();

        assert_eq!(hello.identity_key, identity.socket_key());
        assert_eq!(
            hello.canonical_root.as_path(),
            worktree.canonical_root.as_path()
        );
        assert_eq!(hello.worktree_id, worktree.worktree_id.as_str());
        assert_eq!(hello.config_hash, worktree.config_hash.as_hex());
        assert_eq!(hello.protocol_version, worktree.protocol_version.0);

        fs::remove_dir_all(root).expect("temp root should clean up");
    }

    #[test]
    fn socket_path_is_short_for_default_dir() {
        let root = unique_temp_dir("short-path");
        let identity = identity(&root, 'a');
        let socket =
            DaemonSocket::for_identity(&identity, default_socket_dir()).expect("socket path ok");

        assert!(socket_path_bytes(socket.path()) <= MAX_UNIX_SOCKET_PATH_BYTES);

        fs::remove_dir_all(root).expect("temp root should clean up");
    }

    #[test]
    fn long_socket_directory_is_rejected() {
        let root = unique_temp_dir("long-path");
        let identity = identity(&root, 'a');
        let long_dir = root.join("x".repeat(MAX_UNIX_SOCKET_PATH_BYTES));

        let error = DaemonSocket::for_identity(&identity, long_dir)
            .expect_err("path longer than macOS Unix socket limit should fail");

        assert!(matches!(error, DaemonClientError::Unavailable(_)));
        fs::remove_dir_all(root).expect("temp root should clean up");
    }

    #[test]
    fn degraded_errors_convert_to_fail_open_decisions() {
        let error = DaemonClientError::LaunchFailed("daemon did not start".to_owned());

        let decision = error.degraded_decision();
        let degradation = error.degradation(Timestamp(7));
        let state = error.degraded_state(Timestamp(7));

        assert_eq!(error.kind(), DaemonClientErrorKind::LaunchFailed);
        assert!(error.is_fail_open());
        assert_eq!(error.degradation_reason(), "daemon did not start");
        assert_eq!(degradation.kind, DaemonClientErrorKind::LaunchFailed);
        assert_eq!(degradation.reason, "daemon did not start");
        assert!(degradation.fail_open);
        assert_eq!(degradation.since, Timestamp(7));
        assert_eq!(state.reason, "daemon did not start");
        assert!(state.fail_open);
        assert_eq!(state.since, Timestamp(7));
        assert_eq!(
            decision.decision_kind,
            cairn_protocol::DaemonDecisionKind::Allow
        );
        let degraded = decision
            .degraded
            .expect("degraded decision should carry degraded metadata");
        assert_eq!(degraded.state.reason, "daemon did not start");
        assert!(degraded.state.fail_open);
    }

    #[test]
    fn connect_or_launch_uses_launcher_and_degrades_when_socket_never_appears() {
        let root = unique_temp_dir("launch-timeout");
        let identity = identity(&root, 'a');
        let socket_dir = root.join("s");
        let launches = Arc::new(AtomicUsize::new(0));
        let launcher = Arc::new(CountingLauncher {
            launches: Arc::clone(&launches),
        });
        let config = DaemonClientConfig {
            socket_dir,
            connect_timeout: Duration::from_millis(1),
            launch_timeout: Duration::from_millis(2),
            request_timeout: Duration::from_millis(1),
            poll_interval: Duration::from_millis(1),
        };
        let client =
            LocalDaemonClient::with_launcher(identity, config, launcher).expect("client builds");

        let error = client
            .connect_or_launch()
            .expect_err("missing daemon should degrade");

        assert!(matches!(error, DaemonClientError::LaunchFailed(_)));
        assert_eq!(launches.load(Ordering::SeqCst), 1);
        fs::remove_dir_all(root).expect("temp root should clean up");
    }

    #[cfg(unix)]
    #[test]
    fn connect_or_launch_discovers_existing_identity_socket_without_launching() {
        let root = unique_temp_dir("existing-socket");
        let identity = identity(&root, 'a');
        let socket_dir = root.join("s");
        let launches = Arc::new(AtomicUsize::new(0));
        let client = LocalDaemonClient::with_launcher(
            identity,
            fast_config(socket_dir),
            Arc::new(CountingLauncher {
                launches: Arc::clone(&launches),
            }),
        )
        .expect("client builds");
        let server = accept_one_connection(client.socket().path());

        client
            .connect_or_launch()
            .expect("reachable daemon should be discovered");
        server.join().expect("server should finish");

        assert_eq!(launches.load(Ordering::SeqCst), 0);
        fs::remove_dir_all(root).expect("temp root should clean up");
    }

    #[cfg(unix)]
    #[test]
    fn connect_or_launch_launches_when_identity_socket_is_absent() {
        let root = unique_temp_dir("launch-success");
        let identity = identity(&root, 'a');
        let socket_dir = root.join("s");
        let launches = Arc::new(AtomicUsize::new(0));
        let launcher = Arc::new(ReadinessSocketLauncher {
            launches: Arc::clone(&launches),
        });
        let client = LocalDaemonClient::with_launcher(identity, fast_config(socket_dir), launcher)
            .expect("client builds");

        client
            .connect_or_launch()
            .expect("launcher-created socket should become reachable");

        assert_eq!(launches.load(Ordering::SeqCst), 1);
        fs::remove_dir_all(root).expect("temp root should clean up");
    }

    #[cfg(unix)]
    #[test]
    fn request_status_round_trips_over_json_line_socket() {
        use std::os::unix::net::UnixListener;

        let root = unique_temp_dir("status-roundtrip");
        let identity = identity(&root, 'a');
        let socket_dir = root.join("s");
        fs::create_dir_all(&socket_dir).expect("socket dir should be created");
        let client = LocalDaemonClient::with_launcher(
            identity,
            DaemonClientConfig::new(&socket_dir),
            Arc::new(CountingLauncher::default()),
        )
        .expect("client builds");
        let report = DaemonStatusReport {
            generation_id: Some("gen-status".to_owned()),
            storage_path: Some(root.join("cairn.sqlite")),
            degraded_reason: None,
        };

        let listener = UnixListener::bind(client.socket().path()).expect("listener binds");
        let expected_report = report.clone();
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("client should connect");
            let mut reader = BufReader::new(stream);
            let mut request_line = String::new();
            reader
                .read_line(&mut request_line)
                .expect("request should read");
            let request: DaemonClientRequest =
                serde_json::from_str(&request_line).expect("request should decode");
            assert!(matches!(request, DaemonClientRequest::Status { .. }));

            let mut stream = reader.into_inner();
            serde_json::to_writer(
                &mut stream,
                &DaemonClientResponse::Status(Box::new(expected_report)),
            )
            .expect("status response should encode");
            stream.write_all(b"\n").expect("response newline writes");
        });

        let received = client
            .request_status()
            .expect("status should receive daemon report");

        assert_eq!(received, report);
        server.join().expect("server should finish");
        fs::remove_dir_all(root).expect("temp root should clean up");
    }

    #[cfg(unix)]
    #[test]
    fn send_event_launches_absent_daemon_and_retries_request() {
        let root = unique_temp_dir("send-launch");
        let identity = identity(&root, 'a');
        let socket_dir = root.join("s");
        let launches = Arc::new(AtomicUsize::new(0));
        let (request_sender, request_receiver) = std::sync::mpsc::channel();
        let launcher = Arc::new(EventServerLauncher {
            launches: Arc::clone(&launches),
            request_sender: std::sync::Mutex::new(Some(request_sender)),
        });
        let client = LocalDaemonClient::with_launcher(identity, fast_config(socket_dir), launcher)
            .expect("client builds");

        let decision = client
            .send_event(&heartbeat_event(&client))
            .expect("event should launch daemon and receive decision");
        let request = request_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("event request should be captured");

        assert_eq!(decision, DaemonDecision::allow(Confidence::Verified));
        assert_eq!(launches.load(Ordering::SeqCst), 1);
        let DaemonClientRequest::Event { hello, event } = request else {
            panic!("expected event request");
        };
        assert_eq!(hello.identity_key, client.identity().socket_key());
        assert_eq!(*event, heartbeat_event(&client));
        fs::remove_dir_all(root).expect("temp root should clean up");
    }

    #[cfg(unix)]
    #[test]
    fn degraded_event_response_becomes_fail_open_decision() {
        use std::os::unix::net::UnixListener;

        let root = unique_temp_dir("degraded-event");
        let identity = identity(&root, 'a');
        let socket_dir = root.join("s");
        fs::create_dir_all(&socket_dir).expect("socket dir should be created");
        let client = LocalDaemonClient::with_launcher(
            identity,
            DaemonClientConfig::new(&socket_dir),
            Arc::new(CountingLauncher::default()),
        )
        .expect("client builds");

        let listener = UnixListener::bind(client.socket().path()).expect("listener binds");
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("client should connect");
            let mut reader = BufReader::new(stream);
            let mut request_line = String::new();
            reader
                .read_line(&mut request_line)
                .expect("request should read");
            let request: DaemonClientRequest =
                serde_json::from_str(&request_line).expect("request should decode");
            assert!(matches!(request, DaemonClientRequest::Event { .. }));

            let mut stream = reader.into_inner();
            serde_json::to_writer(
                &mut stream,
                &DaemonClientResponse::Degraded {
                    reason: "storage warming up".to_owned(),
                },
            )
            .expect("response should encode");
            stream.write_all(b"\n").expect("response newline writes");
        });

        let decision = client
            .send_event(&heartbeat_event(&client))
            .expect("degraded daemon response should still return a decision");
        let degraded = decision
            .degraded
            .expect("decision should carry degraded metadata");

        assert!(degraded.state.fail_open);
        assert_eq!(degraded.state.reason, "storage warming up");
        server.join().expect("server should finish");
        fs::remove_dir_all(root).expect("temp root should clean up");
    }

    #[cfg(unix)]
    #[test]
    fn send_event_times_out_without_launching_a_second_daemon() {
        let root = unique_temp_dir("request-timeout");
        let identity = identity(&root, 'a');
        let socket_dir = root.join("s");
        let launches = Arc::new(AtomicUsize::new(0));
        let config = DaemonClientConfig {
            socket_dir,
            connect_timeout: Duration::from_millis(10),
            launch_timeout: Duration::from_millis(25),
            request_timeout: Duration::from_millis(5),
            poll_interval: Duration::from_millis(1),
        };
        let client = LocalDaemonClient::with_launcher(
            identity,
            config,
            Arc::new(CountingLauncher {
                launches: Arc::clone(&launches),
            }),
        )
        .expect("client builds");
        let server = {
            use std::os::unix::net::UnixListener;

            fs::create_dir_all(client.socket().path().parent().expect("socket has parent"))
                .expect("socket parent should be created");
            let listener = UnixListener::bind(client.socket().path()).expect("listener binds");
            thread::spawn(move || {
                let (_stream, _) = listener.accept().expect("client should connect");
                thread::sleep(Duration::from_millis(50));
            })
        };

        let error = client
            .send_event(&heartbeat_event(&client))
            .expect_err("silent daemon should time out");
        server.join().expect("server should finish");

        assert!(matches!(error, DaemonClientError::Timeout));
        assert_eq!(launches.load(Ordering::SeqCst), 0);
        fs::remove_dir_all(root).expect("temp root should clean up");
    }

    #[cfg(unix)]
    #[test]
    fn send_event_round_trips_a_decision() {
        use std::os::unix::net::UnixListener;

        let root = unique_temp_dir("roundtrip");
        let identity = identity(&root, 'a');
        let socket_dir = root.join("s");
        fs::create_dir_all(&socket_dir).expect("socket dir should be created");
        let config = DaemonClientConfig::new(&socket_dir);
        let client = LocalDaemonClient::with_launcher(
            identity,
            config,
            Arc::new(CountingLauncher::default()),
        )
        .expect("client builds");

        let listener = UnixListener::bind(client.socket().path()).expect("listener binds");
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("client should connect");
            let mut reader = BufReader::new(stream);
            let mut request_line = String::new();
            reader
                .read_line(&mut request_line)
                .expect("request should read");
            let request: DaemonClientRequest =
                serde_json::from_str(&request_line).expect("request should decode");

            let mut stream = reader.into_inner();
            serde_json::to_writer(
                &mut stream,
                &DaemonClientResponse::Decision(Box::new(DaemonDecision::allow(
                    Confidence::Verified,
                ))),
            )
            .expect("response should encode");
            stream.write_all(b"\n").expect("response newline writes");
            request
        });

        let decision = client
            .send_event(&heartbeat_event(&client))
            .expect("event should receive decision");
        let request = server.join().expect("server should finish");

        assert_eq!(decision, DaemonDecision::allow(Confidence::Verified));
        assert!(matches!(request, DaemonClientRequest::Event { .. }));
        fs::remove_dir_all(root).expect("temp root should clean up");
    }
}
