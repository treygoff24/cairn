//! `cairn-harness-sim` — the deterministic Phase 1 daemon fixture.
//!
//! This crate provides a sync, in-memory harness for Phase 1 daemon tests. The
//! real daemon/client crates are still landing in the same wave, so the fixture
//! implements the frozen [`cairn_daemon_client::DaemonClient`] trait and records
//! the pieces Phase 1 cares about:
//!
//! - N simulated clients attach to one worktree.
//! - every client registers an [`cairn_types::AdapterCapabilities`] value.
//! - all clients observe one daemon generation.
//!
//! Capability registration is recorded both in harness state and in the heartbeat
//! payload. Phase 2 can replace the in-memory registry with a real daemon transport
//! while keeping the fixture-facing API.

use std::{
    collections::{BTreeMap, BTreeSet, btree_map::Entry},
    error::Error,
    fmt,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
    thread,
};

use cairn_daemon_client::{DaemonClient, DaemonClientError};
use cairn_protocol::{
    AdapterHeartbeat, AdapterKind, AdapterRef, Confidence, DaemonDecision, DaemonEvent,
    PROTOCOL_VERSION,
};
use cairn_types::{AdapterCapabilities, ProtocolVersion, SessionId, Timestamp, WorktreeId};

/// First deterministic generation assigned by the simulated registry.
pub const FIRST_DAEMON_GENERATION: DaemonGeneration = DaemonGeneration(1);

/// Monotonic identity of one daemon process generation for a worktree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DaemonGeneration(u64);

impl DaemonGeneration {
    /// Returns the numeric generation.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A simulated adapter/client that will attach to the Phase 1 daemon fixture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulatedClientSpec {
    client_name: String,
    capabilities: AdapterCapabilities,
}

impl SimulatedClientSpec {
    /// Creates a client spec with a stable name and capability bitset.
    ///
    /// The name becomes the deterministic registration key, so blank names are
    /// rejected before the fixture is launched.
    pub fn new(
        client_name: impl Into<String>,
        capabilities: AdapterCapabilities,
    ) -> Result<Self, HarnessError> {
        let client_name = client_name.into();
        if client_name.trim().is_empty() {
            return Err(HarnessError::EmptyClientName);
        }

        Ok(Self {
            client_name,
            capabilities,
        })
    }

    /// Stable registration name for this simulated client.
    #[must_use]
    pub fn client_name(&self) -> &str {
        &self.client_name
    }

    /// Capability bitset this client registers on attach.
    #[must_use]
    pub const fn capabilities(&self) -> AdapterCapabilities {
        self.capabilities
    }
}

/// Deterministic Phase 1 fixture: one worktree, many simulated clients.
#[derive(Debug, Clone)]
pub struct Phase1DaemonFixture {
    worktree_root: PathBuf,
    client_specs: Vec<SimulatedClientSpec>,
    registry: SharedDaemonRegistry,
}

impl Phase1DaemonFixture {
    /// Creates an empty fixture for `worktree_root`.
    #[must_use]
    pub fn new(worktree_root: impl Into<PathBuf>) -> Self {
        Self {
            worktree_root: worktree_root.into(),
            client_specs: Vec::new(),
            registry: SharedDaemonRegistry::default(),
        }
    }

    /// Creates a fixture with explicit client specs.
    pub fn with_client_specs(
        worktree_root: impl Into<PathBuf>,
        client_specs: Vec<SimulatedClientSpec>,
    ) -> Result<Self, HarnessError> {
        ensure_unique_client_names(&client_specs)?;

        Ok(Self {
            worktree_root: worktree_root.into(),
            client_specs,
            registry: SharedDaemonRegistry::default(),
        })
    }

    /// Creates `client_count` clients named `sim-client-0001`, ... with identical
    /// capabilities.
    pub fn with_n_clients(
        worktree_root: impl Into<PathBuf>,
        client_count: usize,
        capabilities: AdapterCapabilities,
    ) -> Result<Self, HarnessError> {
        let specs = deterministic_client_specs(client_count, capabilities)?;
        Self::with_client_specs(worktree_root, specs)
    }

