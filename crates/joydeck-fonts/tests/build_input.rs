#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "Test assertions"
)]
//! Exercise the build-input parser without changing process environment variables.
#[allow(dead_code)]
#[path = "../build.rs"]
mod build_script;
