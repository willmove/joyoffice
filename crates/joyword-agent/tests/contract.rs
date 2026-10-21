//! T400 contract: default off, kill switch, fake provider, policy, redaction.
//! No socket is opened.

use std::sync::{Arc, Barrier};
use std::thread;

use joyword_agent::{
    AdminPolicy, AgentRuntime, AiConfig, AiGate, AiPhase, AigcStatus, ApplyBlocked, ChangeProposal,
    CommitDecision, CredentialStore, Endpoint, FakeProvider, GateError, HttpProvider,
    LogCredential, MemoryCredentialStore, OsPolicyBackend, PolicySource, Provider, ProviderError,
    ProviderId, RecordingTransport, Script, Secret, StaticPolicy, ToolId, UnavailableSystemStore,
    UnmanagedPolicy, labeling_status, redact, write_label,
};

#[test]
fn first_launch_startup_makes_zero_provider_requests() {
    let runtime = AgentRuntime::boot_disabled();
    let report = runtime.startup_sequence();
    assert_eq!(report.steps, 3);
    assert_eq!(report.provider_calls, 0);
    assert_eq!(report.invocations, 0);
    assert_eq!(report.upload_attempts, 0);
    assert_eq!(report.log_entries, 0);
    assert_eq!(runtime.document_mutations(), 0);
    let settings = runtime.settings().expect("settings");
    assert!(!settings.user_enabled);
    assert!(!settings.effective_enabled);
    assert!(!settings.managed);
    assert!(settings.entry_available);
    assert!(settings.reason.is_none());
}

#[test]
fn enabled_startup_still_makes_zero_provider_requests() {
    let mut config = AiConfig::default();
    config.user_enabled = true;
    config.provider = joyword_agent::ProviderSelection::named("fake").expect("id");
    let mut runtime = AgentRuntime::new(
        config,
        AdminPolicy::unmanaged(),
        Box::new(FakeProvider::scripted("fake", vec![Script::Reply("nope".into())]).expect("id")),
        Box::new(MemoryCredentialStore::new()),
    );
    let report = runtime.startup_sequence();
    assert_eq!(report.provider_calls, 0);
    assert_eq!(report.log_entries, 0);
    assert!(runtime.settings().expect("settings").effective_enabled);
    let response = runtime.complete("hello").expect("reply");
    assert_eq!(response.text, "nope");
    assert_eq!(runtime.provider_calls(), 1);
    assert_eq!(runtime.log().len(), 1);
    assert_eq!(runtime.log().entries()[0].prompt_bytes, 5);
    assert_eq!(runtime.log().entries()[0].credential, LogCredential::Absent);
    assert_eq!(runtime.allowed_commits(), 0);
    assert_eq!(runtime.document_mutations(), 0);
    runtime.kill_switch().expect("off");
    let after = runtime.provider_calls();
    let error = runtime.complete("again").expect_err("disabled");
    assert_eq!(error.kind(), "disabled");
    assert_eq!(runtime.provider_calls(), after);
    assert!(runtime.note_failure(error).is_none());
}

#[test]
fn disabled_complete_does_not_touch_the_provider() {
    let runtime = AgentRuntime::boot_disabled();
    let error = runtime.complete("hello").expect_err("off");
    assert_eq!(error.kind(), "disabled");
    assert_eq!(runtime.provider_calls(), 0);
    assert!(runtime.log().is_empty());
    assert_eq!(runtime.allowed_commits(), 0);
}

#[test]
fn admin_force_disable_blocks_enable_and_requests() {
    let mut runtime = AgentRuntime::boot_disabled();
    runtime
        .apply_policy(AdminPolicy::force_disabled())
        .expect("policy");
    let error = runtime.set_user_enabled(true).expect_err("managed");
    assert_eq!(error.kind(), "managed-disabled");
    let settings = runtime.settings().expect("settings");
    assert!(settings.managed);
    assert!(settings.force_disabled);
    assert!(!settings.entry_available);
    assert!(!settings.effective_enabled);
    assert_eq!(
        runtime.complete("x").expect_err("forced").kind(),
        "managed-disabled"
    );
    assert_eq!(runtime.provider_calls(), 0);
    assert_eq!(runtime.startup_sequence().provider_calls, 0);
}

