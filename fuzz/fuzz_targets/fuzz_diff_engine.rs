#![no_main]

use clawdius_core::diff::FileDiff;
use libfuzzer_sys::fuzz_target;
use std::path::PathBuf;

fuzz_target!(|(old, new): (String, String)| {
    // Diff should handle any input without panicking
    let _ = FileDiff::compute(PathBuf::from("test"), Some(&old), &new);
});
