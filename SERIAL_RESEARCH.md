# CC2 Serial and RPMsg Research

## Goal

Investigate how the stock Centauri Carbon 2 firmware communicates with motor-control hardware, with the future goal of controlling motion through Klipper/Moonraker or a COSMOS-style replacement stack.

## Current Context

The local CC2 repo currently contains early reverse-engineering helpers for framebuffer and touch input. It does not yet contain a motion-control stack.

OpenCentauri COSMOS for the original Centauri Carbon uses a different approach from the stock firmware:

- Runs a Yocto Linux image on the stock Allwinner R528 platform.
- Runs Klipper/Kalico as the motion planner.
- Runs Moonraker as the HTTP/WebSocket API layer.
- Uses Klipper MCU firmware on controller endpoints rather than Elegoo's stock motion protocol.
- Talks to the mainboard motion MCU/DSP over an RPMsg/serial-like interface.
- Talks to bed and toolhead boards as additional Klipper MCUs.

Moonraker does not control motors directly. Moonraker talks to Klipper. Klipper controls motors only when the target controller is running Klipper MCU firmware or an equivalent implementation of Klipper's MCU protocol.

## Stock CC2 Device Findings

The stock CC2 OS exposes these visible serial devices:

- `/dev/ttyS0`
- `/dev/ttyS1`
- `/dev/ttyS2`
- `/dev/ttyS3`

Initial passive serial probing found:

- `/dev/ttyS0`: no data at tested baud rates.
- `/dev/ttyS1`: emitted binary data at `115200`, but only while `elegoo_printer` was running.
- `/dev/ttyS2`: open failed with `Not a typewriter`.
- `/dev/ttyS3`: emitted binary data at `1000000`; likely the external CANVAS filament switcher.

File descriptor inspection of `elegoo_printer` showed it opens:

- `/dev/rpmsg1`
- `/dev/ttyS0`
- `/dev/ttyS1`
- `/dev/ttyS3`
- multiple GPIO sysfs `direction` and `value` files
- `/dev/ptmx`
- logs, sockets, pipes, eventfds, and the Elegoo database

The GPIO sysfs handles are likely support/control lines such as reset, power enable, boot pins, or mux selection. They are not likely to be real-time step pulse generation. A normal Linux process writing sysfs GPIO cannot reliably generate 3D-printer stepper timing.

The most important discovery is `/dev/rpmsg1`. This is likely the stock host-to-DSP/control-firmware communication channel.

The system also has:

- `/dev/rpmsg_ctrl-dsp_rproc@0`
- `/sys/class/rpmsg/rpmsg1/name`, which contains `rpmsg1`

This suggests the stock kernel already exposes an RPMsg control device for the DSP remote processor.

## What `/dev/rpmsg1` Likely Is

`/dev/rpmsg1` is probably a Linux RPMsg character endpoint. RPMsg is a Linux framework for passing messages between the main Linux CPU and a remote processor, such as a DSP or firmware-controlled coprocessor.

Likely stack:

```text
elegoo_printer
  -> /dev/rpmsg1
  -> Linux RPMsg char driver
  -> mailbox/shared memory transport
  -> R528 DSP or remote motion firmware
  -> step/dir GPIO + TMC2209 UART
  -> X/Y/Z motors
```

The RPMsg device itself is only a byte/message transport. It does not define motor commands. The command meaning is determined by the firmware and protocol on the other side.

## Stock RPMsg Protocol Observations

Filtered `strace` on fd `26` (`/dev/rpmsg1`) showed periodic binary traffic.

Example passive write/read:

```text
write(26, "\x06\x14\x05\x4a\xb6\x7e", 6)
read(26, "\x0b\x15\x3b\x82\x9f\xb7\xbb\x5d\xe5\x75\x7e\x05\x15\xc9\x2c\x7e", 4096) = 16
```

Example homing-related write/read:

```text
write(26, "\x13\x19\x17\x0d\xa6\x79\x83\x3b\x00\x17\x10\xa6\x79\x83\x3b\x00\xf9\x48\x7e", 19)
read(26, "\x05\x1a\x31\xdb\x7e", 4096) = 5
```

