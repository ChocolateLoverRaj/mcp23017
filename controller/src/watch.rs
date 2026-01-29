use crate::*;

impl Pin<'_, mode::Watch> {
    /// Although this function is `async`, it is only `async` to access a mutex,
    /// so it basically be sync every time.
    pub async fn state(&mut self) -> PinState {
        match self.s.request.read().await.op {
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
        self.s.response_signal.wait().await;
    }
}
