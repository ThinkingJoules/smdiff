use smdiff_common::{diff_addresses_to_u64, read_i_varint, read_u16, read_u8, read_u_varint, size_routine, Add, Copy, CopySrc, FileHeader, Format, Op, Run, Size, WindowHeader, ADD, COPY_D, COPY_O, RUN, SIZE_MASK};






pub fn decode_file_header(header_byte: u8) -> FileHeader {
    let compression_algo = header_byte & 0b11000000;
    let format_bit = header_byte & 0b00100000;
    let operations_bits = header_byte & 0b0001_1111;

    let format = if format_bit == 0 {
        Format::MicroFormat{num_operations:operations_bits}
    } else {
        Format::WindowFormat
    };

    FileHeader {
        compression_algo,
        format,
    }
}


pub fn read_window_header<R: std::io::Read>(reader: &mut R) -> std::io::Result<WindowHeader> {
    let num_operations = read_u_varint(reader)?;
    let output_size = read_u_varint(reader)?;
    Ok(WindowHeader {
        num_operations: num_operations as u16,
        output_size: output_size as u32,
    })
}

pub fn read_section<R: std::io::Read>(reader: &mut R) -> std::io::Result<(Vec<Op>,Option<u32>)> {
    let header = decode_file_header(read_u8(reader)?);
    let mut cur_d_addr = 0;
    let mut cur_o_addr = 0;
    dbg!(&header);
    match header.format {
        Format::WindowFormat => {
            let WindowHeader { num_operations, output_size } = read_window_header(reader)?;
            let mut output = Vec::with_capacity(num_operations as usize);
            for _ in 0..num_operations {
                let op = read_op(reader, &mut cur_d_addr, &mut cur_o_addr)?;
                output.push(op);
            }
            Ok((output, Some(output_size)))
        },
        Format::MicroFormat{num_operations} => {
            let mut output = Vec::with_capacity(num_operations as usize);
            let mut out_size = 0;
            for _ in 0..num_operations {
                let op = read_op(reader, &mut cur_d_addr, &mut cur_o_addr)?;
                out_size += op.oal() as u32;
                output.push(op);
            }
            Ok((output, Some(out_size)))
        }
    }
}

enum OpType{
    CopyD,
    CopyO,
    Add,
    Run
}

struct OpByte{
    op:OpType,
    size:Size
}
fn read_op_byte<R: std::io::Read>(reader: &mut R) -> std::io::Result<OpByte> {
    let byte = read_u8(reader)?;
    let size_indicator = byte & SIZE_MASK;
    let op_type = byte & !SIZE_MASK;

    let size = size_routine(size_indicator as u16);
    match op_type {
        COPY_D => Ok(OpByte{op:OpType::CopyD,size}),
        COPY_O => Ok(OpByte{op:OpType::CopyO,size}),
        ADD => Ok(OpByte{op:OpType::Add,size}),
        RUN => Ok(OpByte{op:OpType::Run,size}),
        _ => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid op type")),
    }
}

fn read_op<R: std::io::Read>(reader: &mut R,cur_d_addr:&mut u64,cur_o_addr:&mut u64) -> std::io::Result<Op> {
    let OpByte { op, size } = read_op_byte(reader)?;
    if matches!(op, OpType::Run) && !matches!(size, Size::Done(_)) {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid size for RUN operation"));
    }
    let size = match size {
        Size::Done(size) => size as u16,
        Size::U8And62 => read_u8(reader)? as u16 + 62,
        Size::U16 => read_u16(reader)?,
    };
    let op = match op {
        OpType::CopyD => {
            let addr = read_i_varint(reader)?;
            let len = size;
            let from = CopySrc::Dict;
            *cur_d_addr = diff_addresses_to_u64(*cur_d_addr, addr);
            Op::Copy(Copy{src:from,addr:*cur_d_addr,len})
        },
        OpType::CopyO => {
            let addr = read_u_varint(reader)?;
            let len = size;
            let src = CopySrc::Output;
            let addr = *cur_o_addr - addr; //here encoding
            Op::Copy(Copy{src,addr,len})
        },
        OpType::Add => {
            let mut bytes = vec![0u8;size as usize];
            reader.read_exact(&mut bytes)?;
            Op::Add(Add{bytes})
        },
        OpType::Run => {
            Op::Run(Run{len:size as u8,byte:read_u8(reader)?})
        }
    };
    *cur_o_addr += op.oal() as u64;
    Ok(op)
}

#[cfg(test)] // Include this section only for testing
mod tests {
    use super::*;

    #[test]
    fn test_decode_microformat_header() {
        // Example MicroFormat header byte (compression 0, MicroFormat, 10 operations)
        let header_byte = 0b0000_1010;

        let result = decode_file_header(header_byte);

        let expected = FileHeader {
            compression_algo: 0,
            format: Format::MicroFormat { num_operations: 10 },
        };

        assert_eq!(result, expected);
    }

    #[test]
    fn test_decode_windowformat_header() {
        // Example WindowFormat header byte (compression 2, WindowFormat)
        let header_byte = 0b0110_0000;

        let result = decode_file_header(header_byte);

        let expected = FileHeader {
            compression_algo: 2,
            format: Format::WindowFormat,
        };

        assert_eq!(result, expected);
    }

    // Consider adding more tests:
    // * Test different compression values (0, 1, 2, 3)
    // * Test operations_bits = 31 (max value) for MicroFormat
    // * Test invalid headers might lead to errors (if your implementation handles it)
}