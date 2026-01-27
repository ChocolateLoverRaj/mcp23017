#![no_std]
mod input;
pub mod mode;
mod output;
mod pin;
mod util;
mod watch;

use core::{array, convert::Infallible};

use embassy_futures::select::{Either, select, select_array};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, watch::Watch};
use embedded_hal::digital::{ErrorType, PinState};
use embedded_hal_async::{
    delay::DelayNs,
    digital::{InputPin, OutputPin, StatefulOutputPin, Wait},
    i2c,
};
use heapless::Vec;
use mcp23017_common::{
    AB, InterruptControl, IoDirection, N_TOTAL_GPIO_PINS, Register, RegisterType,
};
pub use pin::*;
use strum::VariantArray;
use util::*;

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
    pub(crate) op: Op,
    pub(crate) state: RequestState,
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
    pub fn new(
        i2c: I2c,
        address_lower_bits: [bool; 3],
        reset_pin: ResetPin,
        interrupt_pin: InterruptPin,
        delay: Delay,
    ) -> Self {
        Self {
            i2c,
            address_lower_bits,
            reset_pin,
            interrupt_pin,
            delay,
            immutable: Mcp23017Immutable {
                pins: array::from_fn(|_| Watch::new()),
            },
        }
    }

    pub fn run(
        &mut self,
    ) -> (
        impl Future<Output = Result<(), RunError<ResetPin::Error, InterruptPin::Error, I2c::Error>>>,
        [Pin<'_, mode::Input>; N_TOTAL_GPIO_PINS],
    ) {
        self.immutable.pins = array::from_fn(|_| {
            Watch::new_with(Request {
                op: Op::Input {
                    pull_up_enabled: false,
                    op: None,
                },
                state: RequestState::Done,
            })
        });
        (
            async {
                self.reset_pin.set_low().await.map_err(RunError::ResetPin)?;
                self.delay.delay_us(1).await;
                self.reset_pin
                    .set_high()
                    .await
                    .map_err(RunError::ResetPin)?;

                // Configure IOCON
                let address = address(self.address_lower_bits);
                self.i2c
                    .write(
                        address,
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
                        let mut receivers = self
                            .immutable
                            .pins
                            .each_ref()
                            .map(|pin| pin.receiver().unwrap());
                        let mut registers = [PinRegisters::default(); N_TOTAL_GPIO_PINS];
                        loop {
                            // Operations (each operation could be for A and/or B)
                            // Write I/O direction
                            // Write
                            let mut update_io_directions = [None; N_TOTAL_GPIO_PINS];

                            // Write pull up enabled
                            // Write
                            let mut update_pull_ups_enabled = [None; N_TOTAL_GPIO_PINS];

                            // Write latch
                            // Write
                            let mut update_latches = [None; N_TOTAL_GPIO_PINS];

                            // Write interrupt compare
                            // Write

                            // Write interrupt control
                            // Write

                            // Write interrupt enabled
                            // Write

                            // Read GPIO
                            let mut read_gpios = [false; N_TOTAL_GPIO_PINS];

                            let read_requests = self
                                .immutable
                                .pins
                                .each_ref()
                                .map(|pin| pin.try_get().unwrap());
                            for i in 0..N_TOTAL_GPIO_PINS {
                                if read_requests[i].state == RequestState::Requested {
                                    match read_requests[i].op {
                                        Op::Output { latch } => {
                                            if registers[i].direction != IoDirection::Output {
                                                update_io_directions[i] = Some(IoDirection::Output);
                                            }
                                            if registers[i].latch != latch {
                                                update_latches[i] = Some(latch);
                                            }
                                        }
                                        Op::Input {
                                            pull_up_enabled,
                                            op,
                                        } => {
                                            if registers[i].direction != IoDirection::Input {
                                                update_io_directions[i] = Some(IoDirection::Input)
                                            }
                                            if registers[i].pull_up_enabled != pull_up_enabled {
                                                update_pull_ups_enabled[i] = Some(pull_up_enabled);
                                            }
                                            if let Some(op) = op {
                                                match op {
                                                    InputOp::Read { response: _ } => {
                                                        read_gpios[i] = true;
                                                    }
                                                    _ => todo!(),
                                                }
                                            }
                                        }
                                        Op::Watch {
                                            pull_up_enabled,
                                            last_known_value,
                                        } => todo!(),
                                    }
                                    self.immutable.pins[i].sender().send_if_modified(|request| {
                                        let request = request.as_mut().unwrap();
                                        if request.op == read_requests[i].op {
                                            request.state = RequestState::ProcessingRequest;
                                            true
                                        } else {
                                            // TODO: Don't split into try_get and send_if_modified, cuz it creates a race condition
                                            panic!()
                                        }
                                    });
                                }
                            }

                            #[cfg_attr(feature = "defmt", derive(defmt::Format))]
                            #[derive(Debug)]
                            enum OperationType {
                                Read,
                                Write,
                            }

                            #[cfg_attr(feature = "defmt", derive(defmt::Format))]
                            #[derive(Debug)]
                            struct Operation {
                                _type: OperationType,
                                /// Largest size: [register address, register a value, register b value]
                                buffer: Vec<u8, 3>,
                            }

                            impl Operation {
                                pub fn operation(&mut self) -> i2c::Operation<'_> {
                                    match self._type {
                                        OperationType::Read => {
                                            i2c::Operation::Read(&mut self.buffer)
                                        }
                                        OperationType::Write => i2c::Operation::Write(&self.buffer),
                                    }
                                }
                            }

                            // const MAX_OPERATIONS: usize = 3;
                            // let mut operations = Vec::<Operation, MAX_OPERATIONS>::new();

                            fn write_operation<T: Into<bool> + Copy>(
                                register: RegisterType,
                                current: [T; N_TOTAL_GPIO_PINS],
                                updates: [Option<T>; N_TOTAL_GPIO_PINS],
                            ) -> Option<Operation> {
                                if updates.iter().any(Option::is_some) {
                                    Some(Operation {
                                        _type: OperationType::Write,
                                        buffer: {
                                            let mut buffer = Vec::new();
                                            // Placeholder for register address
                                            buffer.push(Default::default()).unwrap();
                                            let mut modified_a = false;
                                            for ab in AB::VARIANTS.iter().copied() {
                                                let updates = &updates[ab.range()];
                                                if updates.iter().any(Option::is_some) {
                                                    buffer
                                                        .push(u8::from_bits_le(array::from_fn(
                                                            |i| {
                                                                (if let Some(direction) = updates[i]
                                                                {
                                                                    if ab == AB::A {
                                                                        modified_a = true;
                                                                    }
                                                                    direction
                                                                } else {
                                                                    current[ab.starting_index() + i]
                                                                })
                                                                .into()
                                                            },
                                                        )))
                                                        .unwrap();
                                                }
                                            }
                                            buffer[0] = Register {
                                                _type: register,
                                                ab: if modified_a { AB::A } else { AB::B },
                                            }
                                            .address(false);
                                            buffer
                                        },
                                    })
                                } else {
                                    None
                                }
                            }

                            let mut do_operation = false;
                            if let Some(mut operation) = write_operation(
                                RegisterType::IODIR,
                                registers.map(|registers| registers.direction),
                                update_io_directions,
                            ) {
                                do_operation = true;
                                self.i2c
                                    .transaction(address, &mut [operation.operation()])
                                    .await
                                    .map_err(RunError::I2c)?;
                            }
                            if let Some(mut operation) = write_operation(
                                RegisterType::GPPU,
                                registers.map(|registers| registers.pull_up_enabled),
                                update_pull_ups_enabled,
                            ) {
                                do_operation = true;
                                self.i2c
                                    .transaction(address, &mut [operation.operation()])
                                    .await
                                    .map_err(RunError::I2c)?;
                            }
                            if let Some(mut operation) = write_operation(
                                RegisterType::OLAT,
                                registers.map(|registers| registers.latch),
                                update_latches,
                            ) {
                                do_operation = true;
                                self.i2c
                                    .transaction(address, &mut [operation.operation()])
                                    .await
                                    .map_err(RunError::I2c)?;
                            }
                            let mut read_pin_states = [None; N_TOTAL_GPIO_PINS];
                            if read_gpios.contains(&true) {
                                let mut read_a = false;
                                let mut buffer = Vec::<_, 2>::new();
                                for ab in AB::VARIANTS.iter().copied() {
                                    if read_gpios[ab.range()].contains(&true) {
                                        if ab == AB::A {
                                            read_a = true;
                                        }
                                        buffer.push(Default::default()).unwrap();
                                    }
                                }
                                self.i2c
                                    .write_read(
                                        address,
                                        &[Register {
                                            _type: RegisterType::GPIO,
                                            ab: if read_a { AB::A } else { AB::B },
                                        }
                                        .address(false)],
                                        &mut buffer,
                                    )
                                    .await
                                    .map_err(RunError::I2c)?;
                                for (i, ab) in AB::VARIANTS
                                    [if read_a { 0..buffer.len() } else { 1..2 }]
                                .iter()
                                .copied()
                                .enumerate()
                                {
                                    read_pin_states[ab.range()].copy_from_slice(
                                        &buffer[i]
                                            .into_bits_le()
                                            .map(|bool| Some(PinState::from(bool))),
                                    );
                                }
                            }

                            if !do_operation {
                                let request = select_array(
                                    receivers.each_mut().map(async |pin| pin.changed().await),
                                )
                                .await;
                                #[cfg(feature = "defmt")]
                                defmt::trace!("request: {}", defmt::Debug2Format(&request));
                                continue;
                            }

                            for i in 0..N_TOTAL_GPIO_PINS {
                                if let Some(value) = update_io_directions[i] {
                                    registers[i].direction = value;
                                }
                                if let Some(value) = update_latches[i] {
                                    registers[i].latch = value;
                                }
                                self.immutable.pins[i].sender().send_if_modified(|request| {
                                    let request = request.as_mut().unwrap();
                                    if request
                                        == (&Request {
                                            op: read_requests[i].op,
                                            state: RequestState::ProcessingRequest,
                                        })
                                    {
                                        match &mut request.op {
                                            Op::Input {
                                                pull_up_enabled: _,
                                                op,
                                            } => match op {
                                                Some(InputOp::Read { response }) => {
                                                    *response = Some(read_pin_states[i].unwrap());
                                                }
                                                _ => {}
                                            },
                                            _ => {}
                                        }
                                        request.state = RequestState::Done;
                                        true
                                    } else {
                                        false
                                    }
                                });
                            }
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
