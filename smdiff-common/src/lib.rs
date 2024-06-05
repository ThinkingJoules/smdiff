
/// Bits for the operation type
pub const OP_MASK: u8 = 0b11000000;
/// Copy from dictionary bit flag value
pub const COPY_D: u8 = 0b00000000;
/// Copy from output bit flag value
pub const COPY_O: u8 = 0b01000000;
/// Add bit flag value
pub const ADD: u8 = 0b10000000;
/// Run bit flag value
pub const RUN: u8 = 0b11000000;
/// Bits for the Size Value
pub const SIZE_MASK: u8 = 0b00111111;
/// Inclusive Upper Bound
pub const MAX_RUN_LEN:u8 = 62;
/// Inclusive Upper Bound
pub const MAX_INST_SIZE:usize = u16::MAX as usize;
/// Inclusive Upper Bound
pub const MAX_WIN_SIZE:usize = (1<<24) - 1; // 16MB

/// Format of the how a section is laid out.
/// * Interleaved: The Add bytes follow each Add Opertion
/// * Segregated: All Add bytes are at the end of the section
/// default: Interleaved
#[derive(Copy,Clone,Debug, PartialEq, Eq)]
pub enum Format {
    /// The Add bytes follow each Add Opertion
    Interleaved,
    /// All Add bytes are at the end of the section
    Segregated,
}
impl Default for Format {
    fn default() -> Self {
        Format::Interleaved
    }
}

impl Format {
    pub fn is_interleaved(&self) -> bool {
        matches!(self, Format::Interleaved{..})
    }
    pub fn is_segregated(&self) -> bool {
        matches!(self, Format::Segregated)
    }
}
/// Header struct for a section
#[derive(Copy,Clone,Debug,Default, PartialEq)]
pub struct SectionHeader {
    /// Should be a value between 0-7 per the spec
    pub compression_algo: u8,
    pub format: Format,
    /// true if there are more sections to decode after this one.
    /// false if this is the last section
    /// default: false
    pub more_sections: bool,
    /// Total number of operations in the section
    pub num_operations: u32,
    /// Total Add bytes at the end of the section (not needed for interleaved format)
    pub num_add_bytes: u32,
    /// Total output size generated from the operations in this section
    /// Maximum value should not exceed (1<<24) - 1
    pub output_size: u32,
}

impl SectionHeader {
    /// Create a new SectionHeader with the given parameters
    /// * num_operations: Total number of operations in the section
    /// * num_add_bytes: Total Add bytes at the end of the section (not needed for interleaved format)
    /// * output_size: Total output size generated from the operations in this section
    pub fn new(num_operations: u32, num_add_bytes: u32, output_size: u32) -> Self {
        Self { num_operations, num_add_bytes, output_size, ..Default::default() }
    }
    /// Set the compression algorithm to use for this section
    /// * compression_algo: Should be a value between 0-7 per the spec
    pub fn set_compression_algo(mut self, compression_algo: u8) -> Self {
        self.compression_algo = compression_algo.clamp(0, 7);
        self
    }
    pub fn set_format(mut self, format: Format) -> Self {
        self.format = format;
        self
    }
    /// Indicate if this section is *not* the last section
    pub fn set_more_sections(mut self, more_sections: bool) -> Self {
        self.more_sections = more_sections;
        self
    }

    pub fn is_compressed(&self) -> bool {
        self.compression_algo != 0
    }
    pub fn is_interleaved(&self) -> bool {
        self.format.is_interleaved()
    }
}

/// Trait for the Add Operation
/// Depending on the usage, we may want to store the Add bytes in different ways
/// This trait allows for different implementations of the Add Operation within an Op
pub trait AddOp{
    /// Get the bytes for the Add Operation
    fn bytes(&self) -> &[u8];
}

/// Run Operation
/// * byte: The byte to repeat
/// * len: The number of times to repeat the byte (1-62)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run{
    pub byte: u8,
    pub len: u8,
}
/// Copy Operation
/// * src: The source of the copy (Dict or Output)
/// * addr: The absolute start position in the src to start copying from
/// * len: The number of bytes to copy.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Copy{
    pub src: CopySrc,
    pub addr: u64,
    pub len: u16,
}

