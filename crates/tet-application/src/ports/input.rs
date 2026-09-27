#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    None,
    MoveLeft,
    MoveRight,
    RotateCW,
    RotateCCW,
    SoftDrop,
    HardDrop,
    Hold,
    /// Emitted by the composition root's timer when `GRAVITY_MS` has elapsed
    /// since the last gravity tick. Routes to `tick::step_gravity`. Not
    /// produced by keyboard polling — humans get gravity via the timer too.
    StepGravity,
}

pub trait InputSource {
    /// Poll current held/pressed keys and produce one Input per frame.
    /// Adapter must clear edge-triggered state between calls.
    fn poll(&mut self) -> Input;
}
