//! Startup and request admission. A disabled gate returns before the provider
//! is touched. Nothing in this type has a document handle.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::config::{AiConfig, ConfigError};
use crate::credential::{CredentialError, CredentialStore, Secret};
use crate::gate::{Admission, AiGate, AiPhase, CommitDecision, GateError, Generation};
use crate::policy::{AdminPolicy, ManagedReason, SettingsView};
use crate::provider::{
    CancelFlag, CompletionRequest, CompletionResponse, LogCredential, LoggedRequest, Provider,
    ProviderError, RequestLog,
};
use crate::telemetry::Telemetry;

const MAX_PROMPT_BYTES: usize = 32 * 1024;
const MAX_PROPOSALS: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentError {
    Gate(GateError),
    Provider(ProviderError),
    Credential(CredentialError),
    Config(ConfigError),
    Prompt,
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentError::Gate(error) => write!(f, "{error}"),
            AgentError::Provider(error) => write!(f, "{error}"),
            AgentError::Credential(error) => write!(f, "{error}"),
            AgentError::Config(error) => write!(f, "{error}"),
            AgentError::Prompt => f.write_str("prompt was rejected"),
        }
    }
}

impl std::error::Error for AgentError {}

impl AgentError {
    /// Static label safe to record. It does not include the prompt or a secret.
    pub const fn kind(self) -> &'static str {
        match self {
            AgentError::Gate(GateError::Disabled) => "disabled",
            AgentError::Gate(GateError::Stopping) => "stopping",
            AgentError::Gate(GateError::Stale) => "stale",
            AgentError::Gate(GateError::ManagedDisabled) => "managed-disabled",
            AgentError::Gate(GateError::GenerationExhausted) => "generation-exhausted",
            AgentError::Gate(GateError::Poisoned) => "poisoned",
            AgentError::Provider(ProviderError::MissingCredential) => "missing-credential",
            AgentError::Provider(ProviderError::Offline) => "offline",
            AgentError::Provider(ProviderError::RateLimited) => "rate-limited",
            AgentError::Provider(ProviderError::TimedOut) => "timed-out",
            AgentError::Provider(ProviderError::ContentFiltered) => "content-filtered",
            AgentError::Provider(ProviderError::InvalidToolParams) => "invalid-tool-params",
            AgentError::Provider(ProviderError::Cancelled) => "cancelled",
            AgentError::Provider(ProviderError::UnexpectedCall) => "unexpected-call",
            AgentError::Provider(ProviderError::EndpointNotAllowed) => "endpoint-not-allowed",
            AgentError::Provider(ProviderError::ProviderNotAllowed) => "provider-not-allowed",
            AgentError::Provider(ProviderError::NoProvider) => "no-provider",
            AgentError::Credential(_) => "credential",
            AgentError::Config(_) => "config",
            AgentError::Prompt => "prompt",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootReport {
    pub steps: u32,
    pub provider_calls: u64,
    pub invocations: u64,
    pub upload_attempts: u64,
    pub log_entries: usize,
}

struct Flight<'a> {
    gate: &'a AiGate,
    ticket: Admission,
}

impl Drop for Flight<'_> {
    fn drop(&mut self) {
        let _ = self.gate.finish_request(self.ticket);
    }
}

/// The AI session shell. `document_mutations` is always zero in this milestone.
pub struct AgentRuntime {
    gate: AiGate,
    config: AiConfig,
    policy: AdminPolicy,
    provider: Box<dyn Provider>,
    credentials: Box<dyn CredentialStore>,
    log: Arc<RequestLog>,
    telemetry: Telemetry,
    invocations: AtomicU64,
    allowed_commits: AtomicU64,
    proposals: Mutex<BTreeSet<String>>,
    boot_steps: AtomicU64,
}

impl AgentRuntime {
    pub fn new(
        config: AiConfig,
        policy: AdminPolicy,
        provider: Box<dyn Provider>,
        credentials: Box<dyn CredentialStore>,
    ) -> Self {
        let gate = AiGate::from_flags(config.user_enabled, policy.force_disabled);
        let telemetry = Telemetry::new(config.telemetry_enabled);
        Self {
            gate,
            config,
            policy,
            provider,
            credentials,
            log: Arc::new(RequestLog::new()),
            telemetry,
            invocations: AtomicU64::new(0),
            allowed_commits: AtomicU64::new(0),
            proposals: Mutex::new(BTreeSet::new()),
            boot_steps: AtomicU64::new(0),
        }
    }

