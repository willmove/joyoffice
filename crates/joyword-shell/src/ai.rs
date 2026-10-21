//! Optional AI toggle (T400).
//!
//! Compiled only with `--features ai`. The default shell build does not
//! include this module and does not link `joyword-agent`. The toggle keeps
//! the agent runtime at its default-off state and does not register a ribbon
//! command: the settings entry is this hook, and document commands stay on
//! the existing registry.

use joyword_agent::{AdminPolicy, AgentError, AgentRuntime, SettingsView};

/// In-memory AI switch for a shell that opted into the `ai` feature.
pub struct AiToggle {
    runtime: AgentRuntime,
}

impl Default for AiToggle {
    fn default() -> Self {
        Self::new()
    }
}

impl AiToggle {
    /// First launch: AI disabled, no provider request.
    pub fn new() -> Self {
        Self {
            runtime: AgentRuntime::boot_disabled(),
        }
    }

    /// Runs the launch / open / idle hooks and returns how many provider
    /// calls they made.
    pub fn startup_provider_requests(&self) -> u64 {
        self.runtime.startup_sequence().provider_calls
    }

    pub fn settings(&self) -> Result<SettingsView, AgentError> {
        self.runtime.settings()
    }

    pub fn set_user_enabled(&mut self, enabled: bool) -> Result<(), AgentError> {
        self.runtime.set_user_enabled(enabled)
    }

    pub fn apply_admin_policy(&mut self, policy: AdminPolicy) -> Result<(), AgentError> {
        self.runtime.apply_policy(policy)
    }

    pub fn kill_switch(&mut self) -> Result<(), AgentError> {
        self.runtime.kill_switch().map(|_| ())
    }
}