    /// Worktree all simulated clients attach to.
    #[must_use]
    pub fn worktree_root(&self) -> &Path {
        &self.worktree_root
    }

    /// Client specs in deterministic launch order.
    #[must_use]
    pub fn client_specs(&self) -> &[SimulatedClientSpec] {
        &self.client_specs
    }

    /// Simulated clients in deterministic launch order. Callers can use these
    /// directly when a Phase 2 test needs to send custom daemon events.
    #[must_use]
    pub fn simulated_clients(&self) -> Vec<SimulatedClient> {
        self.client_specs
            .iter()
            .cloned()
            .map(|spec| {
                SimulatedClient::new(self.worktree_root.clone(), spec, self.registry.clone())
            })
            .collect()
    }

    /// Launches every configured client in deterministic order and returns a
    /// snapshot of registrations, events, and observed daemon generations.
    pub fn launch(&self) -> Result<LaunchReport, HarnessError> {
        let mut launches = Vec::with_capacity(self.client_specs.len());

        for (index, client) in self.simulated_clients().into_iter().enumerate() {
            launches.push(client.launch_at(deterministic_timestamp(index)?)?);
        }

        self.report_from_launches(launches)
    }

    /// Returns the current simulated daemon state without launching clients.
    ///
    /// Phase 2 tests can use [`Phase1DaemonFixture::simulated_clients`] to connect
    /// clients and send custom protocol events, then call this method to inspect
    /// the recorded registrations and events without emitting another heartbeat
    /// batch.
    pub fn snapshot(&self) -> Result<LaunchReport, HarnessError> {
        self.report_from_launches(Vec::new())
    }

    /// Launches every configured client on its own thread and returns a stable
    /// report ordered by the fixture's client-spec order.
    ///
    /// This exercises the one-daemon-generation invariant under simultaneous
    /// attach attempts without asking worker agents to run heavy gates. Snapshot
    /// events and capability registrations are sorted by stable keys so Phase 2
    /// assertions stay deterministic even when thread scheduling changes.
    pub fn launch_concurrently(&self) -> Result<LaunchReport, HarnessError> {
        let mut handles = Vec::with_capacity(self.client_specs.len());

        for (index, client) in self.simulated_clients().into_iter().enumerate() {
            let timestamp = deterministic_timestamp(index)?;
            handles.push(thread::spawn(move || {
                client
                    .launch_at(timestamp)
                    .map(|launch| IndexedLaunch { index, launch })
            }));
        }

        let mut indexed_launches = Vec::with_capacity(handles.len());
        for handle in handles {
            let indexed_launch = handle
                .join()
                .map_err(|_| HarnessError::ClientThreadPanicked)??;
            indexed_launches.push(indexed_launch);
        }
        indexed_launches.sort_by_key(|indexed_launch| indexed_launch.index);

        let launches = indexed_launches
            .into_iter()
            .map(|indexed_launch| indexed_launch.launch)
            .collect();
        self.report_from_launches(launches)
    }

    fn report_from_launches(
        &self,
        launches: Vec<ClientLaunch>,
    ) -> Result<LaunchReport, HarnessError> {
        let snapshot = self.snapshot_or_default()?;
        Ok(LaunchReport {
            worktree_root: self.worktree_root.clone(),
            client_launches: launches,
            capability_registrations: snapshot.capability_registrations,
            events: deterministic_events(snapshot.events),
        })
    }

    fn snapshot_or_default(&self) -> Result<DaemonSnapshot, HarnessError> {
        match self.registry.snapshot(&self.worktree_root) {
            Ok(snapshot) => Ok(snapshot),
            Err(HarnessError::MissingDaemon(_)) => Ok(DaemonSnapshot::default()),
            Err(error) => Err(error),
        }
    }
}

/// Builds deterministic client specs for callers that want to customize the
/// fixture before launch.
pub fn deterministic_client_specs(
    client_count: usize,
    capabilities: AdapterCapabilities,
) -> Result<Vec<SimulatedClientSpec>, HarnessError> {
    (1..=client_count)
        .map(|index| SimulatedClientSpec::new(format!("sim-client-{index:04}"), capabilities))
        .collect()
}

