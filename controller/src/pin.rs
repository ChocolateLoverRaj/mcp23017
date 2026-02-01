use crate::*;

pub struct Pin<'a, Mode> {
    pub(crate) s: &'a Mcp23017ImmutablePin,
    pub(crate) _mode: Mode,
}

impl<Mode> Pin<'_, Mode> {
    pub(crate) async fn update_op(&self, new_op: Op) {
        {
            let mut request = self.s.request.write().await;
            if &request.op == &new_op {
                return;
            }
            request.op = new_op;
            request.state = RequestState::Requested;
            #[cfg(feature = "defmt")]
            defmt::trace!("pin signaling request: {}", defmt::Debug2Format(&request));
            self.s.request_signal.signal(());
        }
        loop {
            {
                let request = self.s.request.read().await;
                if request.state == RequestState::Done {
                    break;
                }
            }
            #[cfg(feature = "defmt")]
            defmt::trace!("pin waiting for response signal");
            self.s.response_signal.wait().await;
            #[cfg(feature = "defmt")]
            defmt::trace!("pin received response signal");
        }
    }
}

impl<'a> Pin<'a, mode::Input> {
    pub(crate) fn new(s: &'a Mcp23017ImmutablePin) -> Self {
        Self {
            s,
            _mode: mode::Input,
        }
    }
}

impl<'a, Mode> Pin<'a, Mode> {
    pub async fn into_output(self, initial_value: PinState) -> Pin<'a, mode::Output> {
        self.update_op(Op::Output {
            latch: initial_value,
        })
        .await;
        Pin {
            s: self.s,
            _mode: mode::Output,
        }
    }

    pub async fn into_input(self, pull_up_enabled: bool) -> Pin<'a, mode::Input> {
        self.update_op(Op::Input {
            pull_up_enabled,
            op: None,
        })
        .await;
        Pin {
            s: self.s,
            _mode: mode::Input,
        }
    }

    pub async fn into_watch(self, pull_up_enabled: bool) -> Pin<'a, mode::Watch> {
        let new_op = Op::Watch {
            pull_up_enabled,
            last_known_value: None,
        };
        {
            let mut request = self.s.request.write().await;
            if &request.op != &new_op {
                request.op = new_op;
                request.state = RequestState::Requested;
                self.s.request_signal.signal(());
            }
        }
        loop {
            {
                let request = self.s.request.read().await;
                match request.op {
                    Op::Watch {
                        pull_up_enabled: _,
                        last_known_value,
                    } => {
                        if last_known_value.is_some() {
                            break;
                        }
                    }
                    _ => {}
                };
            }
            self.s.response_signal.wait().await;
        }
        Pin {
            s: self.s,
            _mode: mode::Watch,
        }
    }
}

impl<Mode> ErrorType for Pin<'_, Mode> {
    type Error = Infallible;
}
