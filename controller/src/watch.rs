use crate::*;

impl Pin<'_, mode::Watch> {
    pub fn watched_value(&self) -> PinState {
        match self.s.try_get().unwrap().op {
            Op::Watch {
                pull_up_enabled: _,
                last_known_value,
            } => last_known_value,
            _ => unreachable!(),
        }
        .unwrap()
    }

    /// Wait until the watched value changes.
    /// After this, call [`Self::watched_value`].
    /// It's possible that the watched value is the same as before even after this function returns.
    pub async fn watch(&mut self) {
        self.s.receiver().unwrap().changed().await;
    }
}
