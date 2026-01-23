#![no_std]

use core::convert::Infallible;

use embedded_hal::digital::OutputPin;
use embedded_hal_async::delay::DelayNs;

pub struct Mcp23017<ResetPin, I2c, InterruptPin> {
    reset_pin: ResetPin,
    i2c: I2c,
    address_lower_bits: [bool; 3],
    interrupt_pin: InterruptPin,
}

impl<ResetPin, I2c, InterruptPin> Mcp23017<ResetPin, I2c, InterruptPin> {
    /// When initializing the reset pin keep it high.
    /// Configure the I2C to be either 100 kHz or 400 kHz.
    ///
    /// Currently this requires 1 reset pin.
    /// Technically you don't need to connect to the reset pin.
    /// Make an issue if you want to be able use this without a reset pin.
    ///
    /// Currently this uses a single interrupt pin.
    /// Technically you don't need the interrupt pin if you aren't going to use interrupt based inputs.
    /// Make an issue if you want to be able to use this without an interrupt pin.
    /// Technically you can use two interrupt pins, one for A and one for B.
    /// However, this shouldn't improve performance much.
    /// Make an issue if you need separate interrupt pins for A and B.
    pub fn new(
        reset_pin: ResetPin,
        i2c: I2c,
        address_lower_bits: [bool; 3],
        interrupt_pin: InterruptPin,
    ) -> Self {
        Self {
            reset_pin,
            i2c,
            address_lower_bits,
            interrupt_pin,
        }
    }
}

impl<ResetPin: OutputPin<Error = Infallible>, I2c, InterruptPin>
    Mcp23017<ResetPin, I2c, InterruptPin>
{
    /// If you want to use an output pin with possible errors, make an issue.
    pub async fn reset(&mut self, delay: &mut impl DelayNs) {
        self.reset_pin.set_low();
        delay.delay_us(1).await;
        self.reset_pin.set_high();
    }
}
