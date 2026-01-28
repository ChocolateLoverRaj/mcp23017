use embassy_futures::{join::join_array, select::select_array};
use embedded_hal_async::{
    delay::DelayNs,
    digital::{OutputPin, Wait},
};

use crate::{register::write_registers, *};

pub async fn run<
    I2c: embedded_hal_async::i2c::I2c,
    ResetPin: OutputPin,
    InterruptPin: Wait,
    Delay: DelayNs,
>(
    mutable: &mut Mcp23017Mutable<I2c, ResetPin, InterruptPin, Delay>,
    immutable: &Mcp23017Immutable,
) -> Result<(), RunError<ResetPin::Error, InterruptPin::Error, I2c::Error>> {
    mutable
        .reset_pin
        .set_low()
        .await
        .map_err(RunError::ResetPin)?;
    mutable.delay.delay_us(1).await;
    mutable
        .reset_pin
        .set_high()
        .await
        .map_err(RunError::ResetPin)?;

    // Configure IOCON
    let address = address(mutable.address_lower_bits);
    mutable
        .i2c
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

    let mut registers = [PinRegisters::default(); N_TOTAL_GPIO_PINS];

    loop {
        // Make sure we have something to do
        #[cfg(feature = "defmt")]
        defmt::info!("Runner is idle");
        let wake_up_source = select(
            select_array(array::from_fn::<_, N_TOTAL_GPIO_PINS, _>(async |i| {
                #[cfg(feature = "defmt")]
                defmt::trace!("pin {} waiting for request signal", i);
                immutable.pins[i].request_signal.wait().await;
                #[cfg(feature = "defmt")]
                defmt::trace!("pin {} received request signal", i);
            })),
            mutable.interrupt_pin.wait_for_low(),
        )
        .await;
        #[cfg(feature = "defmt")]
        defmt::info!(
            "Runner doing something because of {}",
            defmt::Debug2Format(&wake_up_source)
        );

        // Read requests and immediately set them to processing, or done if no action is needed
        #[cfg(feature = "defmt")]
        defmt::trace!("reading requests");
        let requests = join_array(array::from_fn::<_, N_TOTAL_GPIO_PINS, _>(async |i| {
            #[cfg(feature = "defmt")]
            defmt::trace!("acquiring request lock {}", i);
            let mut request = immutable.pins[i].request.write().await;
            #[cfg(feature = "defmt")]
            defmt::trace!("acquired request lock {}", i);
            let request_before = *request;
            match request_before {
                Request {
                    op: Op::Output { latch },
                    state: RequestState::Requested,
                } => {
                    let change_dir = registers[i].io_dir != IoDirection::Output;
                    let change_latch = registers[i].latch != latch;
                    if change_dir || change_latch {
                        request.state = RequestState::ProcessingRequest;
                    } else {
                        request.state = RequestState::Done;
                    }
                    immutable.pins[i].response_signal.signal(());
                }
                _ => {}
            };
            request_before
        }))
        .await;
        #[cfg(feature = "defmt")]
        defmt::info!("requests: {}", defmt::Debug2Format(&requests));

        // Update IODIR
        let new_io_dirs = requests.map(|request| match request.op {
            Op::Output { latch: _ } => IoDirection::Output,
            _ => IoDirection::Input,
        });
        write_registers(
            &mut mutable.i2c,
            address,
            RegisterType::IODIR,
            registers.map(|register| register.io_dir.into()),
            new_io_dirs.map(|io_dir| io_dir.into()),
        )
        .await
        .map_err(RunError::I2c)?;
        for i in 0..N_TOTAL_GPIO_PINS {
            registers[i].io_dir = new_io_dirs[i];
        }

        // Update OLAT
        let new_latches = array::from_fn::<_, N_TOTAL_GPIO_PINS, _>(|i| match requests[i].op {
            Op::Output { latch } => latch,
            _ => registers[i].latch,
        });
        write_registers(
            &mut mutable.i2c,
            address,
            RegisterType::OLAT,
            registers.map(|register| register.latch.into()),
            new_latches.map(|latch| latch.into()),
        )
        .await
        .map_err(RunError::I2c)?;
        for i in 0..N_TOTAL_GPIO_PINS {
            registers[i].latch = new_latches[i];
        }

        // Set requests to done if applicable
        // Only set requests to done if they were not modified since we read them
        join_array(array::from_fn::<_, N_TOTAL_GPIO_PINS, _>(async |i| {
            let mut request = immutable.pins[i].request.write().await;
            #[cfg(feature = "defmt")]
            defmt::trace!("request: {}", defmt::Debug2Format(&request));
            if requests[i].op == request.op && request.state == RequestState::ProcessingRequest {
                match request.op {
                    Op::Output { latch: _ } => {
                        request.state = RequestState::Done;
                        immutable.pins[i].response_signal.signal(());
                    }
                    _ => {}
                }
            }
        }))
        .await;
    }
}
