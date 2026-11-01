use crate::{
    BasicMatrix, Color, Input,
    config::Config,
    place::PlaceFinder,
    shape::{ShapeTable, Transform, srs},
};

mod analysis;
mod b_star;
mod eval;
mod state;

pub use b_star::{Search, Step};
pub use state::State;

// Input / output types

/// A game state snapshot to begin an analysis from.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Snapshot {
    pub hold: Option<Color>,
    pub queue: Vec<Color>,
    pub matrix: BasicMatrix,
}

/// A suggested sequence and its rating.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Suggestion {
    /// List of inputs to perform the suggested move.
    pub inputs: Vec<Input>,
    // The "rating" is an abstract measurement for how good a move is (lower is better).
    pub rating: i64,
}

/// Statistics about the analysis after it has finished.
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct Stats {
    /// Number of iterations of the search algorithm.
    pub iterations: usize,
    /// Number of nodes generated.
    pub nodes: usize,
    /// Total time taken to do the analysis.
    pub time_taken: std::time::Duration,
}

// Evaluation function interface

pub use eval::Eval;

/// Performs the static analysis function on a snapshot.
pub fn static_eval(snapshot: &Snapshot) -> Eval {
    eval::eval(&snapshot.matrix)
}

// AI interface

// Re-export
pub use analysis::{Analysis, AnalysisDone, MoveId};

/// An instance of the Blockfish AI. Holds engine configuration and can be used to spawn
/// an analysis.
///
/// `AI` can currently be seen as just a sort of builder-pattern type for creating
/// `Analysis`'s.  However, in the future it could be extended to handle more things such
/// as a reusable thread pool and/or the place to call `static_eval`.
pub struct AI {
    config: Config,
    shape_table: std::sync::Arc<ShapeTable>,
    all_tx: Option<std::sync::mpsc::Sender<Suggestion>>,
}

impl AI {
    /// Constructs a new Blockfish AI instance with the given engine configuration.
    pub fn new(config: Config) -> Self {
        Self {
            config,
            shape_table: std::sync::Arc::new(srs()),
            all_tx: None,
        }
    }

    pub fn config(&self) -> Config {
        self.config.clone()
    }

    pub fn config_mut(&mut self) -> &mut Config {
        &mut self.config
    }

    /// Begins a new analysis of `snapshot`, returning a handle to it.
    pub fn analyze(&mut self, snapshot: Snapshot) -> Analysis {
        analysis::spawn(
            self.shape_table.clone(),
            self.config.clone(),
            snapshot.into(),
            self.all_tx.take(),
        )
    }

    /// Configures the next analysis (via `analyze()`) to send every suggestion it
    /// encounters to a non-blocking channel. Returns the rx end of that channel.
    ///
    /// NOTE: Even rejected suggestions are sent to this channel; this channel should just
    /// be used for diagnostic purposes.
    pub fn listen_all(&mut self) -> std::sync::mpsc::Receiver<Suggestion> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.all_tx = Some(tx);
        rx
    }
}

pub fn analyze(
    shape_table: &ShapeTable,
    config: &Config,
    root: State,
) -> Option<(i64, Vec<usize>)> {
    let mut search = Search::new(shape_table, config.parameters.clone());
    search.start(root);

    let mut result = None;
    while search.node_count() < config.search_limit {
        match search.step() {
            Ok(Step::RatingChanged { trace, rating, .. }) => {
                if result
                    .as_ref()
                    .is_none_or(|&(best_rating, _)| rating < best_rating)
                {
                    result = Some((rating, trace));
                }
            }
            Ok(Step::SequenceRejected { .. }) | Ok(Step::Other) => {}
            Err(_) => break,
        }
    }
    result
}

pub fn reconstruct_first_placement(
    shape_table: &ShapeTable,
    state: State,
    trace: &[usize],
) -> Option<(Color, Transform, bool)> {
    let first = *trace.first()?;
    let mut pfind = PlaceFinder::new(shape_table);
    let placement = state.placements(&mut pfind).find(|p| p.idx == first)?;
    Some((placement.shape.color(), placement.tf, placement.did_hold))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn analyze_empty_board_finds_move() {
        let shape_table = srs();
        let config = Config::default();
        let snapshot = Snapshot {
            hold: None,
            queue: "JLSZTIO".chars().map(Color::n).collect(),
            matrix: BasicMatrix::with_cols(10),
        };

        let result = analyze(&shape_table, &config, snapshot.clone().into());
        assert!(
            result.is_some(),
            "should find at least one move on an empty board"
        );
        let (_, trace) = result.unwrap();
        assert!(!trace.is_empty(), "trace must not be empty");

        let placement = reconstruct_first_placement(&shape_table, State::from(snapshot), &trace);
        assert!(placement.is_some(), "must be able to reconstruct placement");
    }
}
