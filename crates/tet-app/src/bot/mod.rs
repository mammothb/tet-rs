mod runtime;
mod worker;

use tet_application::{GameSession, GameTime, Player};
use tet_infrastructure::{SmallRng, bot::create_bot};

use crate::settings::BotConfig;

pub(crate) use runtime::BotRuntime;
pub(crate) use worker::spawn_bot_worker;

/// Create one worker, player, and runtime for each configured bot.
pub(crate) fn create_runtimes(
    runtime: &tokio::runtime::Handle,
    session: &mut GameSession,
    configs: &[BotConfig],
) -> Vec<BotRuntime> {
    let mut bot_runtimes = Vec::with_capacity(configs.len());

    for bot_config in configs {
        let bot = create_bot(bot_config.bot_type);
        let handle = spawn_bot_worker(runtime, bot);
        let (_, queue_rng) = SmallRng::new(None);
        let (_, attack_rng) = SmallRng::new(None);
        let player_idx =
            session.add_player(Player::new_bot(Box::new(queue_rng), Box::new(attack_rng)));
        let bot_runtime = BotRuntime::new(
            player_idx,
            bot_config.bot_type,
            bot_config.name.clone(),
            bot_config.speed,
            handle,
            GameTime::ZERO,
        )
        .expect("validated bot speed should produce a schedule");
        bot_runtimes.push(bot_runtime);
    }

    bot_runtimes
}

/// Refresh fixed-speed deadlines and report missed cadence intervals.
pub(crate) fn refresh_schedules(runtimes: &mut [BotRuntime], now: GameTime) {
    for runtime in runtimes {
        if runtime.refresh_schedule(now) {
            let Some(configured_pps) = runtime.configured_pps() else {
                continue;
            };
            tracing::warn!(
                target: "bot",
                player = runtime.player_idx(),
                bot = %runtime.name(),
                bot_type = runtime.bot_type().as_str(),
                configured_pps,
                "bot missed fixed-speed deadline",
            );
        }
    }
}

/// Build display labels indexed by player index.
pub(crate) fn player_labels(
    player_count: usize,
    human_idx: usize,
    bot_runtimes: &[BotRuntime],
) -> Vec<String> {
    let mut labels = vec![String::new(); player_count];
    if let Some(label) = labels.get_mut(human_idx) {
        label.push_str("human");
    }
    for runtime in bot_runtimes {
        if let Some(label) = labels.get_mut(runtime.player_idx()) {
            label.push_str(runtime.name());
        }
    }
    labels
}

#[cfg(test)]
mod tests {
    use super::*;

    use tet_application::BotSpeed;
    use tet_domain::Ruleset;
    use tet_infrastructure::bot::BotType;

    #[tokio::test]
    async fn creates_one_runtime_and_player_per_configured_bot() {
        let configs = vec![
            BotConfig {
                bot_type: BotType::Blockfish,
                name: "One".to_owned(),
                speed: BotSpeed::TurnBased,
            },
            BotConfig {
                bot_type: BotType::Blockfish,
                name: "Two".to_owned(),
                speed: BotSpeed::TurnBased,
            },
        ];
        let mut session = GameSession::new(Ruleset::guideline());

        let runtimes = create_runtimes(&tokio::runtime::Handle::current(), &mut session, &configs);

        assert_eq!(runtimes.len(), 2);
        assert_eq!(session.players.len(), 2);
        assert_eq!(runtimes[0].player_idx(), 0);
        assert_eq!(runtimes[1].player_idx(), 1);
    }

    #[tokio::test]
    async fn creates_no_bot_runtimes_for_empty_roster() {
        let mut session = GameSession::new(Ruleset::guideline());

        let runtimes = create_runtimes(&tokio::runtime::Handle::current(), &mut session, &[]);

        assert!(runtimes.is_empty());
        assert!(session.players.is_empty());
    }

    #[tokio::test]
    async fn labels_use_human_and_configured_bot_names() {
        let configs = vec![
            BotConfig::default(),
            BotConfig {
                bot_type: BotType::Blockfish,
                name: "Fast Fish".to_owned(),
                speed: BotSpeed::TurnBased,
            },
        ];
        let mut session = GameSession::new(Ruleset::guideline());
        let (_, queue_rng) = SmallRng::new(None);
        let (_, attack_rng) = SmallRng::new(None);
        session.add_player(Player::new_human(Box::new(queue_rng), Box::new(attack_rng)));
        let runtimes = create_runtimes(&tokio::runtime::Handle::current(), &mut session, &configs);

        let labels = player_labels(session.players.len(), 0, &runtimes);

        assert_eq!(labels, vec!["human", "Blockfish", "Fast Fish"]);
    }
}
