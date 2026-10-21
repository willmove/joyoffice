//! User AI configuration. The default is off, telemetry is off, and secret
//! material is refused instead of stored in the unknown-key bag.

use std::fmt;

use crate::credential::valid_slot;
use crate::http::Endpoint;
use crate::provider::ProviderId;

pub const SCHEMA: u32 = 1;
const MAX_TEXT: usize = 64 * 1024;
const MAX_UNKNOWN: usize = 64;

const FORBIDDEN_KEY_PARTS: &[&str] = &[
    "secret",
    "api_key",
    "apikey",
    "password",
    "token",
    "authorization",
];

/// Which provider a user has selected. `None` is the default and names no endpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderSelection {
    None,
    Named {
        id: ProviderId,
        endpoint: Option<Endpoint>,
    },
}

impl ProviderSelection {
    pub fn named(id: &str) -> Result<Self, ConfigError> {
        Ok(Self::Named {
            id: ProviderId::parse(id)?,
            endpoint: None,
        })
    }

    pub fn http(id: &str, endpoint: &str) -> Result<Self, ConfigError> {
        Ok(Self::Named {
            id: ProviderId::parse(id)?,
            endpoint: Some(Endpoint::parse(endpoint)?),
        })
    }

    pub fn id(&self) -> Option<&ProviderId> {
        match self {
            Self::None => None,
            Self::Named { id, .. } => Some(id),
        }
    }

    pub fn endpoint(&self) -> Option<&Endpoint> {
        match self {
            Self::None => None,
            Self::Named { endpoint, .. } => endpoint.as_ref(),
        }
    }
}

/// Persisted AI settings. Construct with [`AiConfig::default`] for first launch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiConfig {
    pub schema: u32,
    /// User preference. Ineffective while an admin policy forces AI off.
    pub user_enabled: bool,
    pub provider: ProviderSelection,
    /// Slot name only. The secret itself is not a field of this struct.
    pub credential_slot: Option<String>,
    /// QUALITY-007: default false. No uploader is compiled in either state.
    pub telemetry_enabled: bool,
    unknown: Vec<(String, String)>,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            schema: SCHEMA,
            user_enabled: false,
            provider: ProviderSelection::None,
            credential_slot: None,
            telemetry_enabled: false,
            unknown: Vec::new(),
        }
    }
}

