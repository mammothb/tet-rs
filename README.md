# tet-rs

A Rust implementation of standard Tetris with configurable in-process bots.

## Run

```bash
cargo run --release
```

The game reads controls, timing, display, and bot configuration from `settings.toml`.

## Bot configuration

Bots are configured as a list of `[[bots]]` tables. Each entry creates one bot-controlled player.

```toml
[[bots]]
type = "blockfish"
name = "Blockfish" # optional; defaults to the bot type's display name
speed = { mode = "turn_based" }
```

### Bot type

`type` selects a bot implementation compiled into the game. The currently supported type is:

- `blockfish` — Blockfish placement engine.

Bot implementations run in-process; no external bot executable is required.

### Bot name

`name` is the label shown above the bot's board. It is optional and defaults to the canonical display name of `type` (`blockfish` defaults to `Blockfish`). Names are display labels rather than identity, so duplicate names are allowed.

### Bot speed

#### Turn-based

```toml
speed = { mode = "turn_based" }
```

A turn-based bot calculates its next move ahead of time but only places it after the human places a piece.

#### Fixed pieces per second

```toml
speed = { mode = "fixed_pps", pieces_per_second = 2.0 }
```

A fixed-speed bot places pieces on the configured wall-clock cadence, independently of human turns. Its effective rate can be lower when calculation takes longer than the configured interval.

### Multiple bots and no bots

Multiple bot entries create multiple independent bot boards:

```toml
[[bots]]
type = "blockfish"
name = "Opponent"
speed = { mode = "turn_based" }

[[bots]]
type = "blockfish"
name = "Clock"
speed = { mode = "fixed_pps", pieces_per_second = 0.5 }
```

To run without bots:

```toml
bots = []
```

## Test

```bash
cargo test --workspace
```

Run the long Blockfish regression scenario with:

```bash
cargo test --release -p tet-infrastructure --test blockfish_bot -- --ignored -- --nocapture
```
