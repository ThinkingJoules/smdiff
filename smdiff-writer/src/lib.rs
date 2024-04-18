
use smdiff_common::{diff_addresses_to_i64, size_routine, write_i_varint, write_u16, write_u8, write_u_varint, Add, Copy, CopySrc, FileHeader, Format, Op, Run, Size, WindowHeader, COPY_D, COPY_O, RUN, SIZE_MASK};

// ... (Other Structs and Enums remain the same)

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

pub fn write_section<W: std::io::Write>(ops: Vec<Op>, output_size: Option<u32>, mut writer: W) -> std::io::Result<()> {
    // Derive header from section data
    let header = FileHeader {
        compression_algo: 0, // Assume compression is not used for writing
        format: if ops.len() > 31 {
            Format::WindowFormat
        } else {
            Format::MicroFormat { num_operations: ops.len() as u8 }
        }
    };

    write_file_header(&header, &mut writer)?;

    match header.format {
        Format::WindowFormat => {
            let window_header = WindowHeader {
                num_operations: ops.len() as u16,
                output_size: output_size.unwrap(), // Assumption: output_size is always present
            };
            write_window_header(&window_header, &mut writer)?;
        },
        Format::MicroFormat { .. } => {
            // No window header needed in MicroFormat
        }
    }

    let mut cur_d_addr = 0;
    let mut cur_o_addr = 0;
    for op in ops {
        write_op(&mut writer, &op, &mut cur_d_addr, &mut cur_o_addr)?;
    }
    Ok(())
}


fn write_op<W: std::io::Write>(writer: &mut W, op: &Op, cur_d_addr: &mut u64, cur_o_addr: &mut u64) -> std::io::Result<()> {
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

    match op {
        Op::Run(run) => write_u8(writer, run.byte)?,
        Op::Add(add) => writer.write_all(&add.bytes)?,
        Op::Copy (Copy{ src,addr, .. }) => {
            // Derive difference based on target or source address
            match src {
                CopySrc::Dict => {
                    let int = diff_addresses_to_i64(*cur_d_addr, *addr);
                    write_i_varint(writer, int)?;
                    *cur_d_addr = *addr;
                },
                CopySrc::Output =>{
                    let int = *cur_o_addr - *addr;
                    write_u_varint(writer, int)?;
                },
            }
        }
    }
    *cur_o_addr += op.oal() as u64;
    Ok(())
}

