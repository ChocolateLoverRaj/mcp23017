use core::{array, fmt::Debug, slice};

use embassy_futures::select::{Either, select};
use embedded_hal::{digital::PinState, i2c::Operation};
use embedded_hal_async::digital::Wait;
use mcp23017_common::{AB, IoDirection, N_GPIO_PINS_PER_SET, Register, RegisterType};

use crate::{
    Mcp23017,
    util::{FromBits, IntoBits},
};

pub struct InterruptsEnabled;

pub struct Input<'a, ResetPin, I2c, InterruptPin> {
    mcp: &'a Mcp23017<ResetPin, I2c, InterruptPin>,
    index: usize,
    current_state: PinState,
}

impl<'a, ResetPin, I2c: embedded_hal_async::i2c::I2c, InterruptPin>
    Input<'a, ResetPin, I2c, InterruptPin>
{
    /// Currently this will always enable interrupts for this pin. Create an issue if you don't want this.
    pub async fn new(
        mcp: &'a Mcp23017<ResetPin, I2c, InterruptPin>,
        index: usize,
        pull_up_enabled: bool,
    ) -> Result<Self, I2c::Error> {
        let mut borrowed_pins = mcp.borrowed_pins.lock().await;
        let borrowed_pin = &mut borrowed_pins[index];
        if *borrowed_pin {
            panic!("pin {index} is already borrowed. tried to borrow it twice.")
        }
        *borrowed_pin = true;

        let mut i2c = mcp.i2c.lock().await;
        let mut registers_data = mcp.registers_data.lock().await;
        let ab = AB::from_index(index);
        {
            let new_io_directions = {
                let mut io_directions = registers_data.io_directions;
                io_directions[index] = IoDirection::Input;
                io_directions
            };
            if registers_data.io_directions != new_io_directions {
                i2c.write(
                    mcp.address(),
                    &[
                        Register {
                            _type: RegisterType::IODIR,
                            ab,
                        }
                        .address(false),
                        u8::from_bits_le(
                            new_io_directions.as_chunks::<N_GPIO_PINS_PER_SET>().0[ab.set_index()]
                                .map(bool::from),
                        ),
                    ],
                )
                .await?;
                registers_data.io_directions = new_io_directions;
            }
        }
        {
            let new_pull_ups_enabled = {
                let mut pull_ups_enabled = registers_data.pull_ups_enabled;
                pull_ups_enabled[index] = pull_up_enabled;
                pull_ups_enabled
            };
            if registers_data.pull_ups_enabled != new_pull_ups_enabled {
                i2c.write(
                    mcp.address(),
                    &[
                        Register {
                            _type: RegisterType::GPPU,
                            ab,
                        }
                        .address(false),
                        u8::from_bits_le(
                            new_pull_ups_enabled.as_chunks::<N_GPIO_PINS_PER_SET>().0
                                [ab.set_index()]
                            .map(bool::from),
                        ),
                    ],
                )
                .await?;
                registers_data.pull_ups_enabled = new_pull_ups_enabled;
            }
        }
        {
            let new_interrupts_enabled = {
                let mut interrupts_enabled = registers_data.interrupts_enabled;
                interrupts_enabled[index] = true;
                interrupts_enabled
            };
            if registers_data.interrupts_enabled != new_interrupts_enabled {
                i2c.write(
                    mcp.address(),
                    &[
                        Register {
                            _type: RegisterType::GPINTEN,
                            ab,
                        }
                        .address(false),
                        u8::from_bits_le(
                            new_interrupts_enabled.as_chunks::<N_GPIO_PINS_PER_SET>().0
                                [ab.set_index()]
                            .map(bool::from),
                        ),
                    ],
                )
                .await?;
                registers_data.interrupts_enabled = new_interrupts_enabled;
            }
        }
        let pin_state = {
            let mut byte = Default::default();
            i2c.write_read(
                mcp.address(),
                &[Register {
                    _type: RegisterType::GPIO,
                    ab,
                }
                .address(false)],
                slice::from_mut(&mut byte),
            )
            .await?;
            (byte & (1 << index / N_GPIO_PINS_PER_SET) != 0).into()
        };

        Ok(Self {
            mcp,
            index,
            current_state: pin_state,
        })
    }
}

#[derive(Debug)]
pub enum WaitForChangeError<InterruptPinError, I2cError> {
    InterruptPin(InterruptPinError),
    I2c(I2cError),
}

