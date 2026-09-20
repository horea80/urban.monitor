#![allow(dead_code)]

use std::path::PathBuf;

pub fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join(name)
}

pub fn fixture(name: &str) -> String {
    std::fs::read_to_string(fixture_path(name)).unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

pub fn fixture_bytes(name: &str) -> Vec<u8> {
    std::fs::read(fixture_path(name)).unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

pub const LISTING_URL: &str = "https://primariaclujnapoca.ro/strategii-urbane/comisia-tehnica-de-amenajare-a-teritoriului-si-urbanism/sedinte-comisie/";
pub const MEETING_0916: &str = "https://primariaclujnapoca.ro/urbanism/sedinte-comisie/sedinta-din-16-septembrie-2026/";
pub const MEETING_0114: &str = "https://primariaclujnapoca.ro/urbanism/sedinte-comisie/sedinta-din-14-ianuarie-2026/";
pub const MEETING_0527: &str = "https://primariaclujnapoca.ro/urbanism/sedinte-comisie/sedinta-din-27-mai-2026/";
