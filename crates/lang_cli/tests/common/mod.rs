//! Shared test helpers.

use std::process::{Child, Output};

/// A child process that is killed if the test ends (or fails) without
/// waiting for it, so a failing test leaves no server running.
pub struct Reaped(Option<Child>);

impl Reaped {
    pub fn new(c: Child) -> Reaped {
        Reaped(Some(c))
    }

    /// Wait for its exit and collect its output (it is then reaped).
    #[allow(dead_code)]
    pub fn output(mut self) -> Output {
        self.0.take().unwrap().wait_with_output().unwrap()
    }
}

impl std::ops::Deref for Reaped {
    type Target = Child;
    fn deref(&self) -> &Child {
        self.0.as_ref().unwrap()
    }
}

impl std::ops::DerefMut for Reaped {
    fn deref_mut(&mut self) -> &mut Child {
        self.0.as_mut().unwrap()
    }
}

impl Drop for Reaped {
    fn drop(&mut self) {
        if let Some(c) = &mut self.0
            && c.try_wait().ok().flatten().is_none()
        {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}