#[test]
fn provider_and_endpoint_allowlists_refuse_before_transport() {
    let transport = RecordingTransport::new();
    let provider =
        HttpProvider::new("cloud", "https://models.example/v1", transport).expect("http");
    let endpoint = provider.endpoint().clone();
    let mut config = AiConfig::default();
    config.user_enabled = true;
    config.provider =
        joyword_agent::ProviderSelection::http("cloud", endpoint.as_str()).expect("cfg");
    let mut policy = AdminPolicy::unmanaged();
    policy.managed = true;
    policy.provider_allowlist = Some([ProviderId::parse("other").expect("id")].into());
    let runtime = AgentRuntime::new(
        config.clone(),
        policy,
        Box::new(provider),
        Box::new(MemoryCredentialStore::new()),
    );
    assert_eq!(
        runtime.complete("ping").expect_err("provider").kind(),
        "provider-not-allowed"
    );
    assert_eq!(runtime.provider_calls(), 0);

    let transport = RecordingTransport::new();
    let provider =
        HttpProvider::new("cloud", "https://models.example/v1", transport).expect("http");
    let mut policy = AdminPolicy::unmanaged();
    policy.endpoint_allowlist = Some(["https://allowed.example/v1".to_owned()].into());
    let runtime = AgentRuntime::new(
        config,
        policy,
        Box::new(provider),
        Box::new(MemoryCredentialStore::new()),
    );
    assert_eq!(
        runtime.complete("ping").expect_err("endpoint").kind(),
        "endpoint-not-allowed"
    );
    assert_eq!(runtime.provider_calls(), 0);
    assert_eq!(runtime.allowed_commits(), 0);
}

#[test]
fn http_transport_redacts_the_credential_and_records_one_exchange() {
    let transport = Arc::new(RecordingTransport::new());
    let provider = HttpProvider::new(
        "cloud",
        "https://models.example/v1",
        ArcTransport(Arc::clone(&transport)),
    )
    .expect("http");
    let mut config = AiConfig::default();
    config.user_enabled = true;
    config.provider =
        joyword_agent::ProviderSelection::http("cloud", "https://models.example/v1").expect("cfg");
    config.credential_slot = Some("cloud.primary".into());
    let credentials = MemoryCredentialStore::new();
    let secret = Secret::new("super-secret-value").expect("secret");
    credentials
        .put("cloud.primary", secret.clone())
        .expect("put");
    let runtime = AgentRuntime::new(
        config,
        AdminPolicy::unmanaged(),
        Box::new(provider),
        Box::new(credentials),
    );
    let response = runtime.complete("ping").expect("ok");
    assert_eq!(response.text, "recorded");
    let exchanges = transport.exchanges();
    assert_eq!(exchanges.len(), 1);
    assert_eq!(exchanges[0].authorization, LogCredential::Redacted);
    assert_eq!(exchanges[0].url, "https://models.example/v1");
    let rendered = format!("{exchanges:?}");
    assert!(!rendered.contains("super-secret-value"));
    assert!(!format!("{:?}", Secret::new("super-secret-value").expect("s")).contains("super"));
    assert_eq!(
        runtime.log().entries()[0].credential,
        LogCredential::Redacted
    );
    assert!(!format!("{:?}", runtime.log().entries()).contains("super-secret-value"));
}

