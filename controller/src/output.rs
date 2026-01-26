use crate::*;

impl Pin<'_, mode::Output> {
    async fn set_state(&mut self, state: PinState) {
        self.update_op(Op::Output { latch: state }).await;
    }

    async fn is_set_state(&mut self, state: PinState) -> bool {
        (match self
            .s
            .receiver()
            .unwrap()
            .changed_and(|request| request.state == RequestState::Done)
            .await
            .op
        {
            Op::Output { latch } => latch,
            _ => unreachable!(),
        }) == state
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
