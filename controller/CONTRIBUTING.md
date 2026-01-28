# Registers we need to read or write
## `IODIR`
Never read.

Written whenever changing from input to output or output to input.

## `IPOL`
Never read.

Never written (we want it to always be `0`).

## `GPINTEN`
Never read.

Written to `1` for whenever WaitForState (after reading the current state and its not the target state), WaitForAnyEdge, WaitForSpecificEdge is requested. Written to `1` in watch mode. In output mode, we don't care what this is. In input mode when the request is None or Read, we set this to `0`.

## `DEFVAL`
Never read.

Never written (we don't care what it is).

## `INTCON`
Never read.

Never written (we want it to always be `0`).

## `IOCON`
Never read.

Written once after a reset to configure interrupt stuff.

## `GPPU`
Never read.

Written whenever a pin is configured as an input and the configured pull is different from the current value in the register.

## `INTF`
Read when receiving an interrupt and we are processing a `WaitForAnyEdge` or `WaitForSpecificEdge` request. 

Never written.

## `INTCAP`
Never read.

Never written.

## `GPIO`
Read when:
- In input mode and a Read is requested
- In input mode and a WaitForState is requested, we first read `GPIO`, and if it's not the state to wait for, after receiving an interrupt we read this again.
- In input mode and WaitForAnyEdge is requested, to clear `INTF`
- In input mode and WaitForSpecificEdge is requested, to clear `INTF`
- In watch mode it is read initially and then it is read again on every interrupt.

Never written

## `OLAT`
Never read.

Written when configuring a pin as an output and when calling changing an output pin's state.

# Processing requests
## Output
- Change the request to processing
- Simply update `IODIR` and `OLAT`
- Change the request to done

## Input(None)
- Change the request to processing
- Simply update `IODIR` and `GPPU`
- Change the request to done

## Input(Read)
- Change the request to processing
- Update `IODIR` and `GPPU`
- Read `GPIO`
- Change the request to done, inserting the read value from `GPIO`

## Input(WaitForState)
- Change the request to processing
- Update `IODIR` and `GPPU`
- Read `GPIO`
- If the state is not the state we're waiting for, write to `GPINTEN` to enable interrupts for the pin.
- On every interrupt, read `GPIO` again.
- Once the `GPIO` is the state we're waiting for, write to `GPINTEN` to disable interrupts for the pin.
- Change the request to done

## Input(WaitForAnyEdge)
- Change the request to processing
- Update `IODIR` and `GPPU`
- Write to `GPINTEN` to enable interrupts for this pin.
- On an interrupt, read `INTF` to check if this changed.
- Then read `GPIO` to clear `INTF`
- Write to `GPINTEN` to disable interrupts for this pin.
- Change request to done

## Input(WaitForSpecificEdge)
- Change the request to processing
- Update `IODIR` and `GPPU`
- Read `GPIO` to know the current state
- Write to `GPINTEN` to enable interrupts for this pin.
- On an interrupt, read `INTF` to check if this changed. Depending on if the current state is the same as the end state we're waiting for, we need to wait for either 1 or 2 `INTF`s.
- Then read `GPIO` to clear `INTF`
- Write to `GPINTEN` to disable interrupts for this pin.
- Change request to done

## Watch
- Change the request to processing
- Update `IODIR` and `GPPU`
- Read `GPIO` and Update the watched value
- Write to `GPINTEN` to enable interrupts for this pin.
- On an interrupt, read `GPIO` and update the watched value

# Note about reading `GPIO`
Reading `GPIO` clears `INTF`. So if we care about `INTF` (whenever we are processing an `WaitForAnyEdge` or `WaitForSpecificEdge` request), we must always read `INTF` before reading `GPIO` and process those requests related to `INTF` if there is a flag that we care about.
