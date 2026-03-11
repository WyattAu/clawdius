#![no_main]

use clawdius_core::rpc::{Request, Response};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = Request::from_json(s);
        let _ = Response::from_json(s);
    }
});
