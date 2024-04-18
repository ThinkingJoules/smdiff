

pub const OP_MASK: u8 = 0b11000000;
pub const COPY_D: u8 = 0b00000000;
pub const COPY_O: u8 = 0b01000000;
pub const ADD: u8 = 0b10000000;
pub const RUN: u8 = 0b11000000;
pub const SIZE_MASK: u8 = 0b00111111;
pub const MAX_RUN_LEN:u8 = 62;
pub const MAX_INST_SIZE:usize = u16::MAX as usize;

#[derive(Copy,Clone,Debug, PartialEq)]
pub enum Format {
    MicroFormat{num_operations: u8},
    WindowFormat,
}

#[derive(Copy,Clone,Debug, PartialEq)]
pub struct FileHeader {
    pub compression_algo: u8,
    pub format: Format,
}

#[derive(Copy,Clone,Debug, PartialEq)]
pub struct WindowHeader {
    pub num_operations: u16,
    pub output_size: u32,
}


#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Add{
    pub bytes: Vec<u8>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run{
    pub byte: u8,
    pub len: u8,
}
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Copy{
    pub src: CopySrc,
    pub addr: u64,
    pub len: u16,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CopySrc{
    Dict,
    Output,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op{
    Run(Run),
    Copy(Copy),
    Add(Add),
}

impl Op {
    pub fn bit_flag(&self) -> u8 {
        match self {
            Op::Run(_) => RUN,
            Op::Copy(copy) => match copy.src {
                CopySrc::Dict => COPY_D,
                CopySrc::Output => COPY_O,
            },
            Op::Add(_) => ADD,
        }
    }
    pub fn is_add(&self) -> bool {
        matches!(self, Op::Add(_))
    }
    pub fn oal(&self) -> u16 {
        match &self {
            Op::Add(add) => add.bytes.len() as u16,
            Op::Copy(copy) => copy.len,
            Op::Run(run) => run.len as u16,
        }
    }
}

/// Used to determine how the size should be handled
pub enum Size{
    Done(u8),
    U8And62,
    U16
}
pub fn size_routine(size: u16)->Size{
    match size {
        0..=62 => Size::Done(size as u8),
        63 => Size::U8And62,
        _ => Size::U16,
    }
}

pub fn zigzag_encode(n: i64) -> u64 {
    ((n << 1) ^ (n >> 63)) as u64
}

pub fn zigzag_decode(z: u64) -> i64 {
    ((z >> 1) as i64) ^ -((z & 1) as i64)
}

pub fn write_u_varint<W: std::io::Write>(writer: &mut W, mut n: u64) -> std::io::Result<()> {
    let mut buf = [0u8; 10]; // Max length for u-varint encoding of u64
    let mut i = 0;
    while n >= 0x80 {
        buf[i] = ((n & 0x7F) | 0x80) as u8;
        n >>= 7;
        i += 1;
    }
    buf[i] = n as u8;
    writer.write_all(&buf[..=i])
}
pub fn read_u_varint<R: std::io::Read>(reader: &mut R) -> std::io::Result<u64> {
    let mut result = 0u64;
    let mut shift = 0;
    let mut b = [0u8; 1];
    loop {
        reader.read_exact(&mut b)?;
        let byte = b[0];
        result |= ((byte & 0x7F) as u64) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
    }
    Ok(result)
}




pub fn write_i_varint<W: std::io::Write>(writer: &mut W, n: i64) -> std::io::Result<()> {
    write_u_varint(writer, zigzag_encode(n as i64))
}

pub fn read_i_varint<R: std::io::Read>(reader: &mut R) -> std::io::Result<i64> {
    Ok(zigzag_decode(read_u_varint(reader)?))
}

pub fn read_u8<R: std::io::Read>(reader: &mut R) -> std::io::Result<u8> {
    let mut buf = [0u8; 1];
    reader.read_exact(&mut buf)?;
    Ok(buf[0])
}
pub fn write_u8<W: std::io::Write>(writer: &mut W, n: u8) -> std::io::Result<()> {
    writer.write_all(&[n])
}
pub fn read_u16<R: std::io::Read>(reader: &mut R) -> std::io::Result<u16> {
    let mut buf = [0u8; 2];
    reader.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}
pub fn write_u16<W: std::io::Write>(writer: &mut W, n: u16) -> std::io::Result<()> {
    writer.write_all(&n.to_le_bytes())
}

pub fn u_varint_encode_size(n: u64) -> usize {
    let mut size = 1;
    let mut n = n >> 7;
    while n > 0 {
        size += 1;
        n >>= 7;
    }
    size
}

pub fn diff_addresses_to_u64(cur: u64, input: i64) -> u64 {
    if input >= 0 {
        // Safe to convert i64 to u64 because i is non-negative
        cur.wrapping_add(input as u64)
    } else {
        // i is negative, so convert the absolute value to u64 and subtract
        let positive_i = input.abs() as u64; // This is safe because `abs()` of a negative i64 can always fit in u64
        cur.wrapping_sub(positive_i)
    }
}

pub fn diff_addresses_to_i64(cur: u64, target: u64) -> i64 {
    if target > cur {
        (target - cur) as i64
    } else {
        (cur - target) as i64 * -1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_ivarint() {
        let mut buffer = Vec::new();
        let values = [0, -1, 1, -2, 2, 1024, -1025, i64::MAX, i64::MIN];
        for &val in &values {
            buffer.clear();
            write_i_varint(&mut buffer, val).unwrap();
            let mut cursor = Cursor::new(&buffer);
            let decoded = read_i_varint(&mut cursor).unwrap();
            assert_eq!(val, decoded, "Failed encoding/decoding {}", val);
        }
    }
    #[test]
    fn test_add_i64_to_u64() {
        // Positive i64 addition
        assert_eq!(diff_addresses_to_u64(10, 5), 15);
        assert_eq!(diff_addresses_to_u64(0, i64::MAX), i64::MAX as u64);

        // Negative i64 addition
        assert_eq!(diff_addresses_to_u64(20, -5), 15);
        assert_eq!(diff_addresses_to_u64(10, -15), 0xFFFFFFFFFFFFFFFB); // Check wrapping behavior

        // Edge cases
        assert_eq!(diff_addresses_to_u64(0, -1), 0xFFFFFFFFFFFFFFFF);
        assert_eq!(diff_addresses_to_u64(u64::MAX, 1), 0); // Check overflow
    }

    #[test]
    fn test_u64_to_i64_diff() {
        // Positive difference
        assert_eq!(diff_addresses_to_i64(5, 10), 5);

        // Negative difference
        assert_eq!(diff_addresses_to_i64(10, 5), -5);

        // Zero difference
        assert_eq!(diff_addresses_to_i64(20, 20), 0);
    }
}
