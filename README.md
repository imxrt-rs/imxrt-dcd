# i.MX RT1060 Device Configuration Data (DCD) Generator

[![Crates.io](https://img.shields.io/crates/v/imxrt-dcd)](https://crates.io/crates/imxrt-dcd)
[![docs.rs](https://img.shields.io/docsrs/imxrt-dcd)](https://docs.rs/imxrt-dcd)

The i.MX RT1050/1060 series of MCUs feature a ROM bootloader. As a part of the boot process, it interprets the Device Configuration Data (DCD) section in the firmware image to perform limited initialization and validation of peripheral registers, e.g. to set up external memory controllers, before any ARM instructions from the firmware image is run.

This crate allows you to generate a DCD binary (byte array) from its semantic description. This is useful e.g. in a `build.rs` script to generate a static variable to be linked to the firmware image. (Shameless plug: See [static-include-bytes](https://crates.io/crates/static-include-bytes).)

# What does the DCD do exactly?

Reference: i.MX RT1060 Reference Manual (rev. 3), §9.7.2 .

The DCD section in the firmware image is a serialized byte array of one or more commands:

- **Write**: Write value (1/2/4-byte) to address.
  - `*address = value` --- direct write
  - `*address &= !value` --- read-modify-write clear bits
  - `*address |= value` --- read-modify-write set bits

- **Check**: Read from address until the value satisfies the condition or too many attempts.
  - `(*address & mask) == 0` --- all clear
  - `(*address & mask) == mask` --- all set
  - `(*address & mask) != mask` --- any clear
  - `(*address & mask) != 0` --- any set

- **NOP**: Ignored --- may behave as a small delay.

## DCD size limit

The DCD serialization format is 4-byte aligned with a 2-byte length field in its header. This allows at most 65532 bytes (all headers included).
However, the boot ROM of a specific chip family may enforce a (much) shorter length limit. For RT1060 this is 1768 bytes.

This crate only enforces the 64 KiB length limit, but does return the size of the serialized DCD so that the user may add a tighter check.

## Write command compression

Multiple consecutive Write commands with the same bit width and operation (i.e. write/clear/set) can be merged (sharing the same command header) to save 4 bytes per extra command.

This crate automatically performs this compression during serialization. This may help meet the DCD size limit.

## Valid Write command address ranges

The boot ROM of a specific chip family may only allow Write commands to a limited number of address ranges.

For example, the following are the valid address ranges for RT1060:

| Begin         | End (inclusive) | Description            |
|---------------|-----------------|------------------------|
| `0x400A_4000` | `0x400A_7FFF`   | IOMUX Control SNVS GPR |
| `0x400A_8000` | `0x400A_BFFF`   | IOMUX Control SNVS     |
| `0x400A_C000` | `0x400A_FFFF`   | IOMUX Control GPR      |
| `0x401F_8000` | `0x401F_BFFF`   | IOMUX Control          |
| `0x400D_8000` | `0x400D_BFFF`   | CCM Analog             |
| `0x400F_C000` | `0x400F_FFFF`   | CCM                    |
| `0x402F_0000` | `0x402F_3FFF`   | SEMC                   |

Writing to anywhere outside these ranges will cause the boot ROM to **immediately abandon interpreting the rest of your DCD**.

This crate does _not_ enforce any address range limitations. The user is expected to provide valid write addresses.

## Check command polling count 

The Check command may specify one of the following:

- Omitted max polling count: ROM will poll indefinitely as long as the condition remains unsatisfied.
- max polling count == 0: Does not poll at all --- equivalent to NOP.
- max polling count > 0: If the max polling count is hit, the boot ROM will **immediately abandon interpreting the rest of your DCD**.

Note that (through my limited experimentation) the boot ROM does _not_ seem to limit the address range of Check commands.


# Toy Example

```rust
use imxrt_dcd::*;

let mut buf = std::io::Cursor::new(vec![]);
let byte_len = serialize(
    &mut buf,
    &[
        Command::Nop,
        Command::Write(Write {
            width: Width::B4,
            op: WriteOp::Write,
            address: 0x01234567,
            value: 0xdeadbeef,
        }),
        Command::Check(Check {
            width: Width::B2,
            cond: CheckCond::AnySet,
            address: 0x89abcdef,
            mask: 0x55aa55aa,
            count: Some(16),
        }),
        Command::Check(Check {
            width: Width::B1,
            cond: CheckCond::AnyClear,
            address: 0x89abcdef,
            mask: 0x55aa55aa,
            count: None,
        }),
    ],
).unwrap();
assert_eq!(byte_len, 48);
assert_eq!(
    &buf.get_ref()[0..48],
    &[
        // DCD header
        0xD2, 0, 48, 0x41,
        // nop
        0xC0, 0x00, 0x04, 0x00,
        // write
        0xCC, 0, 12, 0x04, 0x01, 0x23, 0x45, 0x67, 0xde, 0xad, 0xbe, 0xef,
        // check with count
        0xCF, 0, 16, 0x1a, 0x89, 0xab, 0xcd, 0xef, 0x55, 0xaa, 0x55, 0xaa, 0, 0, 0, 16,
        // check without
        0xCF, 0, 12, 0x09, 0x89, 0xab, 0xcd, 0xef, 0x55, 0xaa, 0x55, 0xaa,
    ]
);
```
