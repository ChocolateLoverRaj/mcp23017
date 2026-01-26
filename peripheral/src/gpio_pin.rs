pub use embedded_hal::digital::PinState;
use mcp23017_common::InterruptMode;
pub use mcp23017_common::IoDirection;

pub trait GpioPin {
    fn configure(&mut self, io_direction: IoDirection, pull_up_enabled: bool, level: PinState);
    /// This function will not be called if this pin is configured to be in output mode.
    fn level(&self) -> PinState;
    /// Returns if the pin is capable of receiving interrupts (in input mode).
    fn can_wait(&mut self) -> bool;
    /// Returns when the pin's level becomes the specified level.
    /// This function will never be called if `can_wait` returns `false`.
    fn wait_for_level(&mut self, level: PinState) -> impl Future<Output = ()>;
}

pub trait InterruptPin {
    fn configure(&mut self, mode: InterruptMode, level: PinState);
}
