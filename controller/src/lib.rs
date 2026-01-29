#![no_std]
mod input;
pub mod mode;
mod output;
mod pin;
mod register;
mod runner;
mod util;
mod watch;

use core::{array, convert::Infallible};

use embassy_futures::select::select;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, rwlock::RwLock, signal::Signal};
use embedded_hal::digital::{ErrorType, PinState};
use embedded_hal_async::{
    delay::DelayNs,
    digital::{InputPin, OutputPin, StatefulOutputPin, Wait},
};
use heapless::Vec;
use mcp23017_common::{
    AB, InterruptControl, IoDirection, N_TOTAL_GPIO_PINS, Register, RegisterType,
};
pub use pin::*;
use util::*;

use crate::{mode::Input, runner::run};

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
    io_dir: IoDirection,
    pull_up_enabled: bool,
    latch: PinState,
    int_enabled: bool,
    int_control: InterruptControl,
    int_compare: PinState,
}

impl Default for PinRegisters {
    fn default() -> Self {
        Self {
            io_dir: IoDirection::Input,
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
    pub(crate) op: Op,
    pub(crate) state: RequestState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequestState {
    Requested,
    ProcessingRequest,
    Done,
}

struct Mcp23017ImmutablePin {
    request: RwLock<M, Request>,
    request_signal: Signal<M, ()>,
    response_signal: Signal<M, ()>,
}

impl Default for Mcp23017ImmutablePin {
    fn default() -> Self {
        Self {
            request: RwLock::new(Request {
                op: Op::Input {
                    pull_up_enabled: false,
                    op: None,
                },
                state: RequestState::Done,
            }),
            request_signal: Signal::new(),
            response_signal: Signal::new(),
        }
    }
}

struct Mcp23017Immutable {
    pins: [Mcp23017ImmutablePin; N_TOTAL_GPIO_PINS],
}

struct Mcp23017Mutable<I2c, ResetPin, InterruptPin, Delay> {
    i2c: I2c,
    address_lower_bits: [bool; 3],
    reset_pin: ResetPin,
    interrupt_pin: InterruptPin,
    delay: Delay,
}

pub struct Mcp23017<I2c, ResetPin, InterruptPin, Delay> {
    immutable: Mcp23017Immutable,
    mutable: Mcp23017Mutable<I2c, ResetPin, InterruptPin, Delay>,
}

#[allow(non_snake_case)]
pub struct InitialPins<'a> {
    pub A0: Pin<'a, Input>,
    pub A1: Pin<'a, Input>,
    pub A2: Pin<'a, Input>,
    pub A3: Pin<'a, Input>,
    pub A4: Pin<'a, Input>,
    pub A5: Pin<'a, Input>,
    pub A6: Pin<'a, Input>,
    pub A7: Pin<'a, Input>,
    pub B0: Pin<'a, Input>,
    pub B1: Pin<'a, Input>,
    pub B2: Pin<'a, Input>,
    pub B3: Pin<'a, Input>,
    pub B4: Pin<'a, Input>,
    pub B5: Pin<'a, Input>,
    pub B6: Pin<'a, Input>,
    pub B7: Pin<'a, Input>,
}

impl<'a> InitialPins<'a> {
    fn new(pins: [Pin<'a, Input>; N_TOTAL_GPIO_PINS]) -> Self {
        #[allow(non_snake_case)]
        let [
            A0,
            A1,
            A2,
            A3,
            A4,
            A5,
            A6,
            A7,
            B0,
            B1,
            B2,
            B3,
            B4,
            B5,
            B6,
            B7,
        ] = pins;
        Self {
            A0,
            A1,
            A2,
            A3,
            A4,
            A5,
            A6,
            A7,
            B0,
            B1,
            B2,
            B3,
            B4,
            B5,
            B6,
            B7,
        }
    }
}

impl<I2c: embedded_hal_async::i2c::I2c, ResetPin: OutputPin, InterruptPin: Wait, Delay: DelayNs>
    Mcp23017<I2c, ResetPin, InterruptPin, Delay>
{
    pub fn new(
        i2c: I2c,
        address_lower_bits: [bool; 3],
        reset_pin: ResetPin,
        interrupt_pin: InterruptPin,
        delay: Delay,
    ) -> Self {
        Self {
            immutable: Mcp23017Immutable {
                pins: array::from_fn(|_| Default::default()),
            },
            mutable: Mcp23017Mutable {
                i2c,
                address_lower_bits,
                reset_pin,
                interrupt_pin,
                delay,
            },
        }
    }

    /// Get a runner future and access to pins.
    /// The runner must be polled basically for the lifetime of the pins.
    /// Currently, all errors will result in the error future being `Poll::Ready(Err(error)))`,
    /// and the only way to recover from the error is to call `run` again.
    ///
    /// If you need to recover from errors and the API is too, inconvenient, create an issue.
    pub fn run(
        &mut self,
    ) -> (
        impl Future<Output = Result<(), RunError<ResetPin::Error, InterruptPin::Error, I2c::Error>>>,
        InitialPins<'_>,
    ) {
        self.immutable.pins = array::from_fn(|_| Default::default());
        (
            run(&mut self.mutable, &self.immutable),
            InitialPins::new(array::from_fn(|index| {
                Pin::new(&self.immutable.pins[index])
            })),
        )
    }
}
