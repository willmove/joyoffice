// Test-only fixture builders intentionally assert valid test input.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod comment_ids;
mod date_fields;
mod macro_packages;
#[cfg(not(target_arch = "wasm32"))]
mod odt_sections;
mod recovery_format;
#[cfg(not(target_arch = "wasm32"))]
mod rtf_sections;
#[cfg(not(target_arch = "wasm32"))]
mod safe_save;
mod three_column_headers;
