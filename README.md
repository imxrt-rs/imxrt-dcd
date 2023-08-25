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

See below for important details and caveats that affect the interpretation of DCD.


# Usage

```rust
use imxrt_dcd as dcd;
use imxrt_ral as ral;  // feature = "imxrt1062"

// RECOMMENDED: using imxrt-ral and convenience macros
let commands_macro = vec![
  dcd::write_reg!(ral::ccm_analog, CCM_ANALOG, PLL_ARM, @BYPASS, BYPASS_CLK_SRC: CLK1),
  dcd::check_all_clear!(ral::ccm, CCM, CDHIPR, @PERIPH_CLK_SEL_BUSY, @PERIPH2_CLK_SEL_BUSY),
];

// equivalent direct construction
let commands_direct = vec![
  dcd::Command::Write(dcd::Write {
      width: dcd::Width::B4,
      op: dcd::WriteOp::Write,
      address: 0x400D_8000,
      value: 0x0001_4000,
  }),
  dcd::Command::Check(dcd::Check {
    width: dcd::Width::B4,
    cond: dcd::CheckCond::AllClear,
    address: 0x400F_C048,
    mask: (1 << 3) | (1 << 5),
    count: None,
  }),
];

assert_eq!(commands_macro, commands_direct);

// `serialize` into a `std::io::Write`
let mut dcd_bytes = vec![];
let num_bytes_written = dcd::serialize(&mut dcd_bytes, &commands_macro).expect("IO error");
assert_eq!(num_bytes_written, 28);
assert_eq!(
  &dcd_bytes,
  &[
    // DCD header
    0xD2, 0, 28, 0x41,
    // write
    0xCC, 0, 12, 0x04, 0x40, 0x0D, 0x80, 0x00, 0x00, 0x01, 0x40, 0x00,
    // check
    0xCF, 0, 12, 0x04, 0x40, 0x0F, 0xC0, 0x48, 0x00, 0x00, 0x00, 0x28,
  ]
);
```


## Convenience Macros

To simplify the construction of commands, the feature `"ral"` (on by default) provides convenience macros designed to work with register definitions in [`imxrt-ral`][ral].

These macros share a common syntax as follows:
```ignore
macro!(ral::path::to::peripheral, INSTANCE, REGISTER, ...args)
```

Where:

- `macro` can be:
  - Write: [`write_reg`] / [`set_reg`] / [`clear_reg`]
  - Check: [`check_all_clear`] / [`check_any_clear`] / [`check_all_set`] / [`check_any_set`]

- `INSTANCE` should be a pointer-to-register-block, e.g. for `ral::ccm` this should be `CCM`.

- Each `arg` can be:
  - `field: value` => `(value << field::offset) & field::mask`
    - Same behavior as [`ral-registers`][ral-reg].
    - Enumerators / named values of the field can be used directly in the `value` expression.
  - `@field` => `field::mask`
    - Reads as "all (bits of) `field`"
    - Useful for set, clear, and check commands working explicitly with field masks.
  - An arbitrary expression
    - May directly refer to fields of the register (e.g. `(0b110 << field1::offset) | field2::mask`).

All args are then bitwise-OR'd together as the final value / mask of the command.

This syntax is inspired by (and is a superset of) `write_reg!` and friends in [`imxrt-ral`][ral] (re-exporting [`ral-registers`][ral-reg]), adapted for the limitations of DCD.

[ral]: https://crates.io/crates/imxrt-ral/
[ral-reg]: https://crates.io/crates/ral-registers



# DCD Details and Caveats

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
