use crate::GameTime;

/// Speed policy for a bot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BotSpeed {
    /// Commit one piece after each human piece lock.
    TurnBased,
    /// Commit pieces on a fixed wall-clock cadence.
    FixedPps { pieces_per_second: f32 },
}

/// Error returned when a speed policy cannot produce a valid schedule.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("pieces per second must be finite and greater than zero")]
pub struct InvalidBotSpeed;

/// Permit and deadline state for one bot.
///
/// Both speed modes use the same permit counter. Turn-based mode receives
/// permits from human piece locks; fixed-PPS mode receives them from its
/// deadline schedule.
#[derive(Clone, Debug)]
pub struct BotSchedule {
    speed: BotSpeed,
    permits: u32,
    fixed: Option<FixedSchedule>,
}

#[derive(Clone, Copy, Debug)]
struct FixedSchedule {
    period_ms: u64,
    next_deadline: GameTime,
}

impl BotSchedule {
    /// Create a schedule starting at `now`.
    ///
    /// Fixed-PPS schedules receive their first deadline immediately. The PPS
    /// value is converted to an integer-millisecond period once, here.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidBotSpeed`] when fixed PPS is not finite and positive.
    pub fn new(speed: BotSpeed, now: GameTime) -> Result<Self, InvalidBotSpeed> {
        let fixed = match speed {
            BotSpeed::TurnBased => None,
            BotSpeed::FixedPps { pieces_per_second } => {
                if !pieces_per_second.is_finite() || pieces_per_second <= 0.0 {
                    return Err(InvalidBotSpeed);
                }

                // The quotient is finite and positive. Rust's float-to-integer
                // cast saturates at `u64::MAX`, so very small PPS values still
                // produce a valid, positive period.
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let period_ms = ((1000.0_f64 / f64::from(pieces_per_second)).round() as u64).max(1);
                Some(FixedSchedule {
                    period_ms,
                    next_deadline: now,
                })
            }
        };

        Ok(Self {
            speed,
            permits: 0,
            fixed,
        })
    }

    /// Configured speed policy.
    #[must_use]
    pub fn speed(&self) -> BotSpeed {
        self.speed
    }

    /// Add one turn permit when the human locks a piece.
    ///
    /// Fixed-PPS bots ignore human locks because their deadline schedule is
    /// the permit source.
    pub fn on_human_piece_locked(&mut self) {
        if matches!(self.speed, BotSpeed::TurnBased) {
            self.permits = self.permits.saturating_add(1);
        }
    }

    /// Grant a fixed-speed permit when the next deadline is due.
    ///
    /// Missed deadlines collapse into at most one outstanding fixed-speed
    /// permit. The next deadline advances beyond `now`, preventing a backlog
    /// of immediate commits.
    ///
    /// Returns `true` when at least one full cadence interval was missed.
    pub fn refresh_fixed_deadline(&mut self, now: GameTime) -> bool {
        let Some(mut fixed) = self.fixed else {
            return false;
        };

        if now < fixed.next_deadline {
            return false;
        }

        let elapsed_ms = now
            .as_millis()
            .saturating_sub(fixed.next_deadline.as_millis());
        let missed = elapsed_ms >= fixed.period_ms;
        self.permits = 1;

        // Advance past every deadline due at or before `now`. At most one
        // permit represents all skipped slots.
        loop {
            let current_ms = fixed.next_deadline.as_millis();
            let next_ms = current_ms.saturating_add(fixed.period_ms);
            if next_ms <= current_ms {
                // The deadline is already at the end of the representable
                // time range and cannot advance further.
                break;
            }
            fixed.next_deadline = GameTime::from_millis(next_ms);
            if fixed.next_deadline > now {
                break;
            }
        }

        self.fixed = Some(fixed);
        missed
    }

    /// Whether a permit is currently available.
    #[must_use]
    pub fn has_permit(&self) -> bool {
        self.permits > 0
    }

