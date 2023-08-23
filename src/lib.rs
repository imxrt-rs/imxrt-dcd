#![doc = include_str!("../README.md")]
use itertools::Itertools;

#[cfg(feature = "ral")]
mod macros;

/// A DCD command.
#[derive(Default, Clone, Debug, Eq, PartialEq)]
pub enum Command {
    /// Dummy command --- may behave as a small delay.
    #[default]
    Nop,
    /// DCD command for writing a value to an address; [`Write`].
    Write(Write),
    /// DCD command for polling an address until the value matches a given bitmask condition; [`Check`].
    Check(Check),
}

/// DCD command for writing a value to an address.
#[derive(Default, Clone, Debug, Eq, PartialEq)]
pub struct Write {
    /// Width of the bus write.
    pub width: Width,
    /// Writing operation --- see [`WriteOp`].
    pub op: WriteOp,
    /// Address to be written to. Note that the ROM may enforce valid address ranges.
    pub address: u32,
    pub value: u32,
}

/// DCD command for polling an address until the value matches a given bitmask condition.
#[derive(Default, Clone, Debug, Eq, PartialEq)]
pub struct Check {
    /// Width of the bus read.
    pub width: Width,
    /// Condition to check --- see [`CheckCond`].
    pub cond: CheckCond,
    /// Address to read from. Unlike [`Write::address`], any address is valid.
    pub address: u32,
    /// Bitmask to check the value against --- see [`CheckCond`].
    pub mask: u32,
    /// Optional poll count:
    /// - `None` => poll indefinitely
    /// - `Some(0)` => equivalent to [`Command::Nop`]
    /// - `Some(x) if x > 0` => poll at most `x` times; if the condition still is not satisfied,
    ///   the boot ROM will abandon interpreting the rest of the DCD.
    pub count: Option<u32>,
}

/// Byte width of the bus read/write.
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum Width {
    /// 1 byte / 8 bit
    B1 = 0b001u8,
    /// 2 bytes / 16 bit
    B2 = 0b010u8,
    /// 4 bytes / 32 bit
    #[default]
    B4 = 0b100u8,
}

/// Write operation variant.
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum WriteOp {
    /// `*address = value` --- direct write
    #[default]
    Write = 0b00_000u8,
    /// `*address &= !value` --- clear bits (read-modify-write)
    Clear = 0b01_000u8,
    /// `*address |= value` --- set bits (read-modify-write)
    Set = 0b11_000u8,
}

/// Check condition variant.
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum CheckCond {
    #[default]
    /// `(*address & mask) == 0` --- All masked bits are 0 in value
    AllClear = 0b00_000u8,
    /// `(*address & mask) != mask` --- Some masked bits are 0 in value
    AnyClear = 0b01_000u8,
    /// `(*address & mask) == mask` --- All masked bits are 1 in value
    AllSet = 0b10_000u8,
    /// `(*address & mask) != 0` --- Some masked bits are 1 in value
    AnySet = 0b11_000u8,
}

///////////////////////////////////////////////////////////////////////////

fn dcd_header(byte_len: u16) -> [u8; 4] {
    let mut header = [0xD2, 0x00, 0x00, 0x41];
    header[1..=2].copy_from_slice(&byte_len.to_be_bytes()[0..=1]);
    header
}

const NOP_HEADER: [u8; 4] = [0xC0, 0x00, 0x04, 0x00];

impl Write {
    fn byte_len(group_size: usize) -> u16 {
        let n = 4 + group_size * 8;
        assert!(n <= u16::MAX as usize);
        n as u16
    }
    fn header(&self, group_size: usize) -> [u8; 4] {
        let mut header = [0xCC, 0x00, 0x00, self.width as u8 | self.op as u8];
        header[1..=2].copy_from_slice(&Self::byte_len(group_size).to_be_bytes()[0..=1]);
        header
    }
    fn payload(&self) -> [u8; 8] {
        let mut payload = [0u8; 8];
        payload[0..4].copy_from_slice(&self.address.to_be_bytes()[0..4]);
        payload[4..8].copy_from_slice(&self.value.to_be_bytes()[0..4]);
        payload
    }
}

impl Check {
    fn byte_len(&self) -> u16 {
        if self.count.is_some() {
            16
        } else {
            12
        }
    }
    fn header(&self) -> [u8; 4] {
        let mut header = [0xCF, 0x00, 0x00, self.width as u8 | self.cond as u8];
        header[1..=2].copy_from_slice(&self.byte_len().to_be_bytes()[0..=1]);
        header
    }
    fn payload(&self) -> [u8; 8] {
        let mut payload = [0u8; 8];
        payload[0..4].copy_from_slice(&self.address.to_be_bytes()[0..4]);
        payload[4..8].copy_from_slice(&self.mask.to_be_bytes()[0..4]);
        payload
    }
    fn payload_with_count(&self) -> [u8; 12] {
        let mut payload = [0u8; 12];
        payload[0..4].copy_from_slice(&self.address.to_be_bytes()[0..4]);
        payload[4..8].copy_from_slice(&self.mask.to_be_bytes()[0..4]);
        payload[8..12].copy_from_slice(&self.count.unwrap().to_be_bytes()[0..4]);
        payload
    }
}

