// borrow/lock.rs

#[derive(Debug)]
pub struct BorrowLock {
    locked: Option<usize>,
}

impl BorrowLock {
    pub fn new() -> Self {
        Self { locked: None }
    }

    pub fn try_lock(&mut self, id: usize) -> Result<(), ()> {
        match self.locked {
            None => {
                self.locked = Some(id);
                Ok(())
            }
            Some(_) => Err(()),
        }
    }

    pub fn unlock(&mut self, id: usize) {
        if self.locked == Some(id) {
            self.locked = None;
        }
    }

    pub fn is_locked(&self) -> bool {
        self.locked.is_some()
    }
}
