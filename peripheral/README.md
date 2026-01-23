# MCP23017 Emulator
## Requirements
- 16 GPIO pins that can be used as an input or an output, and can enable / disable a pull-up resistor. If they support interrupts and waiting  for changes with `async`, that's great. If not, the emulator falls back to polling.
- 2 GPIO output pins that can be configured to be push-pull or open-drain.
- 1 GPIO input pin to emulate the reset pin
- I2C peripheral capability

You can use this crate with any micro controller that supports these requirements. You just need to implement two traits.

## STM32
The traits are already implemented for STM32 micro controllers. Due to the way `embassy-stm32` requires features, each individual chip needs a feature to be added to this crate. Currently the `stm32f103c8` chip is supported, but more can be easily added!
