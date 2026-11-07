use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use tet_application::BotSpeed;
use tet_infrastructure::bot::BotType;

/// Error returned when settings cannot be loaded or validated.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("failed to read settings file `{path}`: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to parse settings file `{path}`: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("invalid bot configuration in `{path}`: {source}")]
    Bot {
        path: PathBuf,
        #[source]
        source: BotConfigError,
    },
}

/// Error returned when one bot configuration entry is invalid.
#[derive(Debug, thiserror::Error)]
pub enum BotConfigError {
    #[error("unknown bot type `{bot_type}` at bots[{index}] (supported types: {supported})")]
    UnknownType {
        index: usize,
        bot_type: String,
        supported: String,
    },
    #[error("bot name at bots[{index}] must not be empty")]
    EmptyName { index: usize },
    #[error(
        "bot speed at bots[{index}] must be finite and greater than zero (got {pieces_per_second})"
    )]
    InvalidPps {
        index: usize,
        pieces_per_second: f32,
    },
}

/// Validated bot configuration used by the composition root.
#[derive(Clone, Debug, PartialEq)]
pub struct BotConfig {
    pub bot_type: BotType,
    pub name: String,
    pub speed: BotSpeed,
}

impl Default for BotConfig {
    fn default() -> Self {
        Self {
            bot_type: BotType::Blockfish,
            name: BotType::Blockfish.display_name().to_owned(),
            speed: BotSpeed::TurnBased,
        }
    }
}

/// Runtime settings loaded from `settings.toml`.
#[derive(Debug)]
pub struct Settings {
    pub controls: ControlSettings,
    pub timing: TimingSettings,
    pub display: DisplaySettings,
    pub bots: Vec<BotConfig>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            controls: ControlSettings::default(),
            timing: TimingSettings::default(),
            display: DisplaySettings::default(),
            bots: vec![BotConfig::default()],
        }
    }
}

impl Settings {
    /// Load and validate settings from `path`.
    ///
    /// A missing file uses defaults. A file that exists but cannot be read,
    /// parsed, or validated returns an error instead of silently selecting a
    /// different configuration.
    pub fn load(path: &Path) -> Result<Self, SettingsError> {
        let text = match fs_err::read_to_string(path) {
            Ok(text) => text,
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(source) => {
                return Err(SettingsError::Read {
                    path: path.to_path_buf(),
                    source,
                });
            }
        };

        let file =
            toml::from_str::<SettingsFile>(&text).map_err(|source| SettingsError::Parse {
                path: path.to_path_buf(),
                source,
            })?;

        Self::try_from(file).map_err(|source| SettingsError::Bot {
            path: path.to_path_buf(),
            source,
        })
    }
}

#[derive(Debug, Deserialize)]
struct SettingsFile {
    #[serde(default)]
    controls: ControlSettings,
    #[serde(default)]
    timing: TimingSettings,
    #[serde(default)]
    display: DisplaySettings,
    #[serde(default)]
    bots: Option<Vec<BotConfigFile>>,
}

#[derive(Debug, Deserialize)]
struct BotConfigFile {
    #[serde(rename = "type")]
    bot_type: String,
    name: Option<String>,
    speed: BotSpeedConfig,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(tag = "mode", rename_all = "snake_case")]
enum BotSpeedConfig {
    TurnBased,
    FixedPps { pieces_per_second: f32 },
}

impl TryFrom<SettingsFile> for Settings {
    type Error = BotConfigError;

    fn try_from(file: SettingsFile) -> Result<Self, Self::Error> {
        let bots = match file.bots {
            Some(configs) => resolve_bot_configs(configs)?,
            None => vec![BotConfig::default()],
        };

        Ok(Self {
            controls: file.controls,
            timing: file.timing,
            display: file.display,
            bots,
        })
    }
}

