use std::path::Path;
use std::process::Stdio;

use async_trait::async_trait;
use tet_application::ports::bot::BotMove;
use tet_application::{BotError, BotTransport, PlayerSnapshot};
use tet_domain::Ruleset;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

use crate::codec::{BotMessage, FrontendMessage, Rules};
use crate::mapper;

pub struct BotSubprocess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    /// Bot metadata collected from the `info` message.
    pub info: BotInfo,
}

// Manual Debug because the child/stdio fields don't implement Debug. Showing
// just `info` is enough for test failure messages. `finish_non_exhaustive()`
// signals that some fields are intentionally omitted (so clippy's
// `missing_fields_in_debug` lint doesn't complain).
impl std::fmt::Debug for BotSubprocess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BotSubprocess")
            .field("info", &self.info)
            .finish_non_exhaustive()
    }
}

#[derive(Default, Debug, PartialEq, Eq)]
pub struct BotInfo {
    pub name: String,
    pub version: String,
    pub author: String,
}

impl BotSubprocess {
    /// Spawn the bot binary, exchange `info` + `rules` + `ready`, and return
    /// a ready-to-use handle.
    ///
    /// # Errors
    ///
    /// - `BotError::Io`: spawn failed (binary not found, permission denied)
    /// - `BotError::Protocol`: bot didn't greet with `info` / didn't reply `ready`
    /// - `BotError::Exited`: bot process died during handshake
    pub async fn spawn(bot_path: &Path) -> Result<Self, BotError> {
        let mut child = Command::new(bot_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| BotError::Io)?;
        let stdin = child.stdin.take().ok_or(BotError::Io)?;
        let stdout = child.stdout.take().ok_or(BotError::Io)?;
        let stderr = child.stderr.take();
        if let Some(stderr) = stderr {
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    tracing::warn!(target: "bot_stderr", "{}", line);
                }
            });
        }

        let mut bot = Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            info: BotInfo::default(),
        };

        let msg = bot.recv().await?;
        let info = match msg {
            // `BotMessage::Info` is a tuple variant wrapping the `Info` struct.
            // Destructure the tuple to get the struct, then take the fields.
            BotMessage::Info(info) => BotInfo {
                name: info.name,
                version: info.version,
                author: info.author,
            },
            other => {
                let msg = format!("expected Info message, got {other:?}");
                tracing::warn!("{}", msg);
                return Err(BotError::Protocol(msg));
            }
        };

        bot.send(&FrontendMessage::Rules(Rules::default())).await?;

        let msg = bot.recv().await?;
        if !matches!(msg, BotMessage::Ready(_)) {
            let detail = format!("expected Ready message, got {msg:?}");
            tracing::warn!("{}", detail);
            return Err(BotError::Protocol(detail));
        }

        bot.info = info;
        Ok(bot)
    }

    /// Send one `FrontendMessage` over stdin (line-delimited JSON).
    async fn send(&mut self, msg: &FrontendMessage) -> Result<(), BotError> {
        use tokio::io::ErrorKind;

        let mut json = serde_json::to_string(msg)
            .map_err(|e| BotError::Protocol(format!("serialize FrontendMessage: {e}")))?;
        json.push('\n'); // TBP wire format is one JSON message per line

        // SIGKILL of the bot can close stdin/stdout before our write lands.
        // BrokenPipe at this point means the bot is gone — treat as Exited.
        if let Err(e) = self.stdin.write_all(json.as_bytes()).await {
            return Err(if e.kind() == ErrorKind::BrokenPipe {
                BotError::Exited
            } else {
                BotError::Io
            });
        }
        if let Err(e) = self.stdin.flush().await {
            return Err(if e.kind() == ErrorKind::BrokenPipe {
                BotError::Exited
            } else {
                BotError::Io
            });
        }
        Ok(())
    }

    /// Read one `BotMessage` from stdout (blocking until newline).
    async fn recv(&mut self) -> Result<BotMessage, BotError> {
        use tokio::io::ErrorKind;

        let mut line = String::new();
        let n = match self.stdout.read_line(&mut line).await {
            Ok(n) => n,
            // SIGKILL of the bot can produce a broken-pipe error rather than
            // clean EOF, depending on timing. Treat it as Exited too.
            Err(e) if e.kind() == ErrorKind::BrokenPipe => return Err(BotError::Exited),
            Err(_) => return Err(BotError::Io),
        };
        if n == 0 {
            // EOF — bot closed stdout, probably exited.
            return Err(BotError::Exited);
        }
        let trimmed = line.trim_end_matches('\n').trim_end_matches('\r');
        serde_json::from_str(trimmed)
            .map_err(|e| BotError::Protocol(format!("deserialize BotMessage: {e}")))
    }
}

#[async_trait]
impl BotTransport for BotSubprocess {
    fn start(&mut self, _rules: &Ruleset) -> Result<(), BotError> {
        // `rules` was already sent during `spawn` handshake.
        // Nothing to do here — `start` is the equivalent of "we're ready
        // for the next game", but `BotSubprocess` only supports one game
        // per process (the protocol allows re-start but we don't need it
        // for first slice).
        Ok(())
    }

    async fn update(&mut self, snapshot: &PlayerSnapshot) -> Result<(), BotError> {
        let start = mapper::snapshot_to_start(snapshot);
        self.send(&FrontendMessage::Start(start)).await?;
        // For per-frame updates, the protocol uses `new_piece` events
        // between suggests, not full `start` re-sends. `update()` here
        // sends `start` for game-start; mid-game per-piece updates flow
        // through `suggest`/`play` flow.
        Ok(())
    }

    async fn suggest(&mut self) -> Result<Vec<BotMove>, BotError> {
        self.send(&FrontendMessage::Suggest(crate::codec::Suggest::default()))
            .await?;
        match self.recv().await? {
            BotMessage::Suggestion(s) => s.moves.into_iter().map(mapper::tbp_move_to_bot).collect(),
            BotMessage::Error(e) => Err(mapper::tbp_error_to_bot(&e)),
            other => {
                let detail = format!("expected Suggestion message, got {other:?}");
                tracing::warn!("{}", detail);
                Err(BotError::Protocol(detail))
            }
        }
    }

    async fn stop(&mut self) {
        let _ = self.child.kill().await;
        let _ = self.child.wait().await; // reap zombie
    }
}

impl Drop for BotSubprocess {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}