impl<ResetPin, I2c: embedded_hal_async::i2c::I2c, InterruptPin: Wait>
    Input<'_, ResetPin, I2c, InterruptPin>
{
    /// Reads INTF and INTCAP registers, signals other pins that were captured (not this pin),
    /// and returns the captured state of this pin (if this pin was captured)
    async fn collect_captured_interrupts(&mut self) -> Result<Option<PinState>, I2c::Error> {
        let mut flags_byte = Default::default();
        let mut captured_byte = Default::default();
        self.mcp
            .i2c
            .lock()
            .await
            .transaction(
                self.mcp.address(),
                &mut [
                    Operation::Write(&[Register {
                        _type: RegisterType::INTF,
                        ab: AB::from_index(self.index),
                    }
                    .address(false)]),
                    Operation::Read(slice::from_mut(&mut flags_byte)),
                    Operation::Write(&[Register {
                        _type: RegisterType::INTCAP,
                        ab: AB::from_index(self.index),
                    }
                    .address(false)]),
                    Operation::Read(slice::from_mut(&mut captured_byte)),
                ],
            )
            .await?;

        #[cfg(feature = "defmt")]
        defmt::trace!(
            "interrupt flags: {:010b}. interrupt captured states: {:010b}",
            flags_byte,
            captured_byte,
        );

        let captured_states = array::from_fn::<_, 8, _>(|i| {
            if flags_byte.into_bits_le()[i] {
                Some(PinState::from(captured_byte.into_bits_le()[i]))
            } else {
                None
            }
        });
        let self_pin_index_within_byte = self.index / N_GPIO_PINS_PER_SET;
        for (i, captured_state) in captured_states.iter().copied().enumerate() {
            if i != self_pin_index_within_byte
                && let Some(captured_state) = captured_state
            {
                self.mcp.unread_interrupts[AB::from_index(self.index).starting_index() + i]
                    .signal(captured_state);
            }
        }
        Ok(captured_states[self_pin_index_within_byte])
    }

    pub async fn wait_for_change(
        &mut self,
    ) -> Result<(), WaitForChangeError<InterruptPin::Error, I2c::Error>> {
        let pin_state = match select(self.mcp.unread_interrupts[self.index].wait(), async {
            Ok({
                let mut interrupt_pin = self.mcp.interrupt_pin.lock().await;
                loop {
                    interrupt_pin
                        .wait_for_low()
                        .await
                        .map_err(WaitForChangeError::InterruptPin)?;
                    let captured_state = self
                        .collect_captured_interrupts()
                        .await
                        .map_err(WaitForChangeError::I2c)?;
                    if let Some(captured_state) = captured_state {
                        break captured_state;
                    }
                }
            })
        })
        .await
        {
            Either::First(pin_state) => pin_state,
            Either::Second(result) => result?,
        };
        self.current_state = pin_state;
        Ok(())
    }

    /// When this input is initialized, its state is read.
    /// When [`Self::wait_for_change`] receives an interrupt, it reads the new state
    /// and saves it.
    ///
    /// So remember to call [`Self::wait_for_change`].
    pub fn last_known_state(&self) -> PinState {
        self.current_state
    }

    // /// This method currently does not combine reads with other pins in the set,
    // /// which could be very inefficient in scenarios when reading from multiple pins
    // /// from the same set at the same time.
    // pub async fn state(&mut self) -> Result<PinState, I2c::Error> {
    //     let mut byte = Default::default();
    //     self.mcp
    //         .i2c
    //         .lock()
    //         .await
    //         .write_read(
    //             self.mcp.address(),
    //             &[Register {
    //                 _type: RegisterType::GPIO,
    //                 ab: AB::from_index(self.index),
    //             }
    //             .address(false)],
    //             slice::from_mut(&mut byte),
    //         )
    //         .await?;
    //     let pin_state = (byte & (1 << self.index / N_GPIO_PINS_PER_SET) != 0).into();
    //     self.current_state = pin_state;
    //     Ok(pin_state)
    // }
}

// #[derive(Debug)]
// pub struct InputError<T>(T);

// impl<T: Debug> embedded_hal::digital::Error for InputError<T> {
//     fn kind(&self) -> embedded_hal::digital::ErrorKind {
//         ErrorKind::Other
//     }
// }

// impl<ResetPin, I2c: embedded_hal_async::i2c::I2c, InterruptPin> ErrorType
//     for Input<'_, ResetPin, I2c, InterruptPin>
// {
//     type Error = InputError<I2c::Error>;
// }

// impl<ResetPin, I2c: embedded_hal_async::i2c::I2c, InterruptPin> InputPin
//     for Input<'_, ResetPin, I2c, InterruptPin>
// {
//     /// See the documentation for [`Self::state`].
//     async fn is_high(&mut self) -> Result<bool, Self::Error> {
//         Ok(self.state().await.map_err(InputError)? == PinState::High)
//     }

//     /// See the documentation for [`Self::state`].
//     async fn is_low(&mut self) -> Result<bool, Self::Error> {
//         Ok(self.state().await.map_err(InputError)? == PinState::Low)
//     }
// }

// impl<ResetPin, I2c: embedded_hal_async::i2c::I2c, InterruptPin> embedded_hal::digital::InputPin
//     for Input<'_, ResetPin, I2c, InterruptPin>
// {
//     /// See the documentation for [`Self::state`].
//     fn is_high(&mut self) -> Result<bool, Self::Error> {
//         Ok(self.current_state == PinState::High)
//     }

//     /// See the documentation for [`Self::state`].
//     fn is_low(&mut self) -> Result<bool, Self::Error> {
//         Ok(self.current_state == PinState::High)
//     }
// }

// impl<ResetPin, I2c: embedded_hal_async::i2c::I2c, InterruptPin: Wait> Wait
//     for Input<'_, ResetPin, I2c, InterruptPin>
// {
//     async fn wait_for_high(&mut self) -> Result<(), Self::Error> {
//         todo!()
//     }

//     async fn wait_for_low(&mut self) -> Result<(), Self::Error> {
//         todo!()
//     }

//     async fn wait_for_rising_edge(&mut self) -> Result<(), Self::Error> {
//         todo!()
//     }

//     async fn wait_for_falling_edge(&mut self) -> Result<(), Self::Error> {
//         todo!()
//     }

//     async fn wait_for_any_edge(&mut self) -> Result<(), Self::Error> {}
// }
