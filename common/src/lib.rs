#![no_std]

use core::ops::Range;

use strum::{EnumCount, FromRepr, VariantArray};
/// There are 8 GPIO pins for set A and set B
pub const N_GPIO_PINS_PER_SET: usize = 8;
pub const N_TOTAL_GPIO_PINS: usize = N_GPIO_PINS_PER_SET * AB::COUNT;

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumCount, VariantArray, PartialOrd, Ord)]
pub enum AB {
    A,
    B,
}

impl AB {
    pub fn set_index(&self) -> usize {
        match self {
            Self::A => 0,
            Self::B => 1,
        }
    }

    pub fn from_index(index: usize) -> Self {
        match index / N_GPIO_PINS_PER_SET {
            0 => Self::A,
            1 => Self::B,
            _ => unreachable!(),
        }
    }

    pub fn starting_index(&self) -> usize {
        self.set_index() * N_GPIO_PINS_PER_SET
    }

    pub fn range(&self) -> Range<usize> {
        self.set_index() * N_GPIO_PINS_PER_SET..(self.set_index() + 1) * N_GPIO_PINS_PER_SET
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for AB {
    fn format(&self, fmt: defmt::Formatter) {
        let str = match self {
            Self::A => "A",
            Self::B => "B",
        };
        defmt::write!(fmt, "{}", str);
    }
}

pub struct FormatPinIndex(pub usize);

#[cfg(feature = "defmt")]
impl defmt::Format for FormatPinIndex {
    fn format(&self, fmt: defmt::Formatter) {
        let letter = AB::from_index(self.0);
        let index_within_letter = self.0 % N_GPIO_PINS_PER_SET;
        defmt::write!(fmt, "{}{}", letter, index_within_letter);
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumCount, FromRepr)]
#[repr(u8)]
pub enum RegisterType {
    IODIR,
    IPOL,
    GPINTEN,
    DEFVAL,
    INTCON,
    IOCON,
    GPPU,
    INTF,
    INTCAP,
    GPIO,
    OLAT,
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Register {
    pub _type: RegisterType,
    pub ab: AB,
}

impl Register {
    /// If the address is invalid, returns `None`
    pub fn from_address(address: u8, bank_mode: bool) -> Option<Self> {
        Some({
            if bank_mode {
                if address < RegisterType::COUNT as u8 {
                    Self {
                        ab: AB::A,
                        _type: RegisterType::from_repr(address)?,
                    }
                } else {
                    Self {
                        ab: AB::B,
                        _type: RegisterType::from_repr(address - RegisterType::COUNT as u8)?,
                    }
                }
            } else {
                Self {
                    ab: if address.is_multiple_of(2) {
                        AB::A
                    } else {
                        AB::B
                    },
                    _type: RegisterType::from_repr(address / 2)?,
                }
            }
        })
    }

    pub fn address(&self, bank_mode: bool) -> u8 {
        if bank_mode {
            self.ab.starting_index() as u8 + self._type as u8
        } else {
            (self._type as u8) * 2 + self.ab.set_index() as u8
        }
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptControl {
    /// Compare with `DEFVAL` register
    CompareWithConfiguredValue,
    CompareWithPreviousValue,
}

impl From<bool> for InterruptControl {
    fn from(value: bool) -> Self {
        match value {
            true => Self::CompareWithConfiguredValue,
            false => Self::CompareWithPreviousValue,
        }
    }
}

impl From<InterruptControl> for bool {
    fn from(value: InterruptControl) -> Self {
        match value {
            InterruptControl::CompareWithConfiguredValue => true,
            InterruptControl::CompareWithPreviousValue => false,
        }
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoDirection {
    Output,
    Input,
}

impl From<bool> for IoDirection {
    fn from(value: bool) -> Self {
        if value {
            IoDirection::Input
        } else {
            IoDirection::Output
        }
    }
}

impl From<IoDirection> for bool {
    fn from(value: IoDirection) -> Self {
        match value {
            IoDirection::Output => false,
            IoDirection::Input => true,
        }
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptMode {
    OpenDrain,
    ActiveDriver,
}

impl From<bool> for InterruptMode {
    fn from(value: bool) -> Self {
        match value {
            true => Self::OpenDrain,
            false => Self::ActiveDriver,
        }
    }
}

impl From<InterruptMode> for bool {
    fn from(value: InterruptMode) -> Self {
        match value {
            InterruptMode::OpenDrain => true,
            InterruptMode::ActiveDriver => false,
        }
    }
}
