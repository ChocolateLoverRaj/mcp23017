# Regisers we need to read or write
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
Read when we receive an interrupt and we are
