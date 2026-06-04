//! `cairn-cli` — operator command implementations, called by the `cairn` binary.
//!
//! Wave 1.2 keeps this crate synchronous and dependency-light. The root binary
//! wires process arguments and the concrete daemon client later; this crate owns
//! argument parsing, daemon probing through the frozen `DaemonClient` trait, and
//! terse text / JSON rendering.

use std::fmt;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use cairn_daemon_client::{DaemonClient, DaemonClientError, DaemonStatusReport, LocalDaemonClient};
use cairn_protocol::{
    AdapterHeartbeat, AdapterKind, AdapterRef, DaemonDecision, DaemonEvent, PROTOCOL_VERSION,
};
use cairn_types::{AdapterCapabilities, ProtocolVersion, Timestamp, WorktreeId};
use serde_json::{Value, json};

/// Operator command accepted by the Wave 1.2 CLI surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// `cairn status`
    Status { format: OutputFormat },
    /// `cairn daemon doctor --self-test`
    DaemonDoctor { format: OutputFormat },
}

impl Command {
    #[must_use]
    fn format(&self) -> OutputFormat {
        match self {
            Command::Status { format } | Command::DaemonDoctor { format } => *format,
        }
    }

    #[must_use]
    fn name(&self) -> &'static str {
        match self {
            Command::Status { .. } => "status",
            Command::DaemonDoctor { .. } => "daemon doctor",
        }
    }
}

/// Human text by default, compact JSON when requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
}

/// Identity details the app layer resolved before invoking the CLI command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentitySummary {
    pub worktree_id: String,
    pub root: PathBuf,
    pub config_hash: Option<String>,
    pub protocol_version: Option<u32>,
}

impl IdentitySummary {
    #[must_use]
    pub fn new(worktree_id: impl Into<String>, root: impl Into<PathBuf>) -> Self {
        Self {
            worktree_id: worktree_id.into(),
            root: root.into(),
            config_hash: None,
            protocol_version: None,
        }
    }

    #[must_use]
    pub fn with_config_hash(mut self, config_hash: impl Into<String>) -> Self {
        self.config_hash = Some(config_hash.into());
        self
    }

    #[must_use]
    pub fn with_protocol_version(mut self, protocol_version: u32) -> Self {
        self.protocol_version = Some(protocol_version);
        self
    }
}

/// Stable facts rendered by `status` and `daemon doctor`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusSnapshot {
    pub identity: IdentitySummary,
    pub daemon_generation: Option<String>,
    pub storage_path: Option<PathBuf>,
}

impl StatusSnapshot {
    #[must_use]
    pub fn new(
        identity: IdentitySummary,
        daemon_generation: Option<String>,
        storage_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            identity,
            daemon_generation,
            storage_path: Some(storage_path.into()),
        }
    }

    /// Fallback snapshot used before the daemon has reported runtime state.
    #[must_use]
    pub fn unknown(identity: IdentitySummary) -> Self {
        Self {
            identity,
            daemon_generation: None,
            storage_path: None,
        }
    }

    #[must_use]
    fn from_daemon_report(identity: IdentitySummary, report: &DaemonStatusReport) -> Self {
        Self {
            identity,
            daemon_generation: report.generation_id.clone(),
            storage_path: report.storage_path.clone(),
        }
    }
}

/// Inputs supplied by the root binary once app wiring lands.
#[derive(Debug)]
pub struct CommandContext<'a, C> {
    pub client: &'a C,
    pub snapshot: StatusSnapshot,
}

impl<'a, C> CommandContext<'a, C> {
    #[must_use]
    pub fn new(client: &'a C, snapshot: StatusSnapshot) -> Self {
        Self { client, snapshot }
    }
}

/// CLI-local extension over the frozen daemon-client trait.
///
/// Wave 1.2 froze [`DaemonClient`] at connect/send-event. The CLI also needs the
/// daemon's runtime status without changing that trait, so production clients and
/// tests implement this narrow adapter locally.
pub trait DaemonStatusClient: DaemonClient {
    fn request_daemon_status(&self) -> Result<DaemonStatusReport, DaemonClientError>;
}

