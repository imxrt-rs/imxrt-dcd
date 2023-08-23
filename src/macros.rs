//! Macros for conveniently creating [DCD commands][] with [`imxrt-ral`][ral] definitions.
//!
//! Note that these macros do not depend on [`imxrt-ral`][ral] directly, but are expected to work
//! with the implicit structure of the RAL.
//!
//! All macros here share the same syntax, which is an extension of e.g. [`imxrt_ral::write_reg`]:
//! ```ignore
//! macro!(ral::path::to::peripheral, INSTANCE, REGISTER, ...args)
//! ```
//!
//! Where:
//!
//! - `macro` can be:
//!   - Write: [`write_reg`] / [`set_reg`] / [`clear_reg`]
//!   - Check: [`check_all_clear`] / [`check_any_clear`] / [`check_all_set`] / [`check_any_set`]
//!
//! - each `arg` can be:
//!   - `field: value` => `(value << field::offset) & field::mask`
//!   - `@field` => `field::mask` (reads as "all (bits of) `field`")
//!   - arbitrary expression => itself
//!
//! All args are then bitwise-OR'd together as the final value / mask of the command.
//!
//! # Example
//!
//! ```
//! use imxrt_dcd as dcd;
//! use imxrt_ral as ral;  // feature = "imxrt1062"
//!
//! assert_eq!(
//!     dcd::write_reg!(
//!         ral::ccm_analog, CCM_ANALOG, PLL_ARM, @BYPASS, BYPASS_CLK_SRC: CLK1),
//!     dcd::Command::Write(dcd::Write {
//!         width: dcd::Width::B4,
//!         op: dcd::WriteOp::Write,
//!         address: 0x400D_8000,
//!         value: 0x0001_4000,
//!     }),
//! );
//! ```
//!
//! [ral]: https://crates.io/crates/imxrt-ral/
//! [DCD commands]: crate::Command


/// Creates a DCD command that (over-)writes to the specified RAL register,
/// i.e. `register = arg1 | arg2 | ...` .
///
/// Syntax:
/// ```ignore
/// write_reg!(ral::path::to::peripheral, INSTANCE, REGISTER, ...args)
/// ```
/// See [macro overview](self) for details on `args`.
///
/// Returns a [`crate::Command::Write`] with [`crate::WriteOp::Write`].
///
/// # Example
///
/// ```
/// # use imxrt_dcd as dcd;
/// # use imxrt_ral as ral;
/// # _ =
/// dcd::write_reg!(
///     ral::ccm_analog, CCM_ANALOG, PLL_ARM, @BYPASS, BYPASS_CLK_SRC: CLK1)
/// # ;
/// ```
#[macro_export]
macro_rules! write_reg {

    // Internal rule for recursively parsing value / mask arguments.
    // This is shared between all macros, hosted here to avoid exporting private macros.
    //
    // - `access`: e.g. `{W::*, RW::*}` (for importing the correct field value enumerators)

    // `field: value`
    ( @build_value $access:tt $field:ident : $value:expr $(, $($rest:tt)*)?) => {{
        #[allow(unused_imports)]
        use reg::$field::$access;
        (($value << reg::$field::offset ) & reg::$field::mask)
        $(
            | $crate::write_reg!(@build_value $access $($rest)*)
        )?
    }};

    // `@field`
    ( @build_value $access:tt @ $field:ident $(, $($rest:tt)*)?) => {{
        reg::$field::mask
        $(
            | $crate::write_reg!(@build_value $access $($rest)*)
        )?
    }};

    // arbitrary expression
    ( @build_value $access:tt $expr:expr $(, $($rest:tt)*)?) => {{
        #[allow(unused_imports)]
        use reg::*;
        $expr
        $(
            | $crate::write_reg!(@build_value $access $($rest)*)
        )?
    }};

    // Internal rule for making a generic Write command.
    // This is shared between all Write macros, hosted here to avoid exporting private macros.
    //
    // - `address` is computed from the RAL instance-register pair.
    // - `value` is provided --- the expression can refer to `periph` and `reg` aliases.

    ( @make_command $op:ident, $width:ident, $periph:path, $instance:ident, $reg:ident $([$offset:expr])*, $value:expr ) => {{
        #[allow(unused_imports)]
        use $periph as periph;
        #[allow(unused_imports)]
        use $periph::{$reg as reg};

        $crate::Command::Write($crate::Write {
            width: $crate::Width::$width,
            op: $crate::WriteOp::$op,
            address: unsafe {
                ::core::ptr::addr_of!((*(periph::$instance)).$reg $([$offset])*) as u32
            },
            value: $value,
        })
    }};

    // Main macro API. Wraps around @make_command and @build_value.

    ( $periph:path, $instance:ident, $reg:ident $([$offset:expr])*, $($args:tt)+) => {{
        $crate::write_reg!(@make_command
            Write, B4, $periph, $instance, $reg $([$offset])*,
            $crate::write_reg!(@build_value {W::*, RW::*} $($args)+)
        )
    }};
}