    /// First launch: AI off, bomb provider, empty credential store.
    pub fn boot_disabled() -> Self {
        use crate::credential::MemoryCredentialStore;
        use crate::fake::FakeProvider;
        Self::new(
            AiConfig::default(),
            AdminPolicy::unmanaged(),
            Box::new(FakeProvider::bomb()),
            Box::new(MemoryCredentialStore::new()),
        )
    }

    pub fn log(&self) -> Arc<RequestLog> {
        Arc::clone(&self.log)
    }

    pub fn provider_calls(&self) -> u64 {
        self.provider.calls()
    }

    pub fn invocations(&self) -> u64 {
        self.invocations.load(Ordering::SeqCst)
    }

    pub fn allowed_commits(&self) -> u64 {
        self.allowed_commits.load(Ordering::SeqCst)
    }

    /// T400 has no document API. The value is fixed so a regression that
    /// starts mutating state has to remove this on purpose.
    pub const fn document_mutations(&self) -> u8 {
        0
    }

    pub fn generation(&self) -> Result<Generation, AgentError> {
        self.gate.generation().map_err(AgentError::Gate)
    }

    pub fn settings(&self) -> Result<SettingsView, AgentError> {
        let phase = self.gate.phase().map_err(AgentError::Gate)?;
        let forced = self.policy.force_disabled;
        let user = self.config.user_enabled;
        let effective = user && !forced && matches!(phase, AiPhase::EnabledIdle | AiPhase::Running);
        let reason = if forced {
            Some(ManagedReason::ForceDisabled)
        } else if self
            .config
            .provider
            .id()
            .is_some_and(|id| !self.policy.allows_provider(id))
        {
            Some(ManagedReason::ProviderAllowlist)
        } else if self
            .config
            .provider
            .endpoint()
            .is_some_and(|endpoint| !self.policy.allows_endpoint(endpoint))
        {
            Some(ManagedReason::EndpointAllowlist)
        } else {
            None
        };
        Ok(SettingsView {
            user_enabled: user,
            effective_enabled: effective,
            managed: self.policy.managed || forced,
            force_disabled: forced,
            entry_available: !forced,
            reason,
        })
    }

    pub fn set_user_enabled(&mut self, enabled: bool) -> Result<(), AgentError> {
        self.gate
            .set_user_enabled(enabled)
            .map_err(AgentError::Gate)?;
        self.config.user_enabled = enabled;
        Ok(())
    }

    pub fn apply_policy(&mut self, policy: AdminPolicy) -> Result<(), AgentError> {
        let currently = self.gate.policy_forced_off().map_err(AgentError::Gate)?;
        if policy.force_disabled != currently {
            self.gate
                .set_policy_forced_off(policy.force_disabled)
                .map_err(AgentError::Gate)?;
        }
        self.policy = policy;
        Ok(())
    }

    pub fn credentials_mut(&mut self) -> &mut dyn CredentialStore {
        self.credentials.as_mut()
    }

    /// First launch, open, and idle. These hooks do not call the provider,
    /// whether or not the user has enabled AI.
    pub fn startup_sequence(&self) -> BootReport {
        for _phase in [
            BootPhase::FirstLaunch,
            BootPhase::OpenDocument,
            BootPhase::Idle,
        ] {
            self.boot_steps.fetch_add(1, Ordering::SeqCst);
        }
        BootReport {
            steps: u32::try_from(self.boot_steps.load(Ordering::SeqCst)).unwrap_or(u32::MAX),
            provider_calls: self.provider.calls(),
            invocations: self.invocations(),
            upload_attempts: self.telemetry.upload_attempts(),
            log_entries: self.log.len(),
        }
    }