fn group_key(index: usize, command: &Command) -> (usize, Width, WriteOp) {
    match command {
        &Command::Write(Write {
            width, op, ..
        }) => (usize::MAX, width, op),
        _ => (index, Width::default(), WriteOp::default()),
    }
}

///////////////////////////////////////////////////////////////////////////

/// Serializes given commands as a complete DCD block into a byte stream.
/// Consecutive write commands with the same width and op are automatically combined.
///
/// While the ROM may enforce tighter byte size limits, this
///
/// Returns the number of bytes written or error.
///
/// # Examples
///
/// See [crate-level doc](crate).
///
pub fn serialize(mut w: impl std::io::Write, commands: &[Command]) -> std::io::Result<usize> {
    if commands.is_empty() {
        return Ok(0);
    }
    // count num of bytes first
    let mut byte_len: usize = 4; // DCD header
    for (_, mut group) in &commands
        .into_iter()
        .enumerate()
        .group_by(|&(index, command)| group_key(index, command))
    {
        let Some((_, head)) = group.next() else { continue; };
        match head {
            Command::Nop => {
                byte_len += 4;
            }
            Command::Check(check) => {
                byte_len += check.byte_len() as usize;
            }
            Command::Write(_) => {
                byte_len += Write::byte_len(group.count() + 1) as usize;
            }
        }
    }
    if byte_len > u16::MAX as usize {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "DCD byte length too large",
        ));
    }
    w.write_all(&dcd_header(byte_len as u16))?;
    for (_, mut group) in &commands
        .into_iter()
        .enumerate()
        .group_by(|&(index, command)| group_key(index, command))
    {
        let Some((_, head)) = group.next() else { continue; };
        match head {
            Command::Nop => {
                w.write_all(&NOP_HEADER)?;
            }
            Command::Check(check) => {
                w.write_all(&check.header())?;
                if check.count.is_some() {
                    w.write_all(&check.payload_with_count())?;
                } else {
                    w.write_all(&check.payload())?;
                }
            }
            Command::Write(write) => {
                let (counter, rest) = group.tee();
                w.write_all(&write.header(counter.count() + 1))?;
                w.write_all(&write.payload())?;
                for (_, command) in rest {
                    if let Command::Write(write) = command {
                        w.write_all(&write.payload())?;
                    }
                }
            }
        }
    }
    Ok(byte_len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[rustfmt::skip]
    fn serialize_simple() {
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
    }

    #[test]
    #[rustfmt::skip]
    fn serialize_merge() {
        let mut buf = std::io::Cursor::new(vec![0u8; 1024]);
        let byte_len = serialize(
            &mut buf,
            &[
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
                Command::Nop,
                // this is not merged because of the NOP in the middle
                Command::Write(Write {
                    width: Width::B4,
                    op: WriteOp::Write,
                    address: 0x89abcdef,
                    value: 0x13370000,
                }),
                // this is not merged with the previous because they differ in width
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
            ],
        ).expect("IO failure");
        assert_eq!(byte_len, 72);
        assert_eq!(
            &buf.get_ref()[0..72],
            &[
                // DCD header
                0xD2, 0, 72, 0x41,
                // write header
                0xCC, 0, 28, 0x04,
                // write
                0x01, 0x23, 0x45, 0x67, 0xde, 0xad, 0xbe, 0xef,
                // write
                0x89, 0xab, 0xcd, 0xef, 0x13, 0x37, 0x00, 0x00,
                // write
                0x55, 0xaa, 0x55, 0xaa, 0xaa, 0x55, 0xaa, 0x55,
                // nop
                0xC0, 0x00, 0x04, 0x00,
                // write header
                0xCC, 0, 12, 0x04,
                // write
                0x89, 0xab, 0xcd, 0xef, 0x13, 0x37, 0x00, 0x00,
                // write header
                0xCC, 0, 12, 0x02,
                // write
                0x89, 0xab, 0xcd, 0xef, 0x13, 0x37, 0x00, 0x00,
                // write header
                0xCC, 0, 12, 0x04,
                // write
                0x55, 0xaa, 0x55, 0xaa, 0xaa, 0x55, 0xaa, 0x55,
            ]
        );
    }
}
