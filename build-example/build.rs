use std::path::Path;

use imxrt_dcd::*;

fn main() -> std::io::Result<()> {
    let out_dir = std::env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("dcd.bin");

    let commands = [
        // the following 3 writes should be merged
        Command::Write(Write {
            width: Width::B4,
            op: WriteOp::Write,
            address: 0x01234567,
            value: 0xdeadbeef,
        }),
        Command::Write(Write {
            width: Width::B4,
            op: WriteOp::Write,
            address: 0x89abcdef,
            value: 0x13370000,
        }),
        Command::Write(Write {
            width: Width::B4,
            op: WriteOp::Write,
            address: 0x55aa55aa,
            value: 0xaa55aa55,
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
        Command::Write(Write {
            width: Width::B4,
            op: WriteOp::Write,
            address: 0x89abcdef,
            value: 0x13370000,
        }),
        Command::Write(Write {
            width: Width::B2,
            op: WriteOp::Write,
            address: 0x89abcdef,
            value: 0x13370000,
        }),
        // this is not merged (ditto)
        Command::Write(Write {
            width: Width::B4,
            op: WriteOp::Write,
            address: 0x55aa55aa,
            value: 0xaa55aa55,
        }),
    ];

    let mut file = std::io::BufWriter::new(std::fs::File::create(dest_path)?);
    serialize(&mut file, &commands)?;

    Ok(())
}