/// Creates a DCD command that sets specified bits / fields to 1 in the specified RAL register,
/// i.e. `register |= arg1 | arg2 | ...` .
///
/// Syntax:
/// ```ignore
/// write_reg!(ral::path::to::peripheral, INSTANCE, REGISTER, ...args)
/// ```
/// See [macro overview](self) for details on `args`.
///
/// Returns a [`crate::Command::Write`] with [`crate::WriteOp::Set`].
///
/// # Example
///
/// ```
/// # use imxrt_dcd as dcd;
/// # use imxrt_ral as ral;
/// # _ =
/// dcd::set_reg!(ral::ccm_analog, CCM_ANALOG, PLL_ARM, @ENABLE)
/// # ;
/// ```
#[macro_export]
macro_rules! set_reg {
    ( $periph:path, $instance:ident, $reg:ident $([$offset:expr])*, $($args:tt)+) => {{
        $crate::write_reg!(@make_command
            Set, B4, $periph, $instance, $reg $([$offset])*,
            $crate::write_reg!(@build_value {W::*, RW::*} $($args)+)
        )
    }};
}

/// Creates a DCD command that clears specified bits / fields to 0 in the specified RAL register,
/// i.e. `register &= !(arg1 | arg2 | ...)` .
///
/// Syntax:
/// ```ignore
/// write_reg!(ral::path::to::peripheral, INSTANCE, REGISTER, ...args)
/// ```
/// See [macro overview](self) for details on `args`.
///
/// Returns a [`crate::Command::Write`] with [`crate::WriteOp::Clear`].
///
/// # Example
///
/// ```
/// # use imxrt_dcd as dcd;
/// # use imxrt_ral as ral;
/// # _ =
/// dcd::clear_reg!(ral::ccm, CCM, CBCMR, @PERIPH_CLK2_SEL)
/// # ;
/// ```
#[macro_export]
macro_rules! clear_reg {
    ( $periph:path, $instance:ident, $reg:ident $([$offset:expr])*, $($args:tt)+) => {{
        $crate::write_reg!(@make_command
            Clear, B4, $periph, $instance, $reg $([$offset])*,
            $crate::write_reg!(@build_value {W::*, RW::*} $($args)+)
        )
    }};
}

/// Creates a DCD command that polls (indefinitely) to check if all specified bits / fields are 0
/// in the specified RAL register, i.e. `(register & (arg1 | arg2 | ...)) == 0` .
///
/// Syntax:
/// ```ignore
/// check_all_clear!(ral::path::to::peripheral, INSTANCE, REGISTER, ...args)
/// ```
/// See [macro overview](self) for details on `args`.
///
/// Returns a [`crate::Command::Check`] with [`crate::CheckCond::AllClear`].
///
/// # Example
///
/// ```
/// # use imxrt_dcd as dcd;
/// # use imxrt_ral as ral;
/// # _ =
/// dcd::check_all_clear!(ral::ccm, CCM, CDHIPR, @PERIPH_CLK_SEL_BUSY, @PERIPH2_CLK_SEL_BUSY)
/// # ;
/// ```
///
#[macro_export]
macro_rules! check_all_clear {

    // Internal rule for making a generic Check command.
    // This is shared between all Check macros, hosted here to avoid exporting private macros.
    //
    // - `address` is computed from the RAL instance-register pair.
    // - `mask` is provided --- the expression can refer to `periph` and `reg` aliases.
    // - `count` is provided.

    ( @make_command $cond:ident, $width:ident, $count:expr, $periph:path, $instance:ident, $reg:ident $([$offset:expr])*, $mask:expr) => {{
        use $periph as periph;
        #[allow(unused_imports)]
        use periph::$reg as reg;
        $crate::Command::Check($crate::Check {
            width: $crate::Width::$width,
            cond: $crate::CheckCond::$cond,
            address: unsafe {
                ::core::ptr::addr_of!((*(periph::$instance)).$reg $([$offset])*) as u32
            },
            mask: $mask,
            count: $count,
        })
    }};

    ( $periph:path, $instance:ident, $reg:ident $([$offset:expr])*, $($args:tt)+) => {{
        $crate::check_all_clear!(@make_command
            AllClear, B4, None, $periph, $instance, $reg $([$offset])*,
            $crate::write_reg!(@build_value {R::*, RW::*} $($args)+)
        )
    }};
}