/// Where the Copy Operation should copy from.
/// * Dict: Copy from the dictionary (source file)
/// * Output: Copy from the output buffer (output file)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CopySrc{
    Dict,
    Output,
}
/// Enum for the different types of operations
/// * Run: Repeat a byte a number of times
/// * Copy: Copy bytes from a source to the output
/// * Add: Add bytes to the output
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op<A>{
    Run(Run),
    Copy(Copy),
    Add(A),
}
impl<A> Op<A> {
    /// Get the bit flag for the operation type.
    /// This is per the spec.
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
    pub fn is_run(&self) -> bool {
        matches!(self, Op::Run(_))
    }
    pub fn is_copy(&self) -> bool {
        matches!(self, Op::Copy(_))
    }
    pub fn take_copy(&self) -> Option<Copy> {
        if let Op::Copy(copy) = self {
            Some(*copy)
        } else {
            None
        }
    }
}
impl<A:AddOp> Op<A> {
    /// Get the total number of bytes this operation will generate in the output stream.
    pub fn oal(&self) -> u16 {
        match &self {
            Op::Add(add) => add.bytes().len() as u16,
            Op::Copy(copy) => copy.len,
            Op::Run(run) => run.len as u16,
        }
    }
}


/// Used to determine how the size should be handled.
///
/// This is mostly to aid in control flow for encoding/decoding the size of an operation.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Size{
    Done(u8),
    U8And62,
    U16
}

impl Size {
    /// How many bytes will the Size Indicator be in the patch file.
    /// * 0 represents no Size Indicator is present.
    pub fn size_overhead(&self) -> usize {
        match self {
            Size::Done(_) => 0,
            Size::U8And62 => 1,
            Size::U16 => 2,
        }
    }
}
/// Used to determine how an operation of `size` (oal) should be encoded.
#[inline]
pub fn size_routine(size: u16)->Size{
    match size {
        1..=62 => Size::Done(size as u8),
        63 => Size::U8And62,
        _ => Size::U16,
    }
}

/// Convert an i64 to a u64 using ZigZag encoding.
#[inline]
pub fn zigzag_encode(n: i64) -> u64 {
    ((n << 1) ^ (n >> 63)) as u64
}

/// Convert a u64 to an i64 using ZigZag decoding.
#[inline]
pub fn zigzag_decode(z: u64) -> i64 {
    ((z >> 1) as i64) ^ -((z & 1) as i64)
}

/// Write a u64 value to the writer using u-varint encoding.
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

/// Read a u64 value from the reader at its current position using u-varint decoding.
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

/// Write an i64 value to the writer using i-varint encoding.
pub fn write_i_varint<W: std::io::Write>(writer: &mut W, n: i64) -> std::io::Result<()> {
    write_u_varint(writer, zigzag_encode(n as i64))
}

/// Read an i64 value from the reader at its current position using i-varint decoding.
pub fn read_i_varint<R: std::io::Read>(reader: &mut R) -> std::io::Result<i64> {
    Ok(zigzag_decode(read_u_varint(reader)?))
}

/// Read a u8 value from the reader at its current position.
pub fn read_u8<R: std::io::Read>(reader: &mut R) -> std::io::Result<u8> {
    let mut buf = [0u8; 1];
    reader.read_exact(&mut buf)?;
    Ok(buf[0])
}

/// Write a u8 value to the writer.
pub fn write_u8<W: std::io::Write>(writer: &mut W, n: u8) -> std::io::Result<()> {
    writer.write_all(&[n])
}

/// Read a u16(little-endian) value from the reader at its current position.
pub fn read_u16<R: std::io::Read>(reader: &mut R) -> std::io::Result<u16> {
    let mut buf = [0u8; 2];
    reader.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

/// Write a u16(little-endian) value to the writer.
pub fn write_u16<W: std::io::Write>(writer: &mut W, n: u16) -> std::io::Result<()> {
    writer.write_all(&n.to_le_bytes())
}

/// Helper fn to determine how many bytes a given u64 will take when encoded using u-varint.
#[inline]
pub fn u_varint_encode_size(n: u64) -> usize {
    let mut size = 1;
    let mut n = n >> 7;
    while n > 0 {
        size += 1;
        n >>= 7;
    }
    size
}

/// Helper fn to 'decode' a copy address from an i64 to the actual absolute address.
#[inline]
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

/// Helper fn to 'encode' a copy address from an absolute position (u64) to the relative difference from the last used absolute address.
#[inline]
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
    fn test_ivarint_vals() {
        let mut buffer = Vec::new();
        let values = [
            -64,63, // min 3 byte match
            -8192,8191, //min 4 byte match
            -1048576,1048575, //min 5 byte match
            -134217728,134217727, //min 6 byte match
            -123456789
        ];
        for &val in &values {
            buffer.clear();
            let zigged = zigzag_encode(val);
            write_i_varint(&mut buffer, val).unwrap();
            println!("i64: {} buffer:{:?} zigzagged: {}",val,buffer,zigged);
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