/// A conservative observing-only capability set for post-hoc harness adapters.
#[must_use]
pub const fn observing_capabilities() -> AdapterCapabilities {
    AdapterCapabilities {
        can_pre_edit_block: false,
        can_pre_read_decorate: false,
        can_post_read_decorate: true,
        can_command_replace: false,
        can_modify_tool_input: false,
        can_async_notify: true,
        can_precompact: false,
        can_report_token_usage: false,
        can_report_exact_edit_diff: false,
        can_attach_file_precondition: false,
    }
}

/// One simulated client attached through the frozen daemon-client trait.
#[derive(Debug, Clone)]
pub struct SimulatedClient {
    worktree_root: PathBuf,
    spec: SimulatedClientSpec,
    registry: SharedDaemonRegistry,
}

impl SimulatedClient {
    fn new(
        worktree_root: PathBuf,
        spec: SimulatedClientSpec,
        registry: SharedDaemonRegistry,
    ) -> Self {
        Self {
            worktree_root,
            spec,
            registry,
        }
    }

    fn launch_at(&self, at: Timestamp) -> Result<ClientLaunch, HarnessError> {
        self.connect_or_launch()
            .map_err(HarnessError::from_client_error)?;

        let generation = self.connected_generation()?;
        let heartbeat_decision = self
            .send_event(&self.heartbeat_event(at, generation))
            .map_err(HarnessError::from_client_error)?;

        Ok(ClientLaunch {
            client_name: self.spec.client_name.clone(),
            generation,
            heartbeat_decision,
        })
    }

    fn connected_generation(&self) -> Result<DaemonGeneration, HarnessError> {
        self.registry.generation_for(&self.worktree_root)
    }

    fn heartbeat_event(&self, sent_at: Timestamp, generation: DaemonGeneration) -> DaemonEvent {
        DaemonEvent::AdapterHeartbeat(AdapterHeartbeat {
            session_id: Some(SessionId::new(self.spec.client_name.clone())),
            worktree_id: WorktreeId::new(worktree_id_value(&self.worktree_root)),
            adapter: AdapterRef {
                adapter_id: self.spec.client_name.clone(),
                adapter_kind: AdapterKind::HarnessSim,
            },
            protocol_version: ProtocolVersion(PROTOCOL_VERSION),
            capabilities: self.spec.capabilities,
            sent_at,
            daemon_generation_id: Some(generation.get().to_string()),
            token_usage: None,
            queued_event_count: 0,
            degraded: None,
        })
    }

    /// Worktree this simulated client attaches to.
    #[must_use]
    pub fn worktree_root(&self) -> &Path {
        &self.worktree_root
    }

    /// Stable registration name for this simulated client.
    #[must_use]
    pub fn client_name(&self) -> &str {
        self.spec.client_name()
    }

    /// Capability bitset this simulated client registers.
    #[must_use]
    pub const fn capabilities(&self) -> AdapterCapabilities {
        self.spec.capabilities()
    }
}

impl DaemonClient for SimulatedClient {
    fn connect_or_launch(&self) -> Result<(), DaemonClientError> {
        self.registry
            .connect(&self.worktree_root, &self.spec)
            .map(|_| ())
            .map_err(HarnessError::into_client_error)
    }

    fn send_event(&self, event: &DaemonEvent) -> Result<DaemonDecision, DaemonClientError> {
        self.registry
            .record_event(&self.worktree_root, event.clone())
            .map(|()| DaemonDecision::allow(Confidence::Verified))
            .map_err(HarnessError::into_client_error)
    }
}

/// One client's launch result.
#[derive(Debug, Clone, PartialEq)]
pub struct ClientLaunch {
    client_name: String,
    generation: DaemonGeneration,
    heartbeat_decision: DaemonDecision,
}

impl ClientLaunch {
    /// Client that launched.
    #[must_use]
    pub fn client_name(&self) -> &str {
        &self.client_name
    }

    /// Daemon generation observed by this client.
    #[must_use]
    pub const fn generation(&self) -> DaemonGeneration {
        self.generation
    }

    /// Decision returned for the deterministic adapter heartbeat event.
    #[must_use]
    pub fn heartbeat_decision(&self) -> &DaemonDecision {
        &self.heartbeat_decision
    }
}

