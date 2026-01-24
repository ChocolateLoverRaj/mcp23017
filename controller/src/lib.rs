#![no_std]
mod input;
mod util;

use core::convert::Infallible;

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex, signal::Signal};
use embedded_hal::digital::PinState;
use embedded_hal_async::{delay::DelayNs, digital::OutputPin};
pub use input::*;
use mcp23017_common::{AB, IoDirection, N_TOTAL_GPIO_PINS, Register, RegisterType};

type M = CriticalSectionRawMutex;

const BASE_ADDRESS: u8 = 0x20;

struct RegistersData {
    pub io_directions: [IoDirection; N_TOTAL_GPIO_PINS],
    pub pull_ups_enabled: [bool; N_TOTAL_GPIO_PINS],
    pub interrupts_enabled: [bool; N_TOTAL_GPIO_PINS],
}

impl Default for RegistersData {
    fn default() -> Self {
        Self {
            io_directions: [IoDirection::Input; _],
            pull_ups_enabled: [false; _],
            interrupts_enabled: [false; _],
        }
    }
}

/// Currently this heavily uses the Embassy ecosystem.
/// If this doesn't work for you for some reason, make an issue.
pub struct Mcp23017<ResetPin, I2c, InterruptPin> {
    pub(crate) reset_pin: ResetPin,
    pub(crate) i2c: Mutex<M, I2c>,
    pub(crate) address_lower_bits: [bool; 3],
    pub(crate) interrupt_pin: Mutex<M, InterruptPin>,
    pub(crate) unread_interrupts: [Signal<M, PinState>; N_TOTAL_GPIO_PINS],
    pub(crate) borrowed_pins: Mutex<M, [bool; N_TOTAL_GPIO_PINS]>,
    pub(crate) registers_data: Mutex<M, RegistersData>,
}

fn address(address_lower_bits: [bool; 3]) -> u8 {
    let mut address = BASE_ADDRESS;
    for (i, bit) in address_lower_bits.into_iter().enumerate() {
        if bit {
            address |= 1 << i;
        }
    }
    address
}

impl<
    ResetPin: embedded_hal::digital::OutputPin<Error = Infallible>,
    I2c: embedded_hal_async::i2c::I2c,
    InterruptPin,
> Mcp23017<ResetPin, I2c, InterruptPin>
{
    /// When initializing the reset pin keep it high.
    /// Configure the I2C to be either 100 kHz or 400 kHz.
    /// Please make sure the MCP is reset.
    /// It is recommended to call [`Self::reset`].  
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
    pub async fn new(
        mut reset_pin: ResetPin,
        mut i2c: I2c,
        address_lower_bits: [bool; 3],
        interrupt_pin: InterruptPin,
        delay: &mut impl DelayNs,
    ) -> Result<Self, I2c::Error> {
        reset_pin.set_low();
        delay.delay_us(1).await;
        reset_pin.set_high();

        // Configure IOCON
        i2c.write(
            address(address_lower_bits),
            &[
                Register {
                    _type: RegisterType::IOCON,
                    ab: AB::A,
                }
                .address(false),
                // Enable interrupt mirroring and set interrupts to open-drain
                0b01000100,
            ],
        )
        .await?;

        Ok(Self {
            reset_pin,
            i2c: i2c.into(),
            address_lower_bits,
            interrupt_pin: interrupt_pin.into(),
            unread_interrupts: Default::default(),
            borrowed_pins: Default::default(),
            registers_data: Default::default(),
        })
    }
}

// impl<ResetPin: embedded_hal_async::digital::OutputPin<Error = Infallible>, I2c, InterruptPin>
//     Mcp23017<ResetPin, I2c, InterruptPin>
// {
//     /// If you want to use an output pin with possible errors, make an issue.
//     pub async fn reset(&mut self, delay: &mut impl DelayNs) {
//         self.reset_pin.set_low().await;
//         delay.delay_us(1).await;
//         self.reset_pin.set_high().await;
//     }
// }

// impl<ResetPin: embedded_hal::digital::OutputPin<Error = Infallible>, I2c, InterruptPin>
//     Mcp23017<ResetPin, I2c, InterruptPin>
// {
//     /// If you want to use an output pin with possible errors, make an issue.
//     pub async fn reset_sync_pin(&mut self, delay: &mut impl DelayNs) {
//         self.reset_pin.set_low();
//         // The docs say 1 us, but that isn't enough
//         delay.delay_us(100).await;
//         self.reset_pin.set_high();
//     }
// }

impl<ResetPin, I2c, InterruptPin> Mcp23017<ResetPin, I2c, InterruptPin> {
    fn address(&self) -> u8 {
        address(self.address_lower_bits)
    }

    // pub fn output(&self) -> Output<'_, ResetPin, I2c, InterruptPin> {
    //     // TODO: Don't allow the same pin to be borrowed twice at the same time
    //     Output { mcp: self }
    // }
}

impl<ResetPin, I2c: embedded_hal_async::i2c::I2c, InterruptPin>
    Mcp23017<ResetPin, I2c, InterruptPin>
{
    pub async fn input(
        &self,
        index: usize,
        pull_up_enabled: bool,
    ) -> Result<Input<'_, ResetPin, I2c, InterruptPin>, I2c::Error> {
        Input::new(self, index, pull_up_enabled).await
    }
}

// pub struct Output<'a, ResetPin, I2c, InterruptPin> {
//     mcp: &'a Mcp23017<ResetPin, I2c, InterruptPin>,
// }