impl DaemonStatusClient for LocalDaemonClient {
    fn request_daemon_status(&self) -> Result<DaemonStatusReport, DaemonClientError> {
        self.request_status()
    }
}

/// Rendered command result before serialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandReport {
    pub command: &'static str,
    pub snapshot: StatusSnapshot,
    pub degraded: DegradedState,
    pub self_test: Option<SelfTestState>,
}

impl CommandReport {
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "command": self.command,
            "identity": {
                "worktree_id": self.snapshot.identity.worktree_id,
                "root": path_string(&self.snapshot.identity.root),
                "config_hash": self.snapshot.identity.config_hash,
                "protocol_version": self.snapshot.identity.protocol_version,
            },
            "daemon_generation": self.snapshot.daemon_generation,
            "storage_path": self.snapshot.storage_path.as_ref().map(|path| path_string(path)),
            "degraded": self.degraded.is_degraded,
            "fail_open": self.degraded.fail_open,
            "degraded_reason": self.degraded.reason,
            "self_test": self.self_test.map(SelfTestState::as_str),
        })
    }
}

/// Whether the daemon path was reachable enough for the host to trust Cairn's
/// advisory status. Client-side degraded errors are fail-open; daemon-reported
/// degraded decisions preserve the daemon's explicit `fail_open` flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DegradedState {
    pub is_degraded: bool,
    pub fail_open: bool,
    pub reason: Option<String>,
}

impl DegradedState {
    #[must_use]
    pub fn healthy() -> Self {
        Self {
            is_degraded: false,
            fail_open: false,
            reason: None,
        }
    }

    #[must_use]
    pub fn degraded(reason: impl Into<String>) -> Self {
        Self::degraded_with_fail_open(reason, true)
    }

    #[must_use]
    pub fn degraded_with_fail_open(reason: impl Into<String>, fail_open: bool) -> Self {
        Self {
            is_degraded: true,
            fail_open,
            reason: Some(reason.into()),
        }
    }
}

/// Result of `daemon doctor --self-test`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfTestState {
    Ok,
    Degraded,
}

impl SelfTestState {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            SelfTestState::Ok => "ok",
            SelfTestState::Degraded => "degraded",
        }
    }
}

/// Parse arguments after the binary name, e.g. `["status", "--json"]`.
pub fn parse_args<I, S>(args: I) -> Result<Command, ParseError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args = args
        .into_iter()
        .map(|arg| arg.as_ref().to_owned())
        .collect::<Vec<_>>();

    match args.first().map(String::as_str) {
        Some("status") => parse_status(&args[1..]),
        Some("daemon") => parse_daemon(&args[1..]),
        Some(command) => Err(ParseError::UnknownCommand(command.to_owned())),
        None => Err(ParseError::MissingCommand),
    }
}

/// Run a parsed command and render it to `writer`.
pub fn run_to_writer<C, W>(
    command: &Command,
    context: &CommandContext<'_, C>,
    mut writer: W,
) -> Result<(), CliError>
where
    C: DaemonStatusClient,
    W: Write,
{
    let report = run_command(command, context);
    render_report(&report, command.format(), &mut writer).map_err(CliError::Render)
}

/// Parse, run, and render in one call for the future root binary.
pub fn run_args_to_writer<C, I, S, W>(
    args: I,
    context: &CommandContext<'_, C>,
    writer: W,
) -> Result<(), CliError>
where
    C: DaemonStatusClient,
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: Write,
{
    let command = parse_args(args).map_err(CliError::Parse)?;
    run_to_writer(&command, context, writer)
}