/// Recorded capability registration for one simulated client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityRegistration {
    client_name: String,
    generation: DaemonGeneration,
    capabilities: AdapterCapabilities,
}

impl CapabilityRegistration {
    /// Registered client name.
    #[must_use]
    pub fn client_name(&self) -> &str {
        &self.client_name
    }

    /// Daemon generation active when registration occurred.
    #[must_use]
    pub const fn generation(&self) -> DaemonGeneration {
        self.generation
    }

    /// Registered adapter capability bitset.
    #[must_use]
    pub const fn capabilities(&self) -> AdapterCapabilities {
        self.capabilities
    }
}

/// Launch snapshot returned by [`Phase1DaemonFixture`].
#[derive(Debug, Clone, PartialEq)]
pub struct LaunchReport {
    worktree_root: PathBuf,
    client_launches: Vec<ClientLaunch>,
    capability_registrations: Vec<CapabilityRegistration>,
    events: Vec<DaemonEvent>,
}

impl LaunchReport {
    /// Worktree every simulated client attached to.
    #[must_use]
    pub fn worktree_root(&self) -> &Path {
        &self.worktree_root
    }

    /// Per-client launch records in deterministic launch order.
    #[must_use]
    pub fn client_launches(&self) -> &[ClientLaunch] {
        &self.client_launches
    }

    /// Capability registrations in deterministic registration order.
    #[must_use]
    pub fn capability_registrations(&self) -> &[CapabilityRegistration] {
        &self.capability_registrations
    }

    /// Events recorded by the simulated daemon.
    #[must_use]
    pub fn events(&self) -> &[DaemonEvent] {
        &self.events
    }

    /// Count of distinct daemon generations observed by launches or registrations.
    #[must_use]
    pub fn daemon_generation_count(&self) -> usize {
        self.observed_generations().len()
    }

    /// Returns the only observed generation, or an error if the fixture split.
    pub fn single_generation(&self) -> Result<DaemonGeneration, HarnessError> {
        let generations = self.observed_generations();

        match generations.len() {
            0 => Err(HarnessError::NoDaemonGeneration),
            1 => generations
                .first()
                .copied()
                .ok_or(HarnessError::NoDaemonGeneration),
            _ => Err(HarnessError::MultipleDaemonGenerations(
                generations.into_iter().collect(),
            )),
        }
    }

    fn observed_generations(&self) -> BTreeSet<DaemonGeneration> {
        let mut generations = self
            .client_launches
            .iter()
            .map(ClientLaunch::generation)
            .collect::<BTreeSet<_>>();
        generations.extend(
            self.capability_registrations
                .iter()
                .map(CapabilityRegistration::generation),
        );
        generations
    }
}

/// Errors surfaced by the deterministic harness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HarnessError {
    /// A client spec used an empty or whitespace-only name.
    EmptyClientName,
    /// Two configured client specs have the same registration name.
    DuplicateClientName(String),
    /// One client attempted to register different capabilities under an existing
    /// client name.
    ConflictingCapabilities(String),
    /// No daemon was connected or launched for the requested worktree.
    MissingDaemon(PathBuf),
    /// The shared in-memory registry was poisoned.
    RegistryPoisoned,
    /// The daemon-client trait returned a degraded error.
    ClientDegraded(String),
    /// No launch or registration observed a daemon generation.
    NoDaemonGeneration,
    /// More than one generation was observed by launches or registrations.
    MultipleDaemonGenerations(Vec<DaemonGeneration>),
    /// Client count exceeded the deterministic timestamp range.
    TooManyClients(usize),
    /// A concurrent simulated client thread panicked.
    ClientThreadPanicked,
    /// A custom event targeted a different worktree than the fixture owns.
    MismatchedWorktreeId { expected: String, found: String },
}

impl HarnessError {
    fn from_client_error(error: DaemonClientError) -> Self {
        Self::ClientDegraded(error.to_string())
    }

    fn into_client_error(self) -> DaemonClientError {
        DaemonClientError::Unavailable(self.to_string())
    }
}