The frame shape appears to be:

```text
[length] [sequence] [opcode/payload...] [checksum? 2 bytes] [0x7e]
```

The first byte matches the observed frame length:

- `05 ... 7e` = 5 bytes
- `06 ... 7e` = 6 bytes
- `0b ... 7e` = 11 bytes
- `0c ... 7e` = 12 bytes
- `0e ... 7e` = 14 bytes
- `0f ... 7e` = 15 bytes
- `12 ... 7e` = 18 bytes
- `13 ... 7e` = 19 bytes

Likely fields:

- byte 0: total frame length
- byte 1: sequence or transaction counter, commonly cycling through `0x10` to `0x1f`
- byte 2: opcode/message type for frames longer than 5 bytes
- bytes before final `0x7e`: likely checksum or CRC
- final byte: `0x7e` terminator

Observed likely opcodes:

- `0x05`: host write, periodic query/heartbeat
- `0x17`: host write, repeated during homing, likely motion/control related
- `0x21`: host write, likely control/response related to `0x42`
- `0x2a`: host write, appears during homing, likely command/action
- `0x3b`: device read, response to `0x05`
- `0x42`: device read, appears during homing/status exchange
- `0x43`: device read, periodic status
- `0x46`: device read, appears after `0x2a`
- `0x48`: device read, periodic sensor/status

This does not look like Klipper's MCU protocol.

## Public Protocol Research

Searches were performed for:

- Elegoo RPMsg protocol documentation
- Centauri Carbon RPMsg references
- exact frame byte fragments
- generic `length sequence opcode crc 0x7e` protocol patterns
- `0x7e` framed embedded protocols

No public documentation or matching implementation was found for this exact protocol.

The frame resembles common vendor embedded framing, but no known standard matched cleanly.

Rejected or unlikely matches:

- HDLC/PPP: uses `0x7e` flags, but normally escapes in-frame `0x7e`; these frames appear length-prefixed and can contain inner `0x7e` bytes.
- XBee API: uses `0x7e` as a start delimiter followed by length; this traffic appears to use length first and `0x7e` as terminator.
- SLIP: uses `0xc0`, not `0x7e`.
- MAVLink, Klipper, Modbus, Marlin serial, Tuya serial: no clear match.

Common CRC16 variants were tested against sample frames using likely body ranges. No matches were found for:

- CRC-16/IBM-ARC
- MODBUS
- X25
- KERMIT
- CCITT-FALSE
- XMODEM
- AUG-CCITT
- DNP
- USB

Possible explanations:

- custom checksum
- CRC input includes bytes not captured or interpreted differently
- trailer is not CRC16
- packet aggregation/splitting in `strace` affected interpretation

Current conclusion: this is probably an Elegoo-private host-to-DSP protocol.

## Klipper MCU Protocol Difference

Klipper's MCU protocol is not G-code and not a high-level command protocol. It is a binary RPC protocol tightly coupled to Klipper MCU firmware.

Klipper expects the controller to support:

- command dictionary handshake
- clock synchronization
- object allocation for steppers, pins, ADCs, PWM, SPI, UART, endstops, etc.
- timestamped and compressed step queues
- deterministic local timing
- config CRC and shutdown/restart semantics

Klipper motion flow:

```text
G-code move
  -> Klippy host planner computes kinematics and step timing
  -> Klippy sends low-level scheduled step commands
  -> MCU firmware toggles step pins at exact MCU clock ticks
```

Stock Elegoo firmware likely uses a different split, probably with high-level commands or private structured messages over RPMsg to firmware that performs motion/control internally.

A Klipper-to-stock-protocol translation layer would need to emulate enough of Klipper's MCU protocol to satisfy Klippy, then map low-level timed step queues into Elegoo's unknown stock protocol. This is likely harder than porting the COSMOS/Klipper firmware path.

## Safety Notes

