use crate::*;

impl Pin<'_, mode::Watch> {
    pub fn watched_value(&self) -> Option<PinState> {
        self.s.watched_state.try_get()
    }

    /// Wait until the watched value changes.
    /// After this, call [`Self::watched_value`].
    /// It's possible that the watched value is the same as before even after this function returns.
    pub async fn watch(&mut self) {
        self.s.watched_state.receiver().unwrap().changed().await;
    }
}
