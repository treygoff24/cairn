use cairn_daemon_client::DaemonClient;
use cairn_harness_sim::{
    FIRST_DAEMON_GENERATION, HarnessError, LaunchReport, Phase1DaemonFixture, SimulatedClient,
    SimulatedClientSpec, observing_capabilities,
};
use cairn_protocol::{
    AdapterKind, AdapterRef, Confidence, DaemonDecision, DaemonDecisionKind, DaemonEvent,
    SessionStart,
};
use cairn_types::{AdapterCapabilities, SessionId, Timestamp, WorktreeId};
use tempfile::tempdir;

#[test]
fn n_simulated_clients_share_one_daemon_generation() {
    let worktree = tempdir().expect("temp worktree should be created");
    let capabilities = observing_capabilities();
    let fixture = Phase1DaemonFixture::with_n_clients(worktree.path(), 6, capabilities)
        .expect("fixture clients should be valid");

    let report = fixture.launch().expect("simulated clients should launch");

    assert_eq!(report.worktree_root(), worktree.path());
    assert_eq!(report.client_launches().len(), 6);
    assert_eq!(report.daemon_generation_count(), 1);
    assert_eq!(
        report
            .single_generation()
            .expect("one daemon generation should be present"),
        FIRST_DAEMON_GENERATION
    );

    for launch in report.client_launches() {
        assert_eq!(launch.generation(), FIRST_DAEMON_GENERATION);
        assert_eq!(
            launch.heartbeat_decision(),
            &DaemonDecision::allow(Confidence::Verified)
        );
    }
}

#[test]
fn fixture_records_capability_registrations_and_heartbeats_in_order() {
    let worktree = tempdir().expect("temp worktree should be created");
    let capabilities = observing_capabilities();
    let fixture = Phase1DaemonFixture::with_n_clients(worktree.path(), 3, capabilities)
        .expect("fixture clients should be valid");

    let report = fixture.launch().expect("simulated clients should launch");

    let registered_names = report
        .capability_registrations()
        .iter()
        .map(|registration| registration.client_name())
        .collect::<Vec<_>>();
    assert_eq!(
        registered_names,
        vec!["sim-client-0001", "sim-client-0002", "sim-client-0003"]
    );

    assert_capabilities_and_heartbeats(&report, capabilities, [1, 2, 3]);

    for event in report.events() {
        let DaemonEvent::AdapterHeartbeat(heartbeat) = event else {
            panic!("unexpected event recorded: {event:?}");
        };
        assert_eq!(
            heartbeat.harness.adapter_kind,
            cairn_protocol::AdapterKind::HarnessSim
        );
        assert_eq!(heartbeat.capabilities, capabilities);
        assert_eq!(heartbeat.daemon_generation_id.as_deref(), Some("1"));
    }

    for launch in report.client_launches() {
        assert_eq!(
            launch.heartbeat_decision().decision_kind,
            DaemonDecisionKind::Allow
        );
    }
}

#[test]
fn concurrent_launches_still_share_one_daemon_generation() {
    let worktree = tempdir().expect("temp worktree should be created");
    let capabilities = observing_capabilities();
    let fixture = Phase1DaemonFixture::with_n_clients(worktree.path(), 24, capabilities)
        .expect("fixture clients should be valid");

    let report = fixture
        .launch_concurrently()
        .expect("simulated clients should launch concurrently");

    assert_eq!(report.client_launches().len(), 24);
    assert_eq!(report.capability_registrations().len(), 24);
    assert_eq!(report.events().len(), 24);
    assert_eq!(report.daemon_generation_count(), 1);
    assert_eq!(
        report
            .single_generation()
            .expect("one daemon generation should be present"),
        FIRST_DAEMON_GENERATION
    );

    let launched_names = report
        .client_launches()
        .iter()
        .map(|launch| launch.client_name().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        launched_names,
        (1..=24)
            .map(|index| format!("sim-client-{index:04}"))
            .collect::<Vec<_>>()
    );

    for launch in report.client_launches() {
        assert_eq!(launch.generation(), FIRST_DAEMON_GENERATION);
        assert_eq!(
            launch.heartbeat_decision(),
            &DaemonDecision::allow(Confidence::Verified)
        );
    }

    assert_capabilities_and_heartbeats(&report, capabilities, 1..=24);
}

#[test]
fn custom_client_specs_record_distinct_capability_sets() {
    let worktree = tempdir().expect("temp worktree should be created");
    let observing = observing_capabilities();
    let blocking = blocking_capabilities();
    let specs = vec![
        SimulatedClientSpec::new("codex", observing).expect("codex spec should be valid"),
        SimulatedClientSpec::new("claude-code", blocking)
            .expect("claude-code spec should be valid"),
    ];
    let fixture = Phase1DaemonFixture::with_client_specs(worktree.path(), specs)
        .expect("fixture clients should be valid");

    let report = fixture.launch().expect("simulated clients should launch");

    let registrations = report.capability_registrations();
    assert_eq!(registrations.len(), 2);
    assert_eq!(registrations[0].client_name(), "claude-code");
    assert_eq!(registrations[0].capabilities(), blocking);
    assert_eq!(registrations[1].client_name(), "codex");
    assert_eq!(registrations[1].capabilities(), observing);

    for event in report.events() {
        let DaemonEvent::AdapterHeartbeat(heartbeat) = event else {
            panic!("unexpected event recorded: {event:?}");
        };
        let expected_capabilities = match heartbeat.harness.adapter_id.as_str() {
            "codex" => observing,
            "claude-code" => blocking,
            other => panic!("unexpected heartbeat harness: {other}"),
        };
        assert_eq!(heartbeat.capabilities, expected_capabilities);
    }
}