Do not open or read `/dev/rpmsg1` from a second process while `elegoo_printer` is running unless intentionally experimenting with a disposable setup. A second reader can steal messages and desynchronize the stock control process.

Do not replay captured homing or motion packets blindly. Payloads likely include state, sequence counters, position, velocity, acknowledgements, or timing-sensitive data. Replaying traffic can move axes unexpectedly or confuse firmware state.

Do not attempt to generate step pulses from Linux userspace GPIO sysfs. It is not deterministic enough for printer motion control.

## Useful Commands

Find what `elegoo_printer` has open:

```sh
ls -l /proc/$(pidof elegoo_printer)/fd
readlink -f /proc/$(pidof elegoo_printer)/fd/*
```

Trace only reads/writes/ioctls on the RPMsg fd once the fd number is known:

```sh
strace -f -tt -xx -s 1024 -e trace=read,write,ioctl -p $(pidof elegoo_printer) 2>&1 | grep -E '\(26,'
```

Inspect RPMsg devices:

```sh
ls -l /dev/rpmsg*
ls -l /sys/class/rpmsg
find /sys/class/rpmsg -maxdepth 3 -type f -print
```

Inspect remoteproc and kernel messages:

```sh
ls -l /sys/class/remoteproc 2>/dev/null
dmesg | grep -iE 'rpmsg|remoteproc|rproc|dsp|hifi'
```

## Recommended Next Steps

### If The Goal Is Stock Protocol Understanding

Continue only with passive, labeled captures:

- idle
- enable steppers
- disable steppers
- jog X +1 mm
- jog X -1 mm
- jog Y +1 mm
- jog Z +1 mm
- home X/Y/Z/all
- heat bed
- heat nozzle
- filament load/unload

For each action, capture only `/dev/rpmsg1` traffic and label the exact UI action/time. Then group frames by opcode and compare payload deltas.

This path requires reverse engineering the Elegoo protocol.

### If The Goal Is Moonraker/Klipper

Do not target the stock `/dev/rpmsg1` protocol directly. Moonraker cannot speak it, and Klipper cannot use it unless the remote endpoint speaks Klipper MCU protocol.

The better path is COSMOS-style firmware replacement/porting:

```text
Mainsail/Fluidd
  -> Moonraker
  -> Klippy/Kalico
  -> RPMsg/serial bridge
  -> Klipper MCU firmware on R528 DSP or equivalent motion endpoint
  -> TMC2209 step/dir control
```

Work items:

1. Determine how the stock OS boots or loads DSP/remote firmware.
2. Determine whether COSMOS's CC1 DSP bridge assumptions match CC2's `/dev/rpmsg_ctrl-dsp_rproc@0` path.
3. Identify available stock kernel support devices, especially `/dev/dsp_debug`, `/dev/kbuf-*`, `/dev/rpmsg_ctrl*`, and remoteproc sysfs nodes.
4. Compare CC1 COSMOS mainboard pin config to CC2 hardware/pin mappings.
5. Build a minimal Klipper MCU endpoint for the CC2 motion controller or DSP.
6. Once Klippy can connect to a Klipper MCU, test with safe Klipper commands such as `STEPPER_BUZZ STEPPER=stepper_x`.

### If The Goal Is A Quick Motor Test Without Reverse Engineering

There is no safe generic tool that can drive motors through stock `/dev/rpmsg1` without knowing the protocol.

The least invasive options are:

- use the stock UI/API if Elegoo exposes one
- use Klipper only after replacing the controller firmware endpoint with Klipper MCU firmware
- use external hardware wired to step/dir/enable/TMC UART, which is invasive and requires pin mapping

## Current Bottom Line

The stock CC2 motor-control path is most likely through `/dev/rpmsg1` to private Elegoo firmware on a remote processor. The frame format is partially understood at the transport level, but it does not appear publicly documented and does not match Klipper's MCU protocol.

For Moonraker/Klipper, the most practical long-term route is to port the COSMOS/Klipper firmware approach to CC2 rather than translating Klipper to the unknown stock RPMsg protocol.
