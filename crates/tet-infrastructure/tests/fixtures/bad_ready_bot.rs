//! Misbehaving TBP bot: sends `info` correctly, reads `rules`, then sends
//! `error` instead of `ready`.
//!
//! Used by `tests/subprocess_integration.rs` to verify the second handshake
//! check (after rules) catches the protocol error.

use std::io::{self, BufRead, Write};

use serde_json::json;

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut input = stdin.lock();
    let mut output = stdout.lock();

    // Step 1: send `info` correctly.
    writeln!(
        output,
        "{}",
        json!({
            "type": "info",
            "name": "bad_ready_bot",
            "version": "0.0.0",
            "author": "tet-rs tests",
            "features": [],
        })
    )
    .expect("write info");
    output.flush().expect("flush info");

    // Step 2: read `rules` from frontend.
    let mut line = String::new();
    input.read_line(&mut line).expect("read rules");

    // Step 3: send `error` instead of `ready`. Wrong!
    writeln!(
        output,
        "{}",
        json!({
            "type": "error",
            "reason": "unsupported_rules"
        })
    )
    .expect("write error");
    output.flush().expect("flush error");

    std::thread::sleep(std::time::Duration::from_millis(100));
}
