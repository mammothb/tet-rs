pub trait Clock {
    fn now_frame(&self) -> u64;
}
