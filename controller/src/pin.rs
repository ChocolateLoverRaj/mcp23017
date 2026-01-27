use crate::*;

pub struct Pin<'a, Mode> {
    pub(crate) s: &'a Mcp23017ImmutablePin,
    pub(crate) _mode: Mode,
}

impl<Mode> Pin<'_, Mode> {
    pub(crate) async fn update_op(&self, new_op: Op) {
        self.s.sender().send_if_modified(|request| {
            let request = request.as_mut().unwrap();
            if &request.op != &new_op {
                request.op = new_op;
                request.state = RequestState::Requested;
                true
            } else {
                false
            }
        });
        self.s
            .receiver()
            .unwrap()
            .changed_and(|request| request.state == RequestState::Done)
            .await;
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
        self.update_op(Op::Watch {
            pull_up_enabled,
            last_known_value: None,
        })
        .await;
        Pin {
            s: self.s,
            _mode: mode::Watch,
        }
    }
}

impl<Mode> ErrorType for Pin<'_, Mode> {
    type Error = Infallible;
}