#[test]
fn missing_credential_offline_and_rate_limit_do_not_admit_a_commit() {
    let mut config = AiConfig::default();
    config.user_enabled = true;
    config.provider = joyword_agent::ProviderSelection::named("fake").expect("id");
    config.credential_slot = Some("slot".into());
    let runtime = AgentRuntime::new(
        config.clone(),
        AdminPolicy::unmanaged(),
        Box::new(FakeProvider::scripted("fake", vec![Script::Reply("x".into())]).expect("fake")),
        Box::new(MemoryCredentialStore::new()),
    );
    assert_eq!(
        runtime.complete("hi").expect_err("key").kind(),
        "missing-credential"
    );
    assert_eq!(runtime.provider_calls(), 0);
    assert_eq!(runtime.allowed_commits(), 0);

    config.credential_slot = None;
    for (script, kind) in [
        (Script::Fail(ProviderError::Offline), "offline"),
        (Script::Fail(ProviderError::RateLimited), "rate-limited"),
    ] {
        let runtime = AgentRuntime::new(
            config.clone(),
            AdminPolicy::unmanaged(),
            Box::new(FakeProvider::scripted("fake", vec![script]).expect("fake")),
            Box::new(MemoryCredentialStore::new()),
        );
        assert_eq!(runtime.complete("hi").expect_err(kind).kind(), kind);
        assert_eq!(runtime.provider_calls(), 1);
        assert_eq!(runtime.allowed_commits(), 0);
        assert_eq!(runtime.document_mutations(), 0);
    }
}

#[test]
fn system_credential_store_is_a_stub() {
    let mut config = AiConfig::default();
    config.user_enabled = true;
    config.provider = joyword_agent::ProviderSelection::named("fake").expect("id");
    config.credential_slot = Some("slot".into());
    let runtime = AgentRuntime::new(
        config,
        AdminPolicy::unmanaged(),
        Box::new(FakeProvider::scripted("fake", vec![Script::Reply("x".into())]).expect("fake")),
        Box::new(UnavailableSystemStore),
    );
    assert_eq!(
        runtime.complete("hi").expect_err("store").kind(),
        "credential"
    );
    assert_eq!(runtime.provider_calls(), 0);
}

#[test]
fn delete_credential_fails_closed_without_a_call() {
    let mut config = AiConfig::default();
    config.user_enabled = true;
    config.provider = joyword_agent::ProviderSelection::named("fake").expect("id");
    config.credential_slot = Some("slot".into());
    let mut runtime = AgentRuntime::new(
        config,
        AdminPolicy::unmanaged(),
        Box::new(FakeProvider::scripted("fake", vec![Script::Reply("x".into())]).expect("fake")),
        Box::new(MemoryCredentialStore::new()),
    );
    runtime
        .credentials_mut()
        .put("slot", Secret::new("abc").expect("secret"))
        .expect("put");
    runtime.credentials_mut().delete("slot").expect("delete");
    assert_eq!(
        runtime.complete("hi").expect_err("gone").kind(),
        "missing-credential"
    );
    assert_eq!(runtime.provider_calls(), 0);
}

#[test]
fn config_defaults_round_trip_and_reject_secrets() {
    let config = AiConfig::default();
    assert!(!config.user_enabled);
    assert!(!config.telemetry_enabled);
    assert!(config.provider.id().is_none());
    let parsed = AiConfig::parse(&config.serialize()).expect("parse");
    assert_eq!(parsed, config);

    let mut custom = AiConfig::default();
    custom.user_enabled = true;
    custom.provider =
        joyword_agent::ProviderSelection::http("local", "http://127.0.0.1:9/v1").expect("http");
    custom.credential_slot = Some("local.slot".into());
    custom.telemetry_enabled = false;
    let again = AiConfig::parse(&custom.serialize()).expect("parse");
    assert_eq!(again, custom);

    let rejected = AiConfig::parse("schema=1\napi_key=sk-live\n");
    assert!(rejected.is_err());
    let text = "schema=1\nai.enabled=false\n";
    assert!(!text.contains("sk-"));
    assert_eq!(
        redact("bearer super-secret-value", &["super-secret-value"]),
        "bearer [redacted]"
    );
    assert!(Endpoint::parse("https://user:pass@example/v1").is_err());
    assert!(Endpoint::parse("https://models.example/v1").is_ok());
    assert!(AiConfig::parse("schema=99\nai.enabled=true\n").is_err());
}