    /// Consume one permit, returning whether one was available.
    #[must_use]
    pub fn consume_permit(&mut self) -> bool {
        if self.permits == 0 {
            return false;
        }

        self.permits -= 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use rstest::rstest;

    fn at(ms: u64) -> GameTime {
        GameTime::from_millis(ms)
    }

    #[test]
    fn exposes_configured_speed() {
        let schedule = BotSchedule::new(
            BotSpeed::FixedPps {
                pieces_per_second: 2.0,
            },
            at(0),
        )
        .expect("valid speed");
        assert_eq!(
            schedule.speed(),
            BotSpeed::FixedPps {
                pieces_per_second: 2.0
            }
        );
    }

    #[test]
    fn turn_based_starts_without_permit() {
        let schedule = BotSchedule::new(BotSpeed::TurnBased, at(0)).expect("valid speed");
        assert!(!schedule.has_permit());
    }

    #[test]
    fn human_lock_adds_turn_permit() {
        let mut schedule = BotSchedule::new(BotSpeed::TurnBased, at(0)).expect("valid speed");
        schedule.on_human_piece_locked();
        assert!(schedule.has_permit());
    }

    #[test]
    fn consuming_permit_removes_it() {
        let mut schedule = BotSchedule::new(BotSpeed::TurnBased, at(0)).expect("valid speed");
        schedule.on_human_piece_locked();

        assert!(schedule.consume_permit());
        assert!(!schedule.has_permit());
        assert!(!schedule.consume_permit());
    }

    #[test]
    fn multiple_human_locks_accumulate_permits() {
        let mut schedule = BotSchedule::new(BotSpeed::TurnBased, at(0)).expect("valid speed");
        schedule.on_human_piece_locked();
        schedule.on_human_piece_locked();
        schedule.on_human_piece_locked();

        assert!(schedule.consume_permit());
        assert!(schedule.consume_permit());
        assert!(schedule.consume_permit());
        assert!(!schedule.has_permit());
    }

    #[test]
    fn turn_permit_count_saturates() {
        let mut schedule = BotSchedule::new(BotSpeed::TurnBased, at(0)).expect("valid speed");
        schedule.permits = u32::MAX;
        schedule.on_human_piece_locked();
        assert_eq!(schedule.permits, u32::MAX);
    }

    #[test]
    fn human_lock_does_not_grant_fixed_permit() {
        let mut schedule = BotSchedule::new(
            BotSpeed::FixedPps {
                pieces_per_second: 2.0,
            },
            at(0),
        )
        .expect("valid speed");
        schedule.on_human_piece_locked();
        assert!(!schedule.has_permit());
    }

    #[test]
    fn first_fixed_deadline_is_due_immediately() {
        let mut schedule = BotSchedule::new(
            BotSpeed::FixedPps {
                pieces_per_second: 2.0,
            },
            at(0),
        )
        .expect("valid speed");

        schedule.refresh_fixed_deadline(at(0));
        assert!(schedule.has_permit());
    }

    #[test]
    fn fixed_permit_becomes_due_at_next_deadline() {
        let mut schedule = BotSchedule::new(
            BotSpeed::FixedPps {
                pieces_per_second: 2.0,
            },
            at(0),
        )
        .expect("valid speed");

        schedule.refresh_fixed_deadline(at(0));
        assert!(schedule.consume_permit());
        schedule.refresh_fixed_deadline(at(499));
        assert!(!schedule.has_permit());
        schedule.refresh_fixed_deadline(at(500));
        assert!(schedule.has_permit());
    }

    #[test]
    fn missed_fixed_deadlines_collapse_into_one_permit() {
        let mut schedule = BotSchedule::new(
            BotSpeed::FixedPps {
                pieces_per_second: 2.0,
            },
            at(0),
        )
        .expect("valid speed");

        schedule.refresh_fixed_deadline(at(0));
        schedule.refresh_fixed_deadline(at(2_000));
        assert!(schedule.consume_permit());
        assert!(!schedule.has_permit());
    }

    #[test]
    fn fixed_deadline_advances_past_missed_slots() {
        let mut schedule = BotSchedule::new(
            BotSpeed::FixedPps {
                pieces_per_second: 2.0,
            },
            at(0),
        )
        .expect("valid speed");

        schedule.refresh_fixed_deadline(at(0));
        assert!(schedule.consume_permit());
        schedule.refresh_fixed_deadline(at(750));
        assert!(schedule.has_permit());
        assert!(schedule.consume_permit());
        schedule.refresh_fixed_deadline(at(1_000));
        assert!(schedule.has_permit());
    }

    #[test]
    fn fixed_deadline_reports_when_full_period_is_missed() {
        let mut schedule = BotSchedule::new(
            BotSpeed::FixedPps {
                pieces_per_second: 2.0,
            },
            at(0),
        )
        .expect("valid speed");

        assert!(!schedule.refresh_fixed_deadline(at(0)));
        assert!(schedule.refresh_fixed_deadline(at(1_000)));
    }

    #[test]
    fn fixed_pps_is_converted_to_integer_millisecond_period() {
        let schedule = BotSchedule::new(
            BotSpeed::FixedPps {
                pieces_per_second: 3.0,
            },
            at(0),
        )
        .expect("valid speed");

        let fixed = schedule.fixed.expect("fixed schedule");
        assert_eq!(fixed.period_ms, 333);
    }

    #[rstest]
    #[case(0.0)]
    #[case(-1.0)]
    #[case(f32::NAN)]
    #[case(f32::INFINITY)]
    fn invalid_fixed_pps_is_rejected(#[case] pieces_per_second: f32) {
        let result = BotSchedule::new(BotSpeed::FixedPps { pieces_per_second }, at(0));
        assert!(matches!(result, Err(InvalidBotSpeed)));
    }
}
