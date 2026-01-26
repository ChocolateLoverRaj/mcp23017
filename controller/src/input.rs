use crate::*;

impl Pin<'_, mode::Input> {
    async fn state(&self) -> PinState {
        self.s.read.sender().send(ReadState2::Requested);
        let mut receiver = self.s.read.receiver().unwrap();
        loop {
            let read_state = receiver.changed().await;
            if let ReadState2::Done(state) = read_state {
                break state;
            }
        }
    }

    /// Returns the final pin state after the edge.
    async fn wait_for_edge(&self) -> PinState {
        self.s.read_edge.sender().send(ReadState2::Requested);
        let mut receiver = self.s.read_edge.receiver().unwrap();
        loop {
            let read_edge_state = receiver.changed().await;
            if let ReadState2::Done(state) = read_edge_state {
                break state;
            }
        }
    }

    async fn wait_for_specific_edge(&self, final_pin_state: PinState) {
        loop {
            if self.wait_for_edge().await == final_pin_state {
                break;
            }
        }
    }

    async fn wait_for_state(&self, state: PinState) {
        select(
            async {
                if self.state().await != state {
                    pending().await
                }
            },
            self.wait_for_specific_edge(state),
        )
        .await;
    }
}

impl InputPin for Pin<'_, mode::Input> {
    async fn is_high(&mut self) -> Result<bool, Self::Error> {
        Ok(self.state().await == PinState::High)
    }

    async fn is_low(&mut self) -> Result<bool, Self::Error> {
        Ok(self.state().await == PinState::Low)
    }
}

impl Wait for Pin<'_, mode::Input> {
    async fn wait_for_high(&mut self) -> Result<(), Self::Error> {
        self.wait_for_state(PinState::High).await;
        Ok(())
    }

    async fn wait_for_low(&mut self) -> Result<(), Self::Error> {
        self.wait_for_state(PinState::Low).await;
        Ok(())
    }

    async fn wait_for_rising_edge(&mut self) -> Result<(), Self::Error> {
        self.wait_for_specific_edge(PinState::High).await;
        Ok(())
    }

    async fn wait_for_falling_edge(&mut self) -> Result<(), Self::Error> {
        self.wait_for_specific_edge(PinState::Low).await;
        Ok(())
    }

    async fn wait_for_any_edge(&mut self) -> Result<(), Self::Error> {
        self.wait_for_edge().await;
        Ok(())
    }
}