/// Creates a DCD command that polls (indefinitely) to check if any specified bits / fields are 0
/// in the specified RAL register, i.e. `(register & (arg1 | arg2 | ...)) != (arg1 | arg2 | ...)` .
///
/// Syntax:
/// ```ignore
/// check_any_clear!(ral::path::to::peripheral, INSTANCE, REGISTER, ...args)
/// ```
/// See [macro overview](self) for details on `args`.
///
/// Returns a [`crate::Command::Check`] with [`crate::CheckCond::AnyClear`].
///
/// # Example
///
/// ```
/// # use imxrt_dcd as dcd;
/// # use imxrt_ral as ral;
/// # _ =
/// dcd::check_any_clear!(ral::ccm, CCM, CDHIPR, @PERIPH_CLK_SEL_BUSY, @PERIPH2_CLK_SEL_BUSY)
/// # ;
/// ```
///
#[macro_export]
macro_rules! check_any_clear {
    ( $periph:path, $instance:ident, $reg:ident $([$offset:expr])*, $($args:tt)+) => {{
        $crate::check_all_clear!(@make_command
            AnyClear, B4, None, $periph, $instance, $reg $([$offset])*,
            $crate::write_reg!(@build_value {R::*, RW::*} $($args)+)
        )
    }};
}

/// Creates a DCD command that polls (indefinitely) to check if all specified bits / fields are 1
/// in the specified RAL register, i.e. `(register & (arg1 | arg2 | ...)) == (arg1 | arg2 | ...)` .
///
/// Syntax:
/// ```ignore
/// check_all_set!(ral::path::to::peripheral, INSTANCE, REGISTER, ...args)
/// ```
/// See [macro overview](self) for details on `args`.
///
/// Returns a [`crate::Command::Check`] with [`crate::CheckCond::AllSet`].
///
/// # Example
///
/// ```
/// # use imxrt_dcd as dcd;
/// # use imxrt_ral as ral;
/// # _ =
/// dcd::check_all_set!(ral::ccm_analog, CCM_ANALOG, PLL_ARM, @LOCK)
/// # ;
/// ```
///
#[macro_export]
macro_rules! check_all_set {
    ( $periph:path, $instance:ident, $reg:ident $([$offset:expr])*, $($args:tt)+) => {{
        $crate::check_all_clear!(@make_command
            AllSet, B4, None, $periph, $instance, $reg $([$offset])*,
            $crate::write_reg!(@build_value {R::*, RW::*} $($args)+)
        )
    }};
}

/// Creates a DCD command that polls (indefinitely) to check if any specified bits / fields are 1
/// in the specified RAL register, i.e. `(register & (arg1 | arg2 | ...)) != 0` .
///
/// Syntax:
/// ```ignore
/// check_any_set!(ral::path::to::peripheral, INSTANCE, REGISTER, ...args)
/// ```
/// See [macro overview](self) for details on `args`.
///
/// Returns a [`crate::Command::Check`] with [`crate::CheckCond::AnySet`].
///
/// # Example
///
/// ```
/// # use imxrt_dcd as dcd;
/// # use imxrt_ral as ral;
/// # _ =
/// dcd::check_any_set!(ral::iomuxc, IOMUXC, SW_PAD_CTL_PAD_GPIO_B0_03, @DSE)
/// # ;
/// ```
///
#[macro_export]
macro_rules! check_any_set {
    ( $periph:path, $instance:ident, $reg:ident $([$offset:expr])*, $($args:tt)+) => {{
        $crate::check_all_clear!(@make_command
            AnySet, B4, None, $periph, $instance, $reg $([$offset])*,
            $crate::write_reg!(@build_value {R::*, RW::*} $($args)+)
        )
    }};
}