/// Run a parsed command and return the structured report.
#[must_use]
pub fn run_command<C>(command: &Command, context: &CommandContext<'_, C>) -> CommandReport
where
    C: DaemonStatusClient,
{
    let probe = probe_daemon(context);
    let self_test = match command {
        Command::Status { .. } => None,
        Command::DaemonDoctor { .. } => Some(if probe.degraded.is_degraded {
            SelfTestState::Degraded
        } else {
            SelfTestState::Ok
        }),
    };

    CommandReport {
        command: command.name(),
        snapshot: probe.snapshot,
        degraded: probe.degraded,
        self_test,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    MissingCommand,
    UnknownCommand(String),
    MissingDaemonSubcommand,
    UnknownDaemonSubcommand(String),
    MissingSelfTest,
    UnknownFlag(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::MissingCommand => write!(formatter, "missing command"),
            ParseError::UnknownCommand(command) => write!(formatter, "unknown command `{command}`"),
            ParseError::MissingDaemonSubcommand => write!(formatter, "missing daemon subcommand"),
            ParseError::UnknownDaemonSubcommand(command) => {
                write!(formatter, "unknown daemon subcommand `{command}`")
            }
            ParseError::MissingSelfTest => {
                write!(formatter, "`cairn daemon doctor` requires `--self-test`")
            }
            ParseError::UnknownFlag(flag) => write!(formatter, "unknown flag `{flag}`"),
        }
    }
}

impl std::error::Error for ParseError {}

#[derive(Debug)]
pub enum CliError {
    Parse(ParseError),
    Render(io::Error),
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::Parse(error) => write!(formatter, "{error}"),
            CliError::Render(error) => write!(formatter, "failed to render CLI output: {error}"),
        }
    }
}

impl std::error::Error for CliError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CliError::Parse(error) => Some(error),
            CliError::Render(error) => Some(error),
        }
    }
}

fn parse_status(args: &[String]) -> Result<Command, ParseError> {
    let format = parse_json_flag(args)?;
    Ok(Command::Status { format })
}

fn parse_daemon(args: &[String]) -> Result<Command, ParseError> {
    match args.first().map(String::as_str) {
        Some("doctor") => parse_daemon_doctor(&args[1..]),
        Some(command) => Err(ParseError::UnknownDaemonSubcommand(command.to_owned())),
        None => Err(ParseError::MissingDaemonSubcommand),
    }
}

fn parse_daemon_doctor(args: &[String]) -> Result<Command, ParseError> {
    let mut format = OutputFormat::Text;
    let mut self_test = false;

    for arg in args {
        match arg.as_str() {
            "--json" => format = OutputFormat::Json,
            "--self-test" => self_test = true,
            flag if flag.starts_with('-') => return Err(ParseError::UnknownFlag(flag.to_owned())),
            value => return Err(ParseError::UnknownFlag(value.to_owned())),
        }
    }

    if !self_test {
        return Err(ParseError::MissingSelfTest);
    }

    Ok(Command::DaemonDoctor { format })
}

