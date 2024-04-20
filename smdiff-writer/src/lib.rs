
use smdiff_common::{diff_addresses_to_i64, size_routine, write_i_varint, write_u16, write_u8, write_u_varint, AddOp, Copy, CopySrc, FileHeader, Format, Op, Size, WindowHeader, COPY_D, COPY_O, MAX_WIN_SIZE, RUN, SIZE_MASK};



pub fn write_file_header<W: std::io::Write>(header: &FileHeader, mut writer: W) -> std::io::Result<()> {
    let mut header_byte = header.compression_algo;
    if let Format::MicroFormat { num_operations } = header.format {
        header_byte |= num_operations & 0b00011111;
    }else{
        header_byte |= 0b00100000;  // Set format bit
    }
    write_u8(&mut writer, header_byte)
}

pub fn write_window_header<W: std::io::Write>(header: &WindowHeader, mut writer: W) -> std::io::Result<()> {
    write_u_varint(&mut writer, header.num_operations as u64)?;
    write_u_varint(&mut writer, header.output_size as u64)?;
    Ok(())
}

pub fn write_micro_section<W: std::io::Write,A:AddOp>(ops: Vec<Op<A>>, mut writer: W) -> std::io::Result<()> {
    // Derive header from section data
    let header = FileHeader {
        compression_algo: 0, // Assume compression is not used for writing
        format: if ops.len() <= 31 {
            Format::MicroFormat { num_operations: ops.len() as u8}
        } else {
            //err
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "MicroFormat must have 31 or fewer operations"));
        }
    };

    write_file_header(&header, &mut writer)?;

    let mut cur_d_addr = 0;
    let mut cur_o_addr = 0;
    for op in ops {
        write_op_byte_and_size(&mut writer, &op)?;
        write_op_addtl(&mut writer, &op, &mut cur_d_addr, &mut cur_o_addr)?;
    }
    if cur_o_addr > MAX_WIN_SIZE as u64 {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Output size is greater than MAX_WIN_SIZE"));
    }
    Ok(())
}
pub fn write_win_section<W: std::io::Write,A:AddOp>(ops: Vec<Op<A>>, num_add_bytes:usize, output_size: usize, mut writer: W) -> std::io::Result<()> {
    // Derive header from section data
    let header = FileHeader {
        compression_algo: 0, // Assume compression is not used for writing
        format: Format::WindowFormat
    };

    write_file_header(&header, &mut writer)?;

    // if output size is > MAX_WIN_SIZE, return error
    if output_size > MAX_WIN_SIZE{
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Output size is greater than MAX_WIN_SIZE"));
    }
    let window_header = WindowHeader {
        num_operations: ops.len() as u32,
        num_add_bytes: num_add_bytes as u32,
        output_size: output_size as u32,
    };
    write_window_header(&window_header, &mut writer)?;

    let mut cur_d_addr = 0;
    let mut cur_o_addr = 0;
    let mut add_bytes_written = 0;
    let mut add_bytes_slices = Vec::new();
    for op in &ops {
        write_op_byte_and_size(&mut writer, &op)?;
        write_op_addtl(&mut writer, &op, &mut cur_d_addr, &mut cur_o_addr)?;
        match op {
            Op::Add(a) => {
                let slice = a.bytes();
                add_bytes_written += slice.len();
                add_bytes_slices.push(slice)
            },
            a => write_op_addtl(&mut writer, a, &mut cur_d_addr, &mut cur_o_addr)?,
        }
    }
    for slice in add_bytes_slices {
        writer.write_all(slice)?;
    }
    if add_bytes_written != num_add_bytes {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Number of add bytes does not match header"));
    }
    if cur_o_addr as usize != output_size {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Output size does not match header"));
    }
    Ok(())
}


fn write_op_byte_and_size<W: std::io::Write,A:AddOp>(writer: &mut W, op: &Op<A>) -> std::io::Result<()> {
    let byte = op.bit_flag();
    let size = size_routine(op.oal());
    match size {
        Size::Done(size) => write_u8(writer, byte | size)?,
        Size::U8And62 => {
            write_u8(writer, byte | SIZE_MASK)?;
            write_u8(writer, (op.oal() - 62) as u8)?
        },
        Size::U16 => {
            write_u8(writer, byte)?;
            write_u16(writer, op.oal())?;
        }
    }
    Ok(())
}

fn write_op_addtl<W: std::io::Write,A:AddOp>(writer: &mut W, op: &Op<A>, cur_d_addr: &mut u64, cur_o_addr: &mut u64) -> std::io::Result<()> {
    match op {
        Op::Run(run) => write_u8(writer, run.byte)?,
        Op::Add(add) => writer.write_all(&add.bytes())?,
        Op::Copy (Copy{ src,addr, .. }) => {
            // Derive difference based on target or source address
            match src {
                CopySrc::Dict => {
                    let int = diff_addresses_to_i64(*cur_d_addr, *addr);
                    write_i_varint(writer, int)?;
                    *cur_d_addr = *addr;
                },
                CopySrc::Output =>{
                    let int = diff_addresses_to_i64(*cur_o_addr, *addr);
                    write_i_varint(writer, int)?;
                    *cur_o_addr = *addr;
                },
            }
        }
    }
    *cur_o_addr += op.oal() as u64;
    Ok(())
}

