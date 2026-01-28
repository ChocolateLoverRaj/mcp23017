use crate::*;
use mcp23017_common::{
    AB::{self, *},
    N_GPIO_PINS_PER_SET, N_TOTAL_GPIO_PINS, Register, RegisterType,
};

/// Writes to A, B, both, or none,depending on the values that are `Some`.
pub async fn write_registers<I2c: embedded_hal_async::i2c::I2c>(
    i2c: &mut I2c,
    i2c_address: u8,
    register: RegisterType,
    current_values: [bool; N_TOTAL_GPIO_PINS],
    new_values: [bool; N_TOTAL_GPIO_PINS],
) -> Result<(), I2c::Error> {
    let write_a = &current_values[A.range()] != &new_values[A.range()];
    let write_b = &current_values[B.range()] != &new_values[B.range()];
    let write_count = write_a as usize + write_b as usize;

    if write_count > 0 {
        let new_a_byte = u8::from_bits_le(new_values[A.range()].try_into().unwrap());
        let new_b_byte = u8::from_bits_le(new_values[B.range()].try_into().unwrap());
        let buffer: Vec<_, 3> = match (write_a, write_b) {
            (true, false) => Vec::from_slice(&[
                Register {
                    _type: register,
                    ab: A,
                }
                .address(false),
                new_a_byte,
            ]),
            (false, true) => Vec::from_slice(&[
                Register {
                    _type: register,
                    ab: B,
                }
                .address(false),
                new_b_byte,
            ]),
            (true, true) => Vec::from_slice(&[
                Register {
                    _type: register,
                    ab: A,
                }
                .address(false),
                new_a_byte,
                new_b_byte,
            ]),
            _ => unreachable!(),
        }
        .unwrap();
        i2c.write(i2c_address, &buffer).await?;
    }

    Ok(())
}

/// Writes all `Some` with the  read value.
pub async fn read_registers<I2c: embedded_hal_async::i2c::I2c>(
    i2c: &mut I2c,
    i2c_address: u8,
    register: RegisterType,
    values: &mut [Option<bool>; N_TOTAL_GPIO_PINS],
) -> Result<(), I2c::Error> {
    let read_a = values[A.range()].iter().any(|value| value.is_some());
    let read_b = values[B.range()].iter().any(|value| value.is_some());
    let read_count = read_a as usize + read_b as usize;

    if read_count > 0 {
        let mut buffer = [Default::default(); 2];
        i2c.write_read(
            i2c_address,
            &[Register {
                _type: register,
                ab: if read_a { A } else { B },
            }
            .address(false)],
            &mut buffer[..read_count],
        )
        .await?;

        let (a_byte, b_byte) = match (read_a, read_b) {
            (true, false) => (Some(buffer[0]), None),
            (false, true) => (None, Some(buffer[1])),
            (true, true) => (Some(buffer[0]), Some(buffer[1])),
            _ => unreachable!(),
        };
        if let Some(a_byte) = a_byte {
            let a_values = a_byte.into_bits_le();
            let values = &mut values[A.range()];
            for i in 0..N_GPIO_PINS_PER_SET {
                if let Some(value) = values[i].as_mut() {
                    *value = a_values[i];
                }
            }
        }
        if let Some(b_byte) = b_byte {
            let b_values = b_byte.into_bits_le();
            let values = &mut values[B.range()];
            for i in 0..N_GPIO_PINS_PER_SET {
                if let Some(value) = values[i].as_mut() {
                    *value = b_values[i];
                }
            }
        }
    }

    Ok(())
}