    pub fn note_failure(&self, error: AgentError) -> Option<&'static str> {
        self.telemetry.note_local(error.kind())
    }

    pub fn complete(&self, prompt: &str) -> Result<CompletionResponse, AgentError> {
        if prompt.is_empty()
            || prompt.len() > MAX_PROMPT_BYTES
            || prompt.chars().any(|ch| ch == '\0')
        {
            return Err(AgentError::Prompt);
        }
        let ticket = self.gate.begin_request().map_err(AgentError::Gate)?;
        let _flight = Flight {
            gate: &self.gate,
            ticket,
        };
        self.preflight()?;
        let credential = self.load_credential()?;
        self.gate.decide_result(ticket).map_err(AgentError::Gate)?;
        let provider_id = self
            .config
            .provider
            .id()
            .map(|id| id.as_str().to_owned())
            .unwrap_or_else(|| "none".to_owned());
        let recorded = self.log.push(LoggedRequest {
            provider_id,
            prompt_bytes: prompt.len(),
            credential: if credential.is_some() {
                LogCredential::Redacted
            } else {
                LogCredential::Absent
            },
        });
        if !recorded {
            return Err(AgentError::Gate(GateError::Poisoned));
        }
        self.invocations.fetch_add(1, Ordering::SeqCst);
        let cancel = CancelFlag::new();
        let request = CompletionRequest {
            prompt,
            generation: ticket.generation(),
            cancel: &cancel,
            credential: credential.as_ref(),
        };
        let response = self
            .provider
            .complete(&request)
            .map_err(AgentError::Provider)?;
        self.gate.decide_result(ticket).map_err(AgentError::Gate)?;
        Ok(response)
    }

    /// Permission hook for T402. Allowing a commit does not apply commands.
    pub fn admit_commit(&self, ticket: Admission, proposal_id: &str) -> CommitDecision {
        if !valid_proposal_id(proposal_id) {
            return CommitDecision::Rejected;
        }
        if self.gate.decide_commit(ticket) != CommitDecision::Allowed {
            return CommitDecision::Rejected;
        }
        let Ok(mut ids) = self.proposals.lock() else {
            return CommitDecision::Rejected;
        };
        if ids.contains(proposal_id) {
            return CommitDecision::Duplicate;
        }
        if ids.len() >= MAX_PROPOSALS {
            return CommitDecision::Rejected;
        }
        ids.insert(proposal_id.to_owned());
        self.allowed_commits.fetch_add(1, Ordering::SeqCst);
        CommitDecision::Allowed
    }

    /// An admission for the commit hook. Does not call the provider.
    pub fn issue_ticket(&self) -> Result<Admission, AgentError> {
        let ticket = self.gate.begin_request().map_err(AgentError::Gate)?;
        let _ = self.gate.finish_request(ticket);
        Ok(ticket)
    }

    fn preflight(&self) -> Result<(), AgentError> {
        let Some(id) = self.config.provider.id() else {
            return Err(AgentError::Provider(ProviderError::NoProvider));
        };
        if !self.policy.allows_provider(id) {
            return Err(AgentError::Provider(ProviderError::ProviderNotAllowed));
        }
        if let Some(endpoint) = self.config.provider.endpoint()
            && !self.policy.allows_endpoint(endpoint)
        {
            return Err(AgentError::Provider(ProviderError::EndpointNotAllowed));
        }
        Ok(())
    }

    fn load_credential(&self) -> Result<Option<Secret>, AgentError> {
        let Some(slot) = &self.config.credential_slot else {
            return Ok(None);
        };
        match self.credentials.get(slot) {
            Ok(Some(secret)) => Ok(Some(secret)),
            Ok(None) => Err(AgentError::Provider(ProviderError::MissingCredential)),
            Err(error) => Err(AgentError::Credential(error)),
        }
    }
}

fn valid_proposal_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BootPhase {
    FirstLaunch,
    OpenDocument,
    Idle,
}

impl AgentRuntime {
    pub fn phase(&self) -> Result<AiPhase, AgentError> {
        self.gate.phase().map_err(AgentError::Gate)
    }

    pub fn begin_stop(&self) -> Result<(), AgentError> {
        self.gate.begin_stop().map_err(AgentError::Gate)
    }

    /// Linearization point. Clears the user preference and bumps generation.
    pub fn commit_disabled(&mut self) -> Result<Generation, AgentError> {
        let generation = self.gate.commit_disabled().map_err(AgentError::Gate)?;
        self.config.user_enabled = false;
        Ok(generation)
    }

    pub fn kill_switch(&mut self) -> Result<Generation, AgentError> {
        let generation = self.gate.kill_switch().map_err(AgentError::Gate)?;
        self.config.user_enabled = false;
        Ok(generation)
    }
}