#[test]
fn snapshot_supports_phase2_custom_events_without_relaunching() {
    let worktree = tempdir().expect("temp worktree should be created");
    let capabilities = observing_capabilities();
    let (fixture, client) = single_client_fixture(worktree.path(), capabilities);

    client
        .connect_or_launch()
        .expect("simulated client should attach");

    let mut session_start = session_start(
        "phase2-session",
        WorktreeId::new(worktree_id_value(worktree.path())),
        client.client_name(),
        capabilities,
        42,
    );
    session_start.task_id = Some("phase2-task".to_owned());
    session_start.task_summary = Some("phase2 custom event smoke".to_owned());
    let session_start = DaemonEvent::SessionStart(session_start);

    client
        .send_event(&session_start)
        .expect("simulated daemon should record custom event");

    let report = fixture
        .snapshot()
        .expect("snapshot should inspect existing simulated daemon state");

    assert!(report.client_launches().is_empty());
    assert_eq!(report.capability_registrations().len(), 1);
    assert_eq!(report.events(), std::slice::from_ref(&session_start));
    assert_eq!(report.daemon_generation_count(), 1);
    assert_eq!(
        report
            .single_generation()
            .expect("registration should prove one daemon generation"),
        FIRST_DAEMON_GENERATION
    );
}

#[test]
fn custom_events_for_other_worktrees_are_rejected() {
    let worktree = tempdir().expect("temp worktree should be created");
    let capabilities = observing_capabilities();
    let (_fixture, client) = single_client_fixture(worktree.path(), capabilities);

    client
        .connect_or_launch()
        .expect("simulated client should attach");

    let event = DaemonEvent::SessionStart(session_start(
        "wrong-worktree-session",
        WorktreeId::new("different-worktree"),
        client.client_name(),
        capabilities,
        77,
    ));

    let error = client
        .send_event(&event)
        .expect_err("fixture must reject events outside its worktree");
    assert!(
        error
            .to_string()
            .contains("does not match fixture worktree")
    );
}

#[test]
fn duplicate_client_names_are_rejected_before_launch() {
    let worktree = tempdir().expect("temp worktree should be created");
    let capabilities = observing_capabilities();
    let specs = vec![
        SimulatedClientSpec::new("duplicate", capabilities).expect("first spec should be valid"),
        SimulatedClientSpec::new("duplicate", capabilities).expect("second spec should be valid"),
    ];

    let error = Phase1DaemonFixture::with_client_specs(worktree.path(), specs)
        .expect_err("duplicate client names should fail before launch");

    assert_eq!(error, HarnessError::DuplicateClientName("duplicate".into()));
}

fn worktree_id_value(worktree_root: &std::path::Path) -> String {
    format!("harness-sim:{}", worktree_root.display())
}

fn single_client_fixture(
    worktree_root: &std::path::Path,
    capabilities: AdapterCapabilities,
) -> (Phase1DaemonFixture, SimulatedClient) {
    let fixture = Phase1DaemonFixture::with_n_clients(worktree_root, 1, capabilities)
        .expect("fixture clients should be valid");
    let client = fixture
        .simulated_clients()
        .into_iter()
        .next()
        .expect("one simulated client should be present");
    (fixture, client)
}

fn assert_capabilities_and_heartbeats(
    report: &LaunchReport,
    capabilities: AdapterCapabilities,
    expected_timestamps: impl IntoIterator<Item = i64>,
) {
    for registration in report.capability_registrations() {
        assert_eq!(registration.generation(), FIRST_DAEMON_GENERATION);
        assert_eq!(registration.capabilities(), capabilities);
    }

    let heartbeat_timestamps = report
        .events()
        .iter()
        .map(|event| match event {
            DaemonEvent::AdapterHeartbeat(heartbeat) => heartbeat.sent_at.0,
            other => panic!("unexpected event recorded: {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        heartbeat_timestamps,
        expected_timestamps.into_iter().collect::<Vec<_>>()
    );
}

fn session_start(
    agent_session_id: &str,
    worktree_id: WorktreeId,
    client_name: &str,
    capabilities: AdapterCapabilities,
    started_at: i64,
) -> SessionStart {
    let agent_session_id = SessionId::new(agent_session_id);
    SessionStart {
        root_session_id: agent_session_id.clone(),
        agent_session_id,
        parent_session_id: None,
        spawn_event_id: None,
        lineage_depth: 0,
        worktree_id,
        harness: AdapterRef {
            adapter_id: client_name.to_owned(),
            adapter_kind: AdapterKind::HarnessSim,
        },
        capabilities,
        repo_epoch: None,
        started_at: Timestamp(started_at),
        task_id: None,
        task_summary: None,
        inherited_context_frame_ids: Vec::new(),
    }
}

fn blocking_capabilities() -> AdapterCapabilities {
    AdapterCapabilities {
        can_pre_edit_block: true,
        can_pre_read_decorate: true,
        can_post_read_decorate: true,
        can_command_replace: false,
        can_modify_tool_input: true,
        can_async_notify: true,
        can_precompact: true,
        can_report_token_usage: true,
        can_report_exact_edit_diff: true,
        can_attach_file_precondition: true,
    }
}