fn resolve_bot_configs(configs: Vec<BotConfigFile>) -> Result<Vec<BotConfig>, BotConfigError> {
    configs
        .into_iter()
        .enumerate()
        .map(|(index, config)| {
            let Some(bot_type) = BotType::parse(&config.bot_type) else {
                return Err(BotConfigError::UnknownType {
                    index,
                    bot_type: config.bot_type,
                    supported: BotType::ALL
                        .iter()
                        .map(|&kind| kind.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                });
            };

            let name = config
                .name
                .unwrap_or_else(|| bot_type.display_name().to_owned())
                .trim()
                .to_owned();
            if name.is_empty() {
                return Err(BotConfigError::EmptyName { index });
            }

            let speed = match config.speed {
                BotSpeedConfig::TurnBased => BotSpeed::TurnBased,
                BotSpeedConfig::FixedPps { pieces_per_second }
                    if pieces_per_second.is_finite() && pieces_per_second > 0.0 =>
                {
                    BotSpeed::FixedPps { pieces_per_second }
                }
                BotSpeedConfig::FixedPps { pieces_per_second } => {
                    return Err(BotConfigError::InvalidPps {
                        index,
                        pieces_per_second,
                    });
                }
            };

            Ok(BotConfig {
                bot_type,
                name,
                speed,
            })
        })
        .collect()
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct ControlSettings {
    pub move_left: String,
    pub move_right: String,
    pub soft_drop: String,
    pub hard_drop: String,
    pub rotate_cw: String,
    pub rotate_ccw: String,
    pub hold: String,
}

impl Default for ControlSettings {
    fn default() -> Self {
        Self {
            move_left: "Left".to_string(),
            move_right: "Right".to_string(),
            soft_drop: "Down".to_string(),
            hard_drop: "Space".to_string(),
            rotate_cw: "F".to_string(),
            rotate_ccw: "D".to_string(),
            hold: "S".to_string(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct TimingSettings {
    pub das: u16,
    pub arr: u16,
    pub sdf_interval: u16,
}

impl Default for TimingSettings {
    fn default() -> Self {
        Self {
            das: 267,
            arr: 17,
            sdf_interval: 67,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct DisplaySettings {
    pub grid_style: String,
    pub ghost_alpha: u8,
    pub skin: Option<String>,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            grid_style: "full".to_string(),
            ghost_alpha: 128,
            skin: Some("default".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<Settings, BotConfigError> {
        let file = toml::from_str::<SettingsFile>(text).expect("test TOML should parse");
        file.try_into()
    }

    #[test]
    fn missing_bot_list_uses_default_blockfish() {
        let settings = parse("").expect("default settings should be valid");
        assert_eq!(settings.bots.len(), 1);
        assert_eq!(settings.bots[0].bot_type, BotType::Blockfish);
        assert_eq!(settings.bots[0].name, "Blockfish");
        assert_eq!(settings.bots[0].speed, BotSpeed::TurnBased);
    }

    #[test]
    fn omitted_name_defaults_to_bot_type_display_name() {
        let settings = parse(
            r#"
[[bots]]
type = "blockfish"
speed = { mode = "turn_based" }
"#,
        )
        .expect("blockfish config should be valid");

        assert_eq!(settings.bots[0].name, "Blockfish");
    }

    #[test]
    fn custom_name_is_preserved() {
        let settings = parse(
            r#"
[[bots]]
type = "blockfish"
name = "Fast Fish"
speed = { mode = "fixed_pps", pieces_per_second = 2.0 }
"#,
        )
        .expect("custom blockfish config should be valid");

        assert_eq!(settings.bots[0].name, "Fast Fish");
        assert_eq!(
            settings.bots[0].speed,
            BotSpeed::FixedPps {
                pieces_per_second: 2.0
            }
        );
    }

    #[test]
    fn unknown_bot_type_is_rejected() {
        let error = parse(
            r#"
[[bots]]
type = "unknown"
speed = { mode = "turn_based" }
"#,
        )
        .expect_err("unknown bot type should fail");

        assert!(matches!(
            error,
            BotConfigError::UnknownType {
                index: 0,
                ref bot_type,
                ..
            } if bot_type == "unknown"
        ));
    }

    #[test]
    fn empty_name_is_rejected() {
        let error = parse(
            r#"
[[bots]]
type = "blockfish"
name = "   "
speed = { mode = "turn_based" }
"#,
        )
        .expect_err("empty bot name should fail");

        assert!(matches!(error, BotConfigError::EmptyName { index: 0 }));
    }

    #[test]
    fn duplicate_names_are_allowed() {
        let settings = parse(
            r#"
[[bots]]
type = "blockfish"
name = "Same"
speed = { mode = "turn_based" }

[[bots]]
type = "blockfish"
name = "Same"
speed = { mode = "turn_based" }
"#,
        )
        .expect("duplicate bot names should be valid");

        assert_eq!(settings.bots.len(), 2);
        assert_eq!(settings.bots[0].name, "Same");
        assert_eq!(settings.bots[1].name, "Same");
    }

    #[test]
    fn invalid_pps_values_are_rejected() {
        for value in ["0.0", "-1.0", "nan", "inf"] {
            let text = format!(
                r#"
[[bots]]
type = "blockfish"
speed = {{ mode = "fixed_pps", pieces_per_second = {value} }}
"#
            );
            let error = parse(&text).expect_err("invalid PPS should fail");
            assert!(matches!(error, BotConfigError::InvalidPps { index: 0, .. }));
        }
    }

    #[test]
    fn empty_roster_is_supported() {
        let settings = parse("bots = []").expect("empty roster should be valid");
        assert!(settings.bots.is_empty());
    }

    #[test]
    fn multiple_roster_entries_are_supported() {
        let settings = parse(
            r#"
[[bots]]
type = "blockfish"
name = "One"
speed = { mode = "turn_based" }

[[bots]]
type = "blockfish"
name = "Two"
speed = { mode = "fixed_pps", pieces_per_second = 0.5 }
"#,
        )
        .expect("multiple bots should be valid");

        assert_eq!(settings.bots.len(), 2);
        assert_eq!(settings.bots[0].name, "One");
        assert_eq!(settings.bots[1].name, "Two");
    }

    #[test]
    fn missing_file_uses_defaults() {
        let path = std::env::temp_dir().join(format!(
            "tet-rs-missing-settings-{}.toml",
            std::process::id()
        ));
        let _ = fs_err::remove_file(&path);

        let settings = Settings::load(&path).expect("missing settings should use defaults");
        assert_eq!(settings.bots.len(), 1);
        assert_eq!(settings.bots[0].name, "Blockfish");
    }

    #[test]
    fn invalid_file_returns_parse_error() {
        let path = std::env::temp_dir().join(format!(
            "tet-rs-invalid-settings-{}.toml",
            std::process::id()
        ));
        fs_err::write(&path, "bots = [").expect("test settings should be writable");

        let error = Settings::load(&path).expect_err("invalid settings should fail");
        assert!(matches!(error, SettingsError::Parse { .. }));

        fs_err::remove_file(&path).expect("test settings should be removable");
    }
}