impl fmt::Display for HarnessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyClientName => write!(formatter, "client name cannot be empty"),
            Self::DuplicateClientName(name) => write!(formatter, "duplicate client name: {name}"),
            Self::ConflictingCapabilities(name) => {
                write!(formatter, "conflicting capabilities for client: {name}")
            }
            Self::MissingDaemon(worktree_root) => {
                write!(
                    formatter,
                    "missing simulated daemon for {}",
                    worktree_root.display()
                )
            }
            Self::RegistryPoisoned => write!(formatter, "simulated daemon registry was poisoned"),
            Self::ClientDegraded(reason) => {
                write!(formatter, "simulated client degraded: {reason}")
            }
            Self::NoDaemonGeneration => write!(formatter, "no daemon generation was observed"),
            Self::MultipleDaemonGenerations(generations) => {
                write!(
                    formatter,
                    "multiple daemon generations were observed: {generations:?}"
                )
            }
            Self::TooManyClients(count) => {
                write!(
                    formatter,
                    "too many clients for deterministic timestamp range: {count}"
                )
            }
            Self::ClientThreadPanicked => write!(formatter, "simulated client thread panicked"),
            Self::MismatchedWorktreeId { expected, found } => write!(
                formatter,
                "event worktree id `{found}` does not match fixture worktree id `{expected}`"
            ),
        }
    }
}

impl Error for HarnessError {}

#[derive(Debug, Clone, Default)]
struct SharedDaemonRegistry {
    inner: Arc<Mutex<SimulatedDaemonRegistry>>,
}

impl SharedDaemonRegistry {
    fn connect(
        &self,
        worktree_root: &Path,
        spec: &SimulatedClientSpec,
    ) -> Result<DaemonGeneration, HarnessError> {
        let mut registry = self.lock()?;
        registry.connect(worktree_root, spec)
    }

    fn record_event(&self, worktree_root: &Path, event: DaemonEvent) -> Result<(), HarnessError> {
        let mut registry = self.lock()?;
        registry.record_event(worktree_root, event)
    }

    fn generation_for(&self, worktree_root: &Path) -> Result<DaemonGeneration, HarnessError> {
        let registry = self.lock()?;
        registry.generation_for(worktree_root)
    }

    fn snapshot(&self, worktree_root: &Path) -> Result<DaemonSnapshot, HarnessError> {
        let registry = self.lock()?;
        registry.snapshot(worktree_root)
    }

    fn lock(&self) -> Result<MutexGuard<'_, SimulatedDaemonRegistry>, HarnessError> {
        self.inner
            .lock()
            .map_err(|_| HarnessError::RegistryPoisoned)
    }
}

#[derive(Debug, Clone)]
struct SimulatedDaemonRegistry {
    next_generation: u64,
    daemons: BTreeMap<PathBuf, SimulatedDaemonState>,
}

impl Default for SimulatedDaemonRegistry {
    fn default() -> Self {
        Self {
            next_generation: FIRST_DAEMON_GENERATION.get(),
            daemons: BTreeMap::new(),
        }
    }
}

impl SimulatedDaemonRegistry {
    fn connect(
        &mut self,
        worktree_root: &Path,
        spec: &SimulatedClientSpec,
    ) -> Result<DaemonGeneration, HarnessError> {
        match self.daemons.entry(worktree_root.to_path_buf()) {
            Entry::Occupied(entry) => entry.into_mut().record_registration(spec),
            Entry::Vacant(entry) => {
                let generation = DaemonGeneration(self.next_generation);
                self.next_generation += 1;
                entry
                    .insert(SimulatedDaemonState::new(generation))
                    .record_registration(spec)
            }
        }
    }

    fn record_event(
        &mut self,
        worktree_root: &Path,
        event: DaemonEvent,
    ) -> Result<(), HarnessError> {
        validate_event_worktree(worktree_root, &event)?;
        let daemon = self
            .daemons
            .get_mut(worktree_root)
            .ok_or_else(|| HarnessError::MissingDaemon(worktree_root.to_path_buf()))?;
        daemon.events.push(event);
        Ok(())
    }

