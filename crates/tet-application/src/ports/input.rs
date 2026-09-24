pub enum Input {
    None,
    MoveLeft,
    MoveRight,
    RotateCW,
    RotateCCW,
    SoftDrop,
    HardDrop,
    Hold,
}

pub trait InputSource {
    /// Poll current held/pressed keys and produce one Input per frame.
    /// Adapter must clear edge-triggered state between calls.
    fn poll(&mut self) -> Input;
}