#[test]
fn kill_switch_bumps_generation_and_rejects_old_tickets() {
    let gate = AiGate::from_flags(true, false);
    let ticket = gate.begin_request().expect("begin");
    assert_eq!(gate.phase().expect("phase"), AiPhase::Running);
    assert_eq!(gate.decide_commit(ticket), CommitDecision::Allowed);
    gate.finish_request(ticket).expect("finish");
    let before = gate.generation().expect("gen");
    gate.begin_stop().expect("stopping");
    assert_eq!(gate.phase().expect("phase"), AiPhase::Stopping);
    assert!(matches!(gate.begin_request(), Err(GateError::Stopping)));
    assert_eq!(gate.decide_commit(ticket), CommitDecision::Rejected);
    assert_eq!(gate.generation().expect("gen"), before);
    let after = gate.commit_disabled().expect("linearize");
    assert_eq!(after.raw(), before.raw() + 1);
    assert_eq!(gate.phase().expect("phase"), AiPhase::Disabled);
    assert_eq!(gate.decide_commit(ticket), CommitDecision::Rejected);
    assert!(matches!(
        gate.decide_result(ticket),
        Err(GateError::Stale) | Err(GateError::Disabled)
    ));
    let again = gate.kill_switch().expect("idempotent");
    assert_eq!(again, after);
    assert!(gate.set_user_enabled(true).is_ok());
    assert!(matches!(gate.decide_result(ticket), Err(GateError::Stale)));
}

#[test]
fn disable_and_commit_are_mutually_exclusive() {
    for _ in 0..200 {
        let gate = Arc::new(AiGate::from_flags(true, false));
        let ticket = gate.begin_request().expect("begin");
        let barrier = Arc::new(Barrier::new(2));
        let left_gate = Arc::clone(&gate);
        let left_barrier = Arc::clone(&barrier);
        let right_gate = Arc::clone(&gate);
        let right_barrier = Arc::clone(&barrier);
        let decider = thread::spawn(move || {
            left_barrier.wait();
            left_gate.decide_commit(ticket)
        });
        let stopper = thread::spawn(move || {
            right_barrier.wait();
            right_gate.kill_switch().expect("stop")
        });
        let decision = decider.join().expect("decider");
        stopper.join().expect("stopper");
        assert_eq!(gate.phase().expect("phase"), AiPhase::Disabled);
        assert_eq!(gate.decide_commit(ticket), CommitDecision::Rejected);
        assert!(matches!(
            decision,
            CommitDecision::Allowed | CommitDecision::Rejected
        ));
    }
}

#[test]
fn duplicate_proposal_is_admitted_once_and_apply_is_blocked() {
    let mut config = AiConfig::default();
    config.user_enabled = true;
    config.provider = joyword_agent::ProviderSelection::named("fake").expect("id");
    let runtime = AgentRuntime::new(
        config,
        AdminPolicy::unmanaged(),
        Box::new(FakeProvider::scripted("fake", Vec::new()).expect("fake")),
        Box::new(MemoryCredentialStore::new()),
    );
    let ticket = runtime.issue_ticket().expect("ticket");
    assert_eq!(
        runtime.admit_commit(ticket, "proposal-1"),
        CommitDecision::Allowed
    );
    assert_eq!(
        runtime.admit_commit(ticket, "proposal-1"),
        CommitDecision::Duplicate
    );
    assert_eq!(runtime.allowed_commits(), 1);
    assert_eq!(runtime.provider_calls(), 0);
    assert_eq!(runtime.document_mutations(), 0);
}

#[test]
fn stale_commit_after_kill_switch_is_rejected() {
    let mut config = AiConfig::default();
    config.user_enabled = true;
    config.provider = joyword_agent::ProviderSelection::named("fake").expect("id");
    let mut runtime = AgentRuntime::new(
        config,
        AdminPolicy::unmanaged(),
        Box::new(FakeProvider::bomb()),
        Box::new(MemoryCredentialStore::new()),
    );
    let ticket = runtime.issue_ticket().expect("ticket");
    runtime.kill_switch().expect("off");
    assert_eq!(
        runtime.admit_commit(ticket, "proposal-1"),
        CommitDecision::Rejected
    );
    assert_eq!(runtime.allowed_commits(), 0);
    assert_eq!(runtime.provider_calls(), 0);
    let proposal = ChangeProposal {
        proposal_id: "proposal-1".into(),
        document_id: "doc".into(),
        base_version: 1,
        explanation: "not a command".into(),
    };
    assert_eq!(proposal.apply(), Err(ApplyBlocked::DeferredUntilT402));
}