    fn generation_for(&self, worktree_root: &Path) -> Result<DaemonGeneration, HarnessError> {
        self.daemons
            .get(worktree_root)
            .map(SimulatedDaemonState::generation)
            .ok_or_else(|| HarnessError::MissingDaemon(worktree_root.to_path_buf()))
    }

    fn snapshot(&self, worktree_root: &Path) -> Result<DaemonSnapshot, HarnessError> {
        self.daemons
            .get(worktree_root)
            .map(SimulatedDaemonState::snapshot)
            .ok_or_else(|| HarnessError::MissingDaemon(worktree_root.to_path_buf()))
    }
}

#[derive(Debug, Clone)]
struct SimulatedDaemonState {
    generation: DaemonGeneration,
    capability_registrations: BTreeMap<String, CapabilityRegistration>,
    events: Vec<DaemonEvent>,
}

impl SimulatedDaemonState {
    fn new(generation: DaemonGeneration) -> Self {
        Self {
            generation,
            capability_registrations: BTreeMap::new(),
            events: Vec::new(),
        }
    }

    const fn generation(&self) -> DaemonGeneration {
        self.generation
    }

    fn record_registration(
        &mut self,
        spec: &SimulatedClientSpec,
    ) -> Result<DaemonGeneration, HarnessError> {
        match self.capability_registrations.get(spec.client_name()) {
            Some(existing) if existing.capabilities() == spec.capabilities() => Ok(self.generation),
            Some(existing) => Err(HarnessError::ConflictingCapabilities(
                existing.client_name.clone(),
            )),
            None => {
                self.capability_registrations.insert(
                    spec.client_name.clone(),
                    CapabilityRegistration {
                        client_name: spec.client_name.clone(),
                        generation: self.generation,
                        capabilities: spec.capabilities,
                    },
                );
                Ok(self.generation)
            }
        }
    }

    fn snapshot(&self) -> DaemonSnapshot {
        DaemonSnapshot {
            capability_registrations: self.capability_registrations.values().cloned().collect(),
            events: self.events.clone(),
        }
    }
}

struct IndexedLaunch {
    index: usize,
    launch: ClientLaunch,
}

#[derive(Debug, Clone, Default)]
struct DaemonSnapshot {
    capability_registrations: Vec<CapabilityRegistration>,
    events: Vec<DaemonEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct EventOrderKey {
    timestamp: Timestamp,
    kind_order: u8,
    adapter_id: String,
    session_id: String,
    tie_breaker: String,
}

impl EventOrderKey {
    fn new(timestamp: Timestamp, kind_order: u8, adapter_id: &str, session_id: &str) -> Self {
        Self {
            timestamp,
            kind_order,
            adapter_id: adapter_id.to_owned(),
            session_id: session_id.to_owned(),
            tie_breaker: String::new(),
        }
    }

