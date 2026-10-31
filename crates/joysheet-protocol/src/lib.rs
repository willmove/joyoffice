//! Toolkit-free, bounded JSON control envelopes.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const MAX_LINE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_TEXT_BYTES: usize = 1024 * 1024;
pub const MAX_PENDING_COMMANDS: usize = 256;

pub fn token_path(port: u16) -> std::path::PathBuf {
    if let Some(path) = std::env::var_os("JOYSHEET_CONFIG_DIR").filter(|p| !p.is_empty()) {
        return std::path::PathBuf::from(path).join(format!("control-{port}.token"));
    }
    let base = std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("XDG_RUNTIME_DIR").map(std::path::PathBuf::from))
        .unwrap_or_else(|| std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir).join(".cache"));
    base.join("Joysheet").join(format!("control-{port}.token"))
}
pub fn client_token(addr: &str) -> Result<String, String> {
    for name in ["JOYSHEET_CONTROL_TOKEN", "GRIDCRAFT_CONTROL_TOKEN"] {
        if let Ok(token) = std::env::var(name)
            && !token.trim().is_empty()
        {
            return Ok(token.trim().into());
        }
    }
    let port = addr.rsplit(':').next().and_then(|s| s.parse::<u16>().ok()).ok_or("invalid control address")?;
    std::fs::read_to_string(token_path(port)).map(|s| s.trim().to_string()).map_err(|e| format!("start Joysheet with --control {port}: {e}"))
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ControlRequest {
    #[serde(default)]
    pub id: Value,
    pub method: String,
    #[serde(default = "empty_params")]
    pub params: Value,
}
fn empty_params() -> Value {
    json!({})
}
impl ControlRequest {
    pub fn parse(line: &str) -> Result<Self, String> {
        if line.len() > MAX_LINE_BYTES {
            return Err("request exceeds 8 MiB".into());
        }
        let request: Self = serde_json::from_str(line).map_err(|e| e.to_string())?;
        if request.method.is_empty() || request.method.len() > 256 {
            return Err("invalid method name".into());
        }
        if !request.params.is_object() {
            return Err("params must be a JSON object".into());
        }
        Ok(request)
    }
}
pub fn response(id: Value, result: Result<Value, String>) -> Value {
    match result {
        Ok(value) => json!({"id":id,"ok":true,"result":value}),
        Err(error) => json!({"id":id,"ok":false,"error":error}),
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CommandTarget {
    pub window: Option<u64>,
    pub doc: Option<u64>,
}
impl CommandTarget {
    pub fn from_params(params: &Value) -> Result<Self, String> {
        let read = |key: &str| match params.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(value) => value.as_u64().map(Some).ok_or_else(|| format!("{key} must be an unsigned integer")),
        };
        let target = Self { window: read("window")?, doc: read("doc")? };
        if target.window.is_some() && target.doc.is_some() {
            return Err("specify either window or doc".into());
        }
        Ok(target)
    }
}
#[cfg(test)]
#[path = "tests/lib_tests.rs"]
mod tests;