#[cfg(test)]
mod tests {
    use crate as dcd;
    use imxrt_ral as ral;  // feature = "imxrt1062"

    #[test]
    fn field_mask_shorthand() {
        assert_eq!(
            dcd::write_reg!(
                ral::ccm_analog, CCM_ANALOG, PLL_ARM, @BYPASS, BYPASS_CLK_SRC: CLK1),
            dcd::write_reg!(
                ral::ccm_analog, CCM_ANALOG, PLL_ARM, BYPASS::mask | (0b01 << 14)),
        );
    }

    #[test]
    fn write_example() {
        // Here we exercise the argument-parsing / value-building logic. Since it's shared between
        // all macros, we don't need to test it in every variant.
        assert_eq!(
            dcd::write_reg!(
                ral::ccm_analog, CCM_ANALOG, PLL_ARM, @BYPASS, BYPASS_CLK_SRC: CLK1),
            dcd::Command::Write(dcd::Write {
                width: dcd::Width::B4,
                op: dcd::WriteOp::Write,
                address: 0x400D_8000,
                value: 0x0001_4000,
            }),
        );
    }

    #[test]
    fn set_example() {
        assert_eq!(
            dcd::set_reg!(ral::ccm_analog, CCM_ANALOG, PLL_ARM, @ENABLE),
            dcd::Command::Write(dcd::Write {
                width: dcd::Width::B4,
                op: dcd::WriteOp::Set,
                address: 0x400D_8000,
                value: 1 << 13,
            }),
        );
    }

    #[test]
    fn clear_example() {
        assert_eq!(
            dcd::clear_reg!(ral::ccm, CCM, CBCMR, @PERIPH_CLK2_SEL),
            dcd::Command::Write(dcd::Write {
                width: dcd::Width::B4,
                op: dcd::WriteOp::Clear,
                address: 0x400F_C018,
                value: 0b11 << 12,
            }),
        );
    }

    #[test]
    fn check_all_clear_example() {
        assert_eq!(
            dcd::check_all_clear!(ral::ccm, CCM, CDHIPR, @PERIPH_CLK_SEL_BUSY, @PERIPH2_CLK_SEL_BUSY),
            dcd::Command::Check(dcd::Check {
                width: dcd::Width::B4,
                cond: dcd::CheckCond::AllClear,
                address: 0x400F_C048,
                mask: (1 << 3) | (1 << 5),
                count: None,
            }),
        )
    }

    #[test]
    fn check_any_clear_example() {
        assert_eq!(
            dcd::check_any_clear!(ral::ccm, CCM, CDHIPR, @PERIPH_CLK_SEL_BUSY, @PERIPH2_CLK_SEL_BUSY),
            dcd::Command::Check(dcd::Check {
                width: dcd::Width::B4,
                cond: dcd::CheckCond::AnyClear,
                address: 0x400F_C048,
                mask: (1 << 3) | (1 << 5),
                count: None,
            }),
        )
    }

    #[test]
    fn check_all_set_example() {
        assert_eq!(
            dcd::check_all_set!(ral::ccm_analog, CCM_ANALOG, PLL_ARM, @LOCK),
            dcd::Command::Check(dcd::Check {
                width: dcd::Width::B4,
                cond: dcd::CheckCond::AllSet,
                address: 0x400D_8000,
                mask: 1 << 31,
                count: None,
            }),
        )
    }

    #[test]
    fn check_any_set_example() {
        assert_eq!(
            dcd::check_any_set!(ral::iomuxc, IOMUXC, SW_PAD_CTL_PAD_GPIO_B0_03, @DSE),
            dcd::Command::Check(dcd::Check {
                width: dcd::Width::B4,
                cond: dcd::CheckCond::AnySet,
                address: 0x401F_8338,
                mask: 0b111 << 3,
                count: None,
            }),
        )
    }
}