#[test]
fn fake_provider_honors_cancel_and_does_not_debug_the_prompt() {
    let runtime = AgentRuntime::boot_disabled();
    let generation = runtime.generation().expect("generation");
    let fake = FakeProvider::scripted("fake", vec![Script::Reply("hidden".into())]).expect("fake");
    assert!(!fake.capabilities().streaming);
    assert!(!fake.capabilities().tools);
    let cancel = joyword_agent::CancelFlag::new();
    cancel.cancel();
    let secret = Secret::new("nope").expect("secret");
    let request = joyword_agent::CompletionRequest {
        prompt: "body",
        generation,
        cancel: &cancel,
        credential: Some(&secret),
    };
    assert_eq!(fake.complete(&request), Err(ProviderError::Cancelled));
    assert_eq!(fake.calls(), 1);
    let rendered = format!("{request:?}");
    assert!(!rendered.contains("body"));
    assert!(!rendered.contains("nope"));
    assert_eq!(runtime.provider_calls(), 0);
}

#[test]
fn os_policy_backends_are_stubs_and_tools_do_not_run() {
    let backend = OsPolicyBackend::for_target();
    assert!(backend.is_stub());
    assert_eq!(backend.load(), AdminPolicy::unmanaged());
    assert_eq!(UnmanagedPolicy.load(), AdminPolicy::unmanaged());
    let forced = StaticPolicy(AdminPolicy::force_disabled());
    assert!(forced.load().force_disabled);
    for tool in ToolId::ALL {
        assert!(tool.dispatch().is_err());
    }
    assert_eq!(labeling_status(), AigcStatus::AssessmentPending);
    assert_eq!(write_label(), Err(AigcStatus::AssessmentPending));
}

#[test]
fn core_manifests_do_not_name_the_agent_crate() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for rel in [
        "crates/joyword-foundation/Cargo.toml",
        "crates/joyword-model/Cargo.toml",
        "crates/joyword-engine/Cargo.toml",
        "crates/joyword-docx/Cargo.toml",
        "crates/joyword-layout/Cargo.toml",
        "crates/joyword-pdf/Cargo.toml",
        "crates/joyword-render/Cargo.toml",
        "crates/joyoffice-opc/Cargo.toml",
        "crates/joyword-corpus/Cargo.toml",
        "apps/joyword-cli/Cargo.toml",
        "apps/joyword/Cargo.toml",
    ] {
        let text = std::fs::read_to_string(root.join(rel)).expect(rel);
        assert!(!text.contains("joyword-agent"), "{rel}");
    }
    let shell =
        std::fs::read_to_string(root.join("crates/joyword-shell/Cargo.toml")).expect("shell");
    assert!(shell.contains("joyword-agent"));
    assert!(shell.contains("optional = true"));
    assert!(shell.contains("default = []"));
}

#[test]
fn telemetry_defaults_off_and_never_counts_an_upload() {
    let runtime = AgentRuntime::boot_disabled();
    assert_eq!(runtime.startup_sequence().upload_attempts, 0);
    let mut config = AiConfig::default();
    config.telemetry_enabled = true;
    let runtime = AgentRuntime::new(
        config,
        AdminPolicy::unmanaged(),
        Box::new(FakeProvider::bomb()),
        Box::new(MemoryCredentialStore::new()),
    );
    assert_eq!(
        runtime.note_failure(joyword_agent::AgentError::Prompt),
        Some("prompt")
    );
    assert_eq!(runtime.startup_sequence().upload_attempts, 0);
}

/// Shared recording transport. The provider is generic over the transport.
struct ArcTransport(Arc<RecordingTransport>);

impl joyword_agent::Transport for ArcTransport {
    fn exchange(&self, call: &joyword_agent::Outbound<'_>) -> Result<Vec<u8>, ProviderError> {
        self.0.exchange(call)
    }
}
