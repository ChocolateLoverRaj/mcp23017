use crate::*;

pub struct Pin<'a, Mode> {
    pub(crate) s: &'a Mcp23017ImmutablePin,
    pub(crate) index: usize,
    pub(crate) _mode: Mode,
}

impl<Mode> Pin<'_, Mode> {
    pub(crate) async fn update_config(&mut self, update_fn: impl FnOnce(&mut PinRegisters)) {
        let mut config = self.s.config.try_get().unwrap();
        update_fn(&mut config);
        self.s.requested_config.sender().send(config);
        self.s
            .config
            .receiver()
            .unwrap()
            .changed_and(|set_config| set_config == &config)
            .await;
    }
}

impl<'a> Pin<'a, mode::Input> {
    pub(crate) fn new(s: &'a Mcp23017ImmutablePin, index: usize) -> Self {
        Self {
            s,
            index,
            _mode: mode::Input,
        }
    }
}

impl<'a, Mode> Pin<'a, Mode> {
    pub async fn into_output(mut self, initial_value: PinState) -> Pin<'a, mode::Output> {
        self.update_config(|config| {
            config.direction = IoDirection::Output;
            config.latch = initial_value;
        })
        .await;
        Pin {
            s: self.s,
            index: self.index,
            _mode: mode::Output,
        }
    }

    pub async fn into_input(&mut self, pull_up_enabled: bool) -> Pin<'a, mode::Input> {
        self.update_config(|config| {
            config.direction = IoDirection::Input;
            config.pull_up_enabled = pull_up_enabled;
        })
        .await;
        Pin {
            s: self.s,
            index: self.index,
            _mode: mode::Input,
        }
    }

    pub async fn into_watch(&mut self, pull_up_enabled: bool) -> Pin<'a, mode::Watch> {
        todo!()
    }
}

impl<Mode> ErrorType for Pin<'_, Mode> {
    type Error = Infallible;
}
