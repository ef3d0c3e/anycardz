//! Shared by unit and integration tests.

use std::path::PathBuf;

pub fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data")
}
