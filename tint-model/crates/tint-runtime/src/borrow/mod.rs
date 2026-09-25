pub mod lock;
pub mod intrinsic;

pub struct BorrowManager {
    locked: Option<usize>,
}

impl BorrowManager {
    pub fn new() -> Self {
        Self { locked: None }
    }

    pub fn lock(&mut self, id: usize) -> bool {
        if self.locked.is_some() {
            return false;
        }
        self.locked = Some(id);
        true
    }

    pub fn unlock(&mut self) {
        self.locked = None;
    }
}
