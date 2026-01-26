/// Use this mode for **infrequently** reading the pin state.
/// If you are using a `wait_` method in a loop, then you will have better performance
/// (less i2c traffic) using [`Watch`] mode.
pub struct Input;

/// Use this mode whenever you need to use the pin as an output
pub struct Output;

/// Use this mode if you need to constantly read the latest value,
/// using interrupts to notify when the pin changes.
/// The runner will keep interrupts always enabled for this pin, and keep
/// internally updating the last known state of the pin.
pub struct Watch;
