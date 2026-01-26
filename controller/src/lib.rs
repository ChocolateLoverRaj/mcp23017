#![no_std]
mod input;
pub mod mode;
mod output;
mod pin;
mod util;
mod watch;

use core::{array, convert::Infallible, future::pending};

use embassy_futures::select::{Either, select};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, watch::Watch};
use embedded_hal::{
    digital::{ErrorType, PinState},
    spi::Operation,
};
use embedded_hal_async::{
    delay::DelayNs,
    digital::{InputPin, OutputPin, StatefulOutputPin, Wait},
};
use mcp23017_common::{
    AB, InterruptControl, IoDirection, N_TOTAL_GPIO_PINS, Register, RegisterType,
};
pub use pin::*;

type M = CriticalSectionRawMutex;

const BASE_ADDRESS: u8 = 0x20;

fn address(address_lower_bits: [bool; 3]) -> u8 {
    let mut address = BASE_ADDRESS;
    for (i, bit) in address_lower_bits.into_iter().enumerate() {
        if bit {
            address |= 1 << i;
        }
    }
    address
}

#[derive(Debug)]
pub enum RunError<ResetPinError, InterruptPinError, I2cError> {
    ResetPin(ResetPinError),
    InterruptPin(InterruptPinError),
    I2c(I2cError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PinRegisters {
    direction: IoDirection,
    pull_up_enabled: bool,
    latch: PinState,
    int_enabled: bool,
    int_control: InterruptControl,
    int_compare: PinState,
}

impl Default for PinRegisters {
    fn default() -> Self {
        Self {
            direction: IoDirection::Input,
            pull_up_enabled: false,
            int_enabled: false,
            latch: PinState::Low,
            int_control: InterruptControl::CompareWithPreviousValue,
            int_compare: PinState::Low,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputOp {
    Read {
        response: Option<PinState>,
    },
    /// The runner sets the compare value to the opposite state and waits for an interrupt.
    WaitForState(PinState),
    /// The runner reads the GPIO reg, sets the compare value to compare with previous,
    /// and waits for an interrupt.
    WaitForAnyEdge,
    /// The runner reads the GPIO reg. sets the compare value to compare with previous,
    /// and waits for interrupts until the captured value is the specified final state.
    /// The runner should wait for up to 2 interrupts.
    WaitForSpecificEdge {
        after_state: PinState,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    /// Sets io direction to
    Output { latch: PinState },
    Input {
        pull_up_enabled: bool,
        op: Option<InputOp>,
    },
    Watch {
        pull_up_enabled: bool,
        last_known_value: Option<PinState>,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Request {
    op: Op,
    state: RequestState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequestState {
    Requested,
    ProcessingRequest,
    Done,
}

type Mcp23017ImmutablePin = Watch<M, Request, 2>;

struct Mcp23017Immutable {
    pins: [Mcp23017ImmutablePin; N_TOTAL_GPIO_PINS],
}

pub struct Mcp23017<I2c, ResetPin, InterruptPin, Delay> {
    i2c: I2c,
    address_lower_bits: [bool; 3],
    reset_pin: ResetPin,
    interrupt_pin: InterruptPin,
    delay: Delay,
    immutable: Mcp23017Immutable,
}

impl<I2c: embedded_hal_async::i2c::I2c, ResetPin: OutputPin, InterruptPin: Wait, Delay: DelayNs>
    Mcp23017<I2c, ResetPin, InterruptPin, Delay>
{
    pub fn run(
        &mut self,
    ) -> (
        impl Future<Output = Result<(), RunError<ResetPin::Error, InterruptPin::Error, I2c::Error>>>,
        [Pin<'_, mode::Input>; N_TOTAL_GPIO_PINS],
    ) {
        (
            async {
                self.reset_pin.set_low().await.map_err(RunError::ResetPin)?;
                self.delay.delay_us(1).await;
                self.reset_pin
                    .set_high()
                    .await
                    .map_err(RunError::ResetPin)?;

                // Configure IOCON
                self.i2c
                    .write(
                        address(self.address_lower_bits),
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
                    .await
                    .map_err(RunError::I2c)?;

                let interrupt_pin_signal = Watch::<M, _, 1>::new();
                match select(
                    async {
                        let sender = interrupt_pin_signal.sender();
                        loop {
                            self.interrupt_pin
                                .wait_for_low()
                                .await
                                .map_err(RunError::InterruptPin)?;
                            sender.send(PinState::Low);
                            self.interrupt_pin
                                .wait_for_high()
                                .await
                                .map_err(RunError::InterruptPin)?;
                            sender.send(PinState::High);
                        }
                    },
                    async {
                        let mut receiver = interrupt_pin_signal.receiver().unwrap();
                        loop {
                            // Operations (each operation could be for A and/or B)
                            // Write I/O direction
                            // Write

                            // Write pull up enabled
                            // Write

                            // Write latch
                            // Write

                            // Write interrupt compare
                            // Write

                            // Write interrupt control
                            // Write

                            // Write interrupt enabled
                            // Write

                            // let (pin_requests, handle_interrupt) = {
                            //     let mut pin_requests =
                            //         self.immutable.pin_requests.each_ref().map(Signal::try_take);
                            //     let mut handle_interrupt = interrupt_pin_signal
                            //         .try_get()
                            //         .is_some_and(|pin_state| pin_state == PinState::Low);
                            //     if pin_requests.iter().all(Option::is_none) && handle_interrupt {
                            //         match select(
                            //             select_array(
                            //                 self.immutable
                            //                     .pin_requests
                            //                     .each_ref()
                            //                     .map(Signal::wait),
                            //             ),
                            //             async {
                            //                 receiver
                            //                     .changed_and(|pin_state| {
                            //                         *pin_state == PinState::Low
                            //                     })
                            //                     .await;
                            //             },
                            //         )
                            //         .await
                            //         {
                            //             Either::First((message, index)) => {
                            //                 pin_requests[index] = Some(message);
                            //             }
                            //             Either::Second(()) => {
                            //                 handle_interrupt = true;
                            //             }
                            //         }
                            //     }
                            //     (pin_requests, handle_interrupt)
                            // };

                            // For each pin
                            // If change direction, write the direction
                            // If change pull, change the pull
                            // If read_write, read or write
                            // If change interrupts enabled or disabled, set interrupts enabled or disabled

                            // If the direction is an input and interrupts for are enabled, the interrupt is active, and the  pin is not requested to be read, read INTF and GPIO.
                        }
                    },
                )
                .await
                {
                    Either::First(result) => result,
                    Either::Second(result) => result,
                }
            },
            array::from_fn(|index| Pin::new(&self.immutable.pins[index])),
        )
    }
}