fn parse_json_flag(args: &[String]) -> Result<OutputFormat, ParseError> {
    let mut format = OutputFormat::Text;
    for arg in args {
        match arg.as_str() {
            "--json" => format = OutputFormat::Json,
            flag if flag.starts_with('-') => return Err(ParseError::UnknownFlag(flag.to_owned())),
            value => return Err(ParseError::UnknownFlag(value.to_owned())),
        }
    }
    Ok(format)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DaemonProbe {
    snapshot: StatusSnapshot,
    degraded: DegradedState,
}

fn probe_daemon<C>(context: &CommandContext<'_, C>) -> DaemonProbe
where
    C: DaemonStatusClient,
{
    let client = context.client;
    if let Err(error) = client.connect_or_launch() {
        return DaemonProbe {
            snapshot: context.snapshot.clone(),
            degraded: degraded_error(error),
        };
    }

    let (snapshot, mut degraded) = match client.request_daemon_status() {
        Ok(report) => {
            let degraded = report
                .degraded_reason
                .as_deref()
                .map_or_else(DegradedState::healthy, DegradedState::degraded);
            (
                StatusSnapshot::from_daemon_report(context.snapshot.identity.clone(), &report),
                degraded,
            )
        }
        Err(error) => (context.snapshot.clone(), degraded_error(error)),
    };

    let heartbeat = heartbeat_event(&snapshot);

    match client.send_event(&heartbeat) {
        Ok(decision) => {
            if let Some(decision_degraded) = degraded_decision(&decision) {
                degraded = decision_degraded;
            }
        }
        Err(error) => degraded = degraded_error(error),
    }

    DaemonProbe { snapshot, degraded }
}

fn heartbeat_event(snapshot: &StatusSnapshot) -> DaemonEvent {
    DaemonEvent::AdapterHeartbeat(AdapterHeartbeat {
        session_id: None,
        worktree_id: WorktreeId::new(snapshot.identity.worktree_id.clone()),
        adapter: AdapterRef {
            adapter_id: "cairn-cli".to_owned(),
            adapter_kind: AdapterKind::Other,
        },
        protocol_version: ProtocolVersion(
            snapshot
                .identity
                .protocol_version
                .unwrap_or(PROTOCOL_VERSION),
        ),
        capabilities: AdapterCapabilities::default(),
        sent_at: current_timestamp(),
        daemon_generation_id: snapshot.daemon_generation.clone(),
        token_usage: None,
        queued_event_count: 0,
        degraded: None,
    })
}

fn degraded_decision(decision: &DaemonDecision) -> Option<DegradedState> {
    decision.degraded.as_ref().map(|degraded| {
        DegradedState::degraded_with_fail_open(
            degraded.state.reason.clone(),
            degraded.state.fail_open,
        )
    })
}

fn degraded_error(error: DaemonClientError) -> DegradedState {
    DegradedState::degraded(error.to_string())
}

fn render_report<W>(report: &CommandReport, format: OutputFormat, writer: &mut W) -> io::Result<()>
where
    W: Write,
{
    match format {
        OutputFormat::Text => render_text(report, writer),
        OutputFormat::Json => writeln!(writer, "{}", report.to_json()),
    }
}

fn render_text<W>(report: &CommandReport, writer: &mut W) -> io::Result<()>
where
    W: Write,
{
    writeln!(writer, "identity: {}", report.snapshot.identity.worktree_id)?;
    writeln!(
        writer,
        "root: {}",
        path_string(&report.snapshot.identity.root)
    )?;
    writeln!(
        writer,
        "config_hash: {}",
        optional_text(report.snapshot.identity.config_hash.as_deref())
    )?;
    writeln!(
        writer,
        "protocol_version: {}",
        optional_number_text(report.snapshot.identity.protocol_version)
    )?;
    writeln!(
        writer,
        "daemon_generation: {}",
        optional_text(report.snapshot.daemon_generation.as_deref())
    )?;
    let storage_path = report
        .snapshot
        .storage_path
        .as_ref()
        .map_or_else(|| "unknown".to_owned(), |path| path_string(path));
    writeln!(writer, "storage_path: {storage_path}")?;
    writeln!(writer, "degraded: {}", yes_no(report.degraded.is_degraded))?;

    if let Some(reason) = report.degraded.reason.as_deref() {
        writeln!(writer, "fail_open: {}", yes_no(report.degraded.fail_open))?;
        writeln!(writer, "degraded_reason: {reason}")?;
    }

    if let Some(self_test) = report.self_test {
        writeln!(writer, "self_test: {}", self_test.as_str())?;
    }

    Ok(())
}

fn current_timestamp() -> Timestamp {
    let Ok(duration) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return Timestamp(0);
    };

    let nanos = duration
        .as_secs()
        .saturating_mul(1_000_000_000)
        .saturating_add(u64::from(duration.subsec_nanos()));

    Timestamp(nanos.min(i64::MAX as u64) as i64)
}

fn optional_text(value: Option<&str>) -> &str {
    value.unwrap_or("unknown")
}

fn optional_number_text(value: Option<u32>) -> String {
    value.map_or_else(|| "unknown".to_owned(), |number| number.to_string())
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use cairn_protocol::{Confidence, DaemonDecisionKind};

    #[test]
    fn parses_status_json() {
        let command = parse_args(["status", "--json"]).expect("status should parse");

        assert_eq!(
            command,
            Command::Status {
                format: OutputFormat::Json
            }
        );
    }

    #[test]
    fn daemon_doctor_requires_self_test() {
        let error = parse_args(["daemon", "doctor"]).expect_err("missing self-test must fail");

        assert_eq!(error, ParseError::MissingSelfTest);
    }

    #[test]
    fn status_uses_daemon_report_over_fallback_snapshot() {
        let client = FakeDaemonClient::healthy();
        let context = context(&client);
        let mut output = Vec::new();
        assert!(context.snapshot.daemon_generation.is_none());
        assert!(context.snapshot.storage_path.is_none());

        run_args_to_writer(["status"], &context, &mut output).expect("status should render");

        let output = String::from_utf8(output).expect("output should be utf-8");
        assert_eq!(client.connect_calls.get(), 1);
        assert_eq!(client.status_calls.get(), 1);
        assert_eq!(client.send_calls.get(), 1);
        assert!(output.contains("identity: wt-test"));
        assert!(output.contains(
            "config_hash: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        ));
        assert!(output.contains("protocol_version: 1"));
        assert!(output.contains("daemon_generation: gen-1"));
        assert!(output.contains("storage_path: /tmp/cairn/events.sqlite"));
        assert!(!output.contains("daemon_generation: unknown"));
        assert!(!output.contains("storage_path: unknown"));
        assert!(output.contains("degraded: no"));
        assert!(!output.contains("fail_open"));
        assert!(!output.contains("enforcement"));
        assert!(!output.contains("capability"));
    }

    #[test]
    fn status_json_is_machine_parseable_and_includes_identity_fields() {
        let client = FakeDaemonClient::healthy();
        let context = context(&client);
        let mut output = Vec::new();

        run_args_to_writer(["status", "--json"], &context, &mut output)
            .expect("status should render");

        let value: Value = serde_json::from_slice(&output).expect("json should parse");
        assert_eq!(value["command"].as_str(), Some("status"));
        assert_eq!(value["identity"]["worktree_id"].as_str(), Some("wt-test"));
        assert_eq!(value["identity"]["root"].as_str(), Some("/tmp/cairn"));
        assert_eq!(
            value["identity"]["config_hash"].as_str(),
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
        assert_eq!(value["identity"]["protocol_version"].as_u64(), Some(1));
        assert_eq!(value["daemon_generation"].as_str(), Some("gen-1"));
        assert_eq!(
            value["storage_path"].as_str(),
            Some("/tmp/cairn/events.sqlite")
        );
        assert_eq!(value["degraded"].as_bool(), Some(false));
        assert_eq!(value["fail_open"].as_bool(), Some(false));
        assert!(value.get("enforcement").is_none());
        assert!(value.get("capabilities").is_none());
        assert_eq!(client.status_calls.get(), 1);
    }

    #[test]
    fn connect_failure_is_reported_as_degraded_without_followup_calls() {
        let client = FakeDaemonClient::healthy().with_connect_error("socket unavailable");
        let context = context(&client);
        let mut output = Vec::new();

        run_args_to_writer(["daemon", "doctor", "--self-test"], &context, &mut output)
            .expect("doctor should render degraded output");

        let output = String::from_utf8(output).expect("output should be utf-8");
        assert_eq!(client.connect_calls.get(), 1);
        assert_eq!(client.status_calls.get(), 0);
        assert_eq!(client.send_calls.get(), 0);
        assert!(output.contains("degraded: yes"));
        assert!(output.contains("fail_open: yes"));
        assert!(output.contains("degraded_reason: daemon unavailable: socket unavailable"));
        assert!(output.contains("self_test: degraded"));
    }

    #[test]
    fn daemon_report_degraded_reason_drives_self_test() {
        let client = FakeDaemonClient::healthy().with_degraded_status("storage readonly");
        let context = context(&client);
        let mut output = Vec::new();

        run_args_to_writer(
            ["daemon", "doctor", "--self-test", "--json"],
            &context,
            &mut output,
        )
        .expect("doctor should render");

        let value: Value = serde_json::from_slice(&output).expect("json should parse");
        assert_eq!(client.connect_calls.get(), 1);
        assert_eq!(client.status_calls.get(), 1);
        assert_eq!(client.send_calls.get(), 1);
        assert_eq!(value["degraded"].as_bool(), Some(true));
        assert_eq!(value["fail_open"].as_bool(), Some(true));
        assert_eq!(value["degraded_reason"].as_str(), Some("storage readonly"));
        assert_eq!(value["self_test"].as_str(), Some("degraded"));
    }

    #[test]
    fn doctor_json_uses_daemon_report_and_reports_degraded_self_test() {
        let client = FakeDaemonClient::with_decision(DaemonDecision::degraded_allow(
            "lease stale",
            test_timestamp(),
        ));
        let context = context(&client);
        let mut output = Vec::new();
        assert!(context.snapshot.daemon_generation.is_none());
        assert!(context.snapshot.storage_path.is_none());

        run_args_to_writer(
            ["daemon", "doctor", "--self-test", "--json"],
            &context,
            &mut output,
        )
        .expect("doctor should render");

        let value: Value = serde_json::from_slice(&output).expect("json should parse");
        assert_eq!(value["command"].as_str(), Some("daemon doctor"));
        assert_eq!(value["daemon_generation"].as_str(), Some("gen-1"));
        assert_eq!(
            value["storage_path"].as_str(),
            Some("/tmp/cairn/events.sqlite")
        );
        assert_eq!(value["degraded"].as_bool(), Some(true));
        assert_eq!(value["fail_open"].as_bool(), Some(true));
        assert_eq!(value["degraded_reason"].as_str(), Some("lease stale"));
        assert_eq!(value["self_test"].as_str(), Some("degraded"));
        assert!(value.get("capabilities").is_none());
        assert!(value.get("enforcement").is_none());
        assert_eq!(client.status_calls.get(), 1);
    }

    fn context(client: &FakeDaemonClient) -> CommandContext<'_, FakeDaemonClient> {
        CommandContext::new(client, StatusSnapshot::unknown(test_identity()))
    }

    fn test_identity() -> IdentitySummary {
        IdentitySummary::new("wt-test", "/tmp/cairn")
            .with_config_hash("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .with_protocol_version(1)
    }

    struct FakeDaemonClient {
        connect_calls: Cell<u32>,
        status_calls: Cell<u32>,
        send_calls: Cell<u32>,
        decision: DaemonDecision,
        status_report: DaemonStatusReport,
        connect_error: Option<&'static str>,
    }

    impl FakeDaemonClient {
        fn healthy() -> Self {
            Self::with_decision(DaemonDecision::allow(Confidence::Verified))
        }

        fn with_decision(decision: DaemonDecision) -> Self {
            Self {
                connect_calls: Cell::new(0),
                status_calls: Cell::new(0),
                send_calls: Cell::new(0),
                decision,
                status_report: DaemonStatusReport {
                    generation_id: Some("gen-1".to_owned()),
                    storage_path: Some(PathBuf::from("/tmp/cairn/events.sqlite")),
                    degraded_reason: None,
                },
                connect_error: None,
            }
        }

        fn with_connect_error(mut self, reason: &'static str) -> Self {
            self.connect_error = Some(reason);
            self
        }

        fn with_degraded_status(mut self, reason: impl Into<String>) -> Self {
            self.status_report.degraded_reason = Some(reason.into());
            self
        }
    }

    impl DaemonStatusClient for FakeDaemonClient {
        fn request_daemon_status(&self) -> Result<DaemonStatusReport, DaemonClientError> {
            self.status_calls.set(self.status_calls.get() + 1);
            Ok(self.status_report.clone())
        }
    }

    impl DaemonClient for FakeDaemonClient {
        fn connect_or_launch(&self) -> Result<(), DaemonClientError> {
            self.connect_calls.set(self.connect_calls.get() + 1);
            if let Some(reason) = self.connect_error {
                return Err(DaemonClientError::Unavailable(reason.to_owned()));
            }
            Ok(())
        }

        fn send_event(&self, event: &DaemonEvent) -> Result<DaemonDecision, DaemonClientError> {
            self.send_calls.set(self.send_calls.get() + 1);
            let DaemonEvent::AdapterHeartbeat(heartbeat) = event else {
                panic!("expected heartbeat event, got {event:?}");
            };
            assert_eq!(heartbeat.adapter.adapter_id, "cairn-cli");
            assert_eq!(heartbeat.adapter.adapter_kind, AdapterKind::Other);
            assert_eq!(heartbeat.worktree_id.as_str(), "wt-test");
            assert_eq!(heartbeat.protocol_version.0, 1);
            assert_eq!(heartbeat.capabilities, AdapterCapabilities::default());
            assert_eq!(heartbeat.daemon_generation_id.as_deref(), Some("gen-1"));
            assert_eq!(self.decision.decision_kind, DaemonDecisionKind::Allow);
            Ok(self.decision.clone())
        }
    }

    fn test_timestamp() -> Timestamp {
        Timestamp(42)
    }
}
