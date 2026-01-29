use crate::*;

impl Pin<'_, mode::Input> {
    async fn op(&self, _op: InputOp) -> InputOp {
        // self.s.sender().send_if_modified(|request| {
        //     let request = request.as_mut().unwrap();
        //     let pull_up_enabled = match request.op {
        //         Op::Input {
        //             pull_up_enabled,
        //             op: _,
        //         } => pull_up_enabled,
        //         _ => unreachable!(),
        //     };
        //     let op = Op::Input {
        //         pull_up_enabled,
        //         op: Some(op),
        //     };
        //     if request.op != op {
        //         *request = Request {
        //             op,
        //             state: RequestState::Requested,
        //         };
        //         true
        //     } else if request.state == RequestState::Done {
        //         request.state = RequestState::Requested;
        //         true
        //     } else {
        //         false
        //     }
        // });
        // match self
        //     .s
        //     .receiver()
        //     .unwrap()
        //     .changed_and(|request| request.state == RequestState::Done)
        //     .await
        //     .op
        // {
        //     Op::Input {
        //         pull_up_enabled: _,
        //         op,
        //     } => op.unwrap(),
        //     _ => unreachable!(),
        // }
        todo!()
    }

    async fn state(&self) -> PinState {
        match self.op(InputOp::Read { response: None }).await {
            InputOp::Read { response } => response.unwrap(),
            _ => unreachable!(),
        }
    }

    async fn wait_for_specific_edge(&self, after_state: PinState) {
        self.op(InputOp::WaitForSpecificEdge { after_state }).await;
    }

    async fn wait_for_state(&self, state: PinState) {
        self.op(InputOp::WaitForState(state)).await;
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
        self.op(InputOp::WaitForAnyEdge).await;
        Ok(())
    }
}
