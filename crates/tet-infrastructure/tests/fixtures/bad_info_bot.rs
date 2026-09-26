//! Misbehaving TBP bot: sends `ready` first instead of `info`.
//!
//! Used by `tests/subprocess_integration.rs` to verify the handshake
//! catches the protocol error. Lives in `tests/fixtures/` so it doesn't
//! pollute the main library.

use std::io::{self, Write};

use serde_json::json;

fn main() {
    let stdout = io::stdout();
    let mut output = stdout.lock();

    // Send `ready` first — wrong! Should be `info`.
    writeln!(output, "{}", json!({"type": "ready"})).expect("write ready");
    output.flush().expect("flush ready");

    // Stay alive briefly so the parent process can read the bad message and
    // disconnect. exit code doesn't matter.
    std::thread::sleep(std::time::Duration::from_millis(100));
}
