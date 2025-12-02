// runtime/state/signal.rs

#[derive(Debug)]
pub struct SignalCell<T: Clone> {
    value: T,
}

impl<T: Clone> SignalCell<T> {
    pub fn new(value: T) -> Self {
        Self { value }
    }

    pub fn get(&self) -> T {
        self.value.clone()
    }

    pub fn set(&mut self, v: T) {
        self.value = v;
    }
}
