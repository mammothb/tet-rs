//! Minimal TBP bot for integration tests.
//!
//! Speaks just enough of the protocol to exercise `BotSubprocess`:
//! - On startup, sends `info`, reads `rules`, sends `ready`
//! - On `suggest`, replies with a single hardcoded move
//! - On `stop` / `quit`, exits cleanly
//! - Ignores other messages (`new_piece`, `play`)
//!
//! Used by `tests/subprocess_integration.rs`. Located at `tests/fixtures/` so
//! it doesn't pollute the main library. Registered as a `[[bin]]` target in
//! `Cargo.toml` so `env!("CARGO_BIN_EXE_echo_bot")` resolves to the built
//! binary in tests.

use std::io::{self, BufRead, Write};

use serde_json::{Value, json};

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut input = stdin.lock();
    let mut output = stdout.lock();

    // Step 1: send `info`. The `features` field is required by the TBP spec
    // even though it's an empty enum upstream; an empty array is correct.
    writeln!(
        output,
        "{}",
        json!({
            "type": "info",
            "name": "echo_bot",
            "version": "0.0.0",
            "author": "tet-rs tests",
            "features": [],
        })
    )
    .expect("write info");
    output.flush().expect("flush info");

    // Step 2: read `rules`.
    let mut line = String::new();
    input.read_line(&mut line).expect("read rules");

    // Step 3: send `ready`.
    writeln!(output, "{}", json!({"type": "ready"})).expect("write ready");
    output.flush().expect("flush ready");

    // Step 4+: read each subsequent message. Reply to `suggest`, exit on
    // `stop`/`quit`, ignore everything else.
    loop {
        line.clear();
        let n = input.read_line(&mut line).expect("read msg");
        if n == 0 {
            // EOF — frontend closed our stdin.
            break;
        }
        let msg: Value = match serde_json::from_str(line.trim()) {
            Ok(v) => v,
            Err(_) => continue, // tolerate malformed lines
        };
        match msg["type"].as_str().unwrap_or("") {
            "suggest" => {
                // Reply with a single dummy move: T-piece, north, x=4, y=19.
                writeln!(
                    output,
                    "{}",
                    json!({
                        "type": "suggestion",
                        "moves": [{
                            "location": {
                                "type": "T",
                                "orientation": "north",
                                "x": 4,
                                "y": 19
                            },
                            "spin": "none"
                        }]
                    })
                )
                .expect("write suggestion");
                output.flush().expect("flush suggestion");
            }
            "stop" | "quit" => break,
            _ => {} // ignore play / new_piece / start / etc.
        }
    }
}
