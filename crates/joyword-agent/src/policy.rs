//! Administrator policy. OS backends are stubs: they do not read group
//! policy, configuration profiles, or system files. Tests supply
//! [`StaticPolicy`]. A forced-off policy wins over the user and over any
//! provider.

use std::collections::BTreeSet;
use std::fmt;

use crate::http::Endpoint;
use crate::provider::ProviderId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManagedReason {
    ForceDisabled,
    ProviderAllowlist,
    EndpointAllowlist,
}

/// Effective admin constraints. `managed` is what the settings surface shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdminPolicy {
    pub force_disabled: bool,
    pub managed: bool,
    /// `None` means the user may select any configured provider while AI is on.
    pub provider_allowlist: Option<BTreeSet<ProviderId>>,
    /// `None` means a configured endpoint is not further restricted.
    pub endpoint_allowlist: Option<BTreeSet<String>>,
}

impl AdminPolicy {
    pub fn unmanaged() -> Self {
        Self {
            force_disabled: false,
            managed: false,
            provider_allowlist: None,
            endpoint_allowlist: None,
        }
    }

    pub fn force_disabled() -> Self {
        Self {
            force_disabled: true,
            managed: true,
            provider_allowlist: None,
            endpoint_allowlist: None,
        }
    }

    pub fn allows_provider(&self, id: &ProviderId) -> bool {
        self.provider_allowlist
            .as_ref()
            .is_none_or(|allowed| allowed.contains(id))
    }

    pub fn allows_endpoint(&self, endpoint: &Endpoint) -> bool {
        self.endpoint_allowlist
            .as_ref()
            .is_none_or(|allowed| allowed.contains(endpoint.as_str()))
    }
}

pub trait PolicySource: Send + Sync {
    fn load(&self) -> AdminPolicy;
}

/// No administrator policy is installed.
#[derive(Clone, Copy, Debug, Default)]
pub struct UnmanagedPolicy;

impl PolicySource for UnmanagedPolicy {
    fn load(&self) -> AdminPolicy {
        AdminPolicy::unmanaged()
    }
}

/// Fixed policy for tests and for an embedding host that already resolved one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaticPolicy(pub AdminPolicy);

impl PolicySource for StaticPolicy {
    fn load(&self) -> AdminPolicy {
        self.0.clone()
    }
}

/// Platform policy backend. Every variant is a stub in T400.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OsPolicyBackend {
    WindowsGroupPolicy,
    MacOsConfigurationProfile,
    LinuxSystemConfig,
}

impl OsPolicyBackend {
    pub fn for_target() -> Self {
        if cfg!(target_os = "windows") {
            Self::WindowsGroupPolicy
        } else if cfg!(target_os = "macos") {
            Self::MacOsConfigurationProfile
        } else {
            Self::LinuxSystemConfig
        }
    }

    /// Stubs do not consult the operating system.
    pub const fn is_stub(self) -> bool {
        true
    }
}

impl PolicySource for OsPolicyBackend {
    fn load(&self) -> AdminPolicy {
        AdminPolicy::unmanaged()
    }
}

impl fmt::Display for OsPolicyBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::WindowsGroupPolicy => "windows-group-policy-stub",
            Self::MacOsConfigurationProfile => "macos-configuration-profile-stub",
            Self::LinuxSystemConfig => "linux-system-config-stub",
        })
    }
}

/// What a settings surface may show. Entry points stay visible only so the
/// managed state can be explained, and they are not usable while forced off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SettingsView {
    pub user_enabled: bool,
    pub effective_enabled: bool,
    pub managed: bool,
    pub force_disabled: bool,
    pub entry_available: bool,
    pub reason: Option<ManagedReason>,
}