    fn with_tie_breaker(mut self, event: &DaemonEvent) -> Self {
        self.tie_breaker = format!("{event:?}");
        self
    }
}

fn deterministic_events(mut events: Vec<DaemonEvent>) -> Vec<DaemonEvent> {
    events.sort_by_key(event_order_key);
    events
}

fn event_order_key(event: &DaemonEvent) -> EventOrderKey {
    let key = match event {
        DaemonEvent::SessionStart(payload) => EventOrderKey::new(
            payload.started_at,
            0,
            payload.adapter.adapter_id.as_str(),
            payload.session_id.as_str(),
        ),
        DaemonEvent::SessionEnd(payload) => EventOrderKey::new(
            payload.ended_at,
            1,
            payload.adapter.adapter_id.as_str(),
            payload.session_id.as_str(),
        ),
        DaemonEvent::ToolIntent(payload) => EventOrderKey::new(
            payload.occurred_at,
            2,
            payload.adapter.adapter_id.as_str(),
            payload.session_id.as_str(),
        ),
        DaemonEvent::ToolResult(payload) => EventOrderKey::new(
            payload.occurred_at,
            3,
            payload.adapter.adapter_id.as_str(),
            payload.session_id.as_str(),
        ),
        DaemonEvent::ReadObserved(payload) => EventOrderKey::new(
            payload.observed_at,
            4,
            payload.adapter.adapter_id.as_str(),
            payload.session_id.as_str(),
        ),
        DaemonEvent::EditIntent(payload) => EventOrderKey::new(
            payload.occurred_at,
            5,
            payload.adapter.adapter_id.as_str(),
            payload.session_id.as_str(),
        ),
        DaemonEvent::EditApplied(payload) => EventOrderKey::new(
            payload.applied_at,
            6,
            payload.adapter.adapter_id.as_str(),
            payload.session_id.as_str(),
        ),
        DaemonEvent::CommandIntent(payload) => EventOrderKey::new(
            payload.occurred_at,
            7,
            payload.adapter.adapter_id.as_str(),
            payload.session_id.as_str(),
        ),
        DaemonEvent::CommandResult(payload) => EventOrderKey::new(
            payload.occurred_at,
            8,
            payload.adapter.adapter_id.as_str(),
            payload.session_id.as_str(),
        ),
        DaemonEvent::CompactIntent(payload) => EventOrderKey::new(
            payload.occurred_at,
            9,
            payload.adapter.adapter_id.as_str(),
            payload.session_id.as_str(),
        ),
        DaemonEvent::VcsStateChanged(payload) => EventOrderKey::new(
            payload.changed_at,
            10,
            payload
                .adapter
                .as_ref()
                .map(|adapter| adapter.adapter_id.as_str())
                .unwrap_or_default(),
            payload
                .session_id
                .as_ref()
                .map(SessionId::as_str)
                .unwrap_or_default(),
        ),
        DaemonEvent::AdapterHeartbeat(payload) => EventOrderKey::new(
            payload.sent_at,
            11,
            payload.adapter.adapter_id.as_str(),
            payload
                .session_id
                .as_ref()
                .map(SessionId::as_str)
                .unwrap_or_default(),
        ),
    };

    key.with_tie_breaker(event)
}

fn validate_event_worktree(worktree_root: &Path, event: &DaemonEvent) -> Result<(), HarnessError> {
    let expected = worktree_id_value(worktree_root);
    let found = event_worktree_id(event).as_str();
    if found == expected {
        Ok(())
    } else {
        Err(HarnessError::MismatchedWorktreeId {
            expected,
            found: found.to_owned(),
        })
    }
}

fn event_worktree_id(event: &DaemonEvent) -> &WorktreeId {
    match event {
        DaemonEvent::SessionStart(payload) => &payload.worktree_id,
        DaemonEvent::SessionEnd(payload) => &payload.worktree_id,
        DaemonEvent::ToolIntent(payload) => &payload.worktree_id,
        DaemonEvent::ToolResult(payload) => &payload.worktree_id,
        DaemonEvent::ReadObserved(payload) => &payload.worktree_id,
        DaemonEvent::EditIntent(payload) => &payload.worktree_id,
        DaemonEvent::EditApplied(payload) => &payload.worktree_id,
        DaemonEvent::CommandIntent(payload) => &payload.worktree_id,
        DaemonEvent::CommandResult(payload) => &payload.worktree_id,
        DaemonEvent::CompactIntent(payload) => &payload.worktree_id,
        DaemonEvent::VcsStateChanged(payload) => &payload.worktree_id,
        DaemonEvent::AdapterHeartbeat(payload) => &payload.worktree_id,
    }
}

fn ensure_unique_client_names(client_specs: &[SimulatedClientSpec]) -> Result<(), HarnessError> {
    let mut names = BTreeSet::new();
    for spec in client_specs {
        if !names.insert(spec.client_name()) {
            return Err(HarnessError::DuplicateClientName(
                spec.client_name().to_owned(),
            ));
        }
    }

    Ok(())
}

fn deterministic_timestamp(index: usize) -> Result<Timestamp, HarnessError> {
    let zero_based = i64::try_from(index).map_err(|_| HarnessError::TooManyClients(index))?;
    let one_based = zero_based
        .checked_add(1)
        .ok_or(HarnessError::TooManyClients(index))?;
    Ok(Timestamp(one_based))
}

fn worktree_id_value(worktree_root: &Path) -> String {
    format!("harness-sim:{}", worktree_root.display())
}
