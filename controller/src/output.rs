use crate::*;

impl Pin<'_, mode::Output> {
    async fn set_state(&mut self, state: PinState) {
        self.update_config(|config| {
            config.latch = state;
        })
        .await;
    }

    async fn is_set_state(&mut self, state: PinState) -> bool {
        self.s.config.try_get().unwrap().latch == state
    }
}

impl OutputPin for Pin<'_, mode::Output> {
    async fn set_low(&mut self) -> Result<(), Self::Error> {
        self.set_state(PinState::Low).await;
        Ok(())
    }

    async fn set_high(&mut self) -> Result<(), Self::Error> {
        self.set_state(PinState::High).await;
        todo!()
    }
}

impl StatefulOutputPin for Pin<'_, mode::Output> {
    async fn is_set_high(&mut self) -> Result<bool, Self::Error> {
        Ok(self.is_set_state(PinState::High).await)
    }

    async fn is_set_low(&mut self) -> Result<bool, Self::Error> {
        Ok(self.is_set_state(PinState::Low).await)
    }
}