impl AiConfig {
    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        if text.len() > MAX_TEXT {
            return Err(ConfigError::TooLarge);
        }
        let mut entries = Vec::new();
        for line in text.lines() {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, value) = line.split_once('=').ok_or(ConfigError::Syntax)?;
            let key = key.trim();
            if key.is_empty() || key.len() > 64 || value.len() > 2048 {
                return Err(ConfigError::Syntax);
            }
            if forbidden_key(key) {
                return Err(ConfigError::SecretMaterial);
            }
            entries.push((key.to_owned(), value.to_owned()));
        }
        let schema = match entries.iter().find(|(key, _)| key == "schema") {
            Some((_, value)) => value.parse::<u32>().map_err(|_| ConfigError::Syntax)?,
            None => 0,
        };
        if schema > SCHEMA {
            return Err(ConfigError::NewerSchema);
        }
        let mut enabled = None;
        let mut provider = None;
        let mut endpoint = None;
        let mut slot = None;
        let mut telemetry = None;
        let mut unknown = Vec::new();
        for (key, value) in entries {
            match key.as_str() {
                "schema" => {}
                "ai.enabled" => assign(&mut enabled, parse_bool(&value))?,
                "ai.provider" => assign(&mut provider, Some(value))?,
                "ai.endpoint" => assign(&mut endpoint, Some(value))?,
                "ai.credential_slot" => assign(&mut slot, Some(value))?,
                "telemetry.enabled" => assign(&mut telemetry, parse_bool(&value))?,
                _ => {
                    if unknown.len() >= MAX_UNKNOWN {
                        return Err(ConfigError::TooLarge);
                    }
                    unknown.push((key, value));
                }
            }
        }
        let mut config = AiConfig {
            schema: SCHEMA,
            ..AiConfig::default()
        };
        if let Some(flag) = enabled {
            config.user_enabled = flag;
        }
        if let Some(flag) = telemetry {
            config.telemetry_enabled = flag;
        }
        let provider_name = provider.unwrap_or_else(|| "none".to_owned());
        if provider_name == "none" {
            if let Some(value) = endpoint.as_deref()
                && !value.is_empty()
            {
                return Err(ConfigError::InvalidValue);
            }
            config.provider = ProviderSelection::None;
        } else {
            let id = ProviderId::parse(&provider_name)?;
            let endpoint = match endpoint.as_deref() {
                None | Some("") => None,
                Some(value) => Some(Endpoint::parse(value)?),
            };
            config.provider = ProviderSelection::Named { id, endpoint };
        }
        config.credential_slot = match slot.as_deref() {
            None | Some("") => None,
            Some(value) if valid_slot(value) => Some(value.to_owned()),
            Some(_) => return Err(ConfigError::InvalidValue),
        };
        config.unknown = unknown;
        Ok(config)
    }

    pub fn serialize(&self) -> String {
        let mut out = String::from("# JoyWord AI settings. Secrets are not stored here.\n");
        out.push_str(&format!("schema={SCHEMA}\n"));
        out.push_str(&format!("ai.enabled={}\n", self.user_enabled));
        match &self.provider {
            ProviderSelection::None => {
                out.push_str("ai.provider=none\n");
                out.push_str("ai.endpoint=\n");
            }
            ProviderSelection::Named { id, endpoint } => {
                out.push_str(&format!("ai.provider={id}\n"));
                match endpoint {
                    Some(endpoint) => out.push_str(&format!("ai.endpoint={endpoint}\n")),
                    None => out.push_str("ai.endpoint=\n"),
                }
            }
        }
        match &self.credential_slot {
            Some(slot) => out.push_str(&format!("ai.credential_slot={slot}\n")),
            None => out.push_str("ai.credential_slot=\n"),
        }
        out.push_str(&format!("telemetry.enabled={}\n", self.telemetry_enabled));
        for (key, value) in &self.unknown {
            if forbidden_key(key) {
                continue;
            }
            out.push_str(key);
            out.push('=');
            out.push_str(value);
            out.push('\n');
        }
        out
    }
}

fn assign<T>(slot: &mut Option<T>, value: Option<T>) -> Result<(), ConfigError> {
    if slot.is_some() {
        return Err(ConfigError::Syntax);
    }
    *slot = Some(value.ok_or(ConfigError::InvalidValue)?);
    Ok(())
}

fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn forbidden_key(key: &str) -> bool {
    let folded = key.to_ascii_lowercase();
    FORBIDDEN_KEY_PARTS.iter().any(|part| folded.contains(part))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigError {
    Syntax,
    InvalidValue,
    TooLarge,
    NewerSchema,
    /// A key looked like secret material. The value is not retained or echoed.
    SecretMaterial,
    Provider(crate::provider::IdError),
    Endpoint(crate::http::EndpointError),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ConfigError::Syntax => "ai config syntax is invalid",
            ConfigError::InvalidValue => "ai config value is invalid",
            ConfigError::TooLarge => "ai config exceeds its budget",
            ConfigError::NewerSchema => "ai config schema is newer than this build",
            ConfigError::SecretMaterial => "ai config must not contain secret material",
            ConfigError::Provider(_) => "ai provider id is invalid",
            ConfigError::Endpoint(_) => "ai endpoint is invalid",
        })
    }
}

impl std::error::Error for ConfigError {}

impl From<crate::provider::IdError> for ConfigError {
    fn from(error: crate::provider::IdError) -> Self {
        ConfigError::Provider(error)
    }
}

impl From<crate::http::EndpointError> for ConfigError {
    fn from(error: crate::http::EndpointError) -> Self {
        ConfigError::Endpoint(error)
    }
}
