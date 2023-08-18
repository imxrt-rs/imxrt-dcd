# i.MX RT1060 Device Configuration Data (DCD) Generator

[![Crates.io](https://img.shields.io/crates/v/imxrt-dcd)](https://crates.io/crates/imxrt-dcd)
[![docs.rs](https://img.shields.io/docsrs/imxrt-dcd)](https://docs.rs/imxrt-dcd)

The i.MX RT1050/1060 series of MCUs feature a ROM bootloader. As a part of the boot process, it reads a section in the firmware image to perform simple initialization of peripheral registers, e.g. to set up external memory controllers. This section is the Device Configuration Data (DCD). 

This crate allows you to generate a DCD binary (byte array) from its semantic description. This is useful e.g. in a `build.rs` script to generate a static variable to be linked to the firmware image.

# What does DCD do?

Reference: i.MX RT1060 Reference Manual, §9.7.2 .

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

- **NOP**: Ignored (might act as a delay?)

Multiple write commands with the same bit width and operation (i.e. write/clear/set) can be merged (sharing the same command header) to save some bytes. This might be helpful as there is a hardcoded byte length limit in the ROM (1768 bytes for RT1060, including headers). This crate automatically performs this compression but does not enforce any byte size limit.

# Usage

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
)
.expect("IO failure");
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
