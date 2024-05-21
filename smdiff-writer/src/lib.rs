
use smdiff_common::{diff_addresses_to_i64, size_routine, write_i_varint, write_u16, write_u8, write_u_varint, AddOp, Copy, CopySrc, FileHeader, Format, Op, Size, WindowHeader, MAX_WIN_SIZE, SIZE_MASK};



pub fn write_file_header<W: std::io::Write>(header: &FileHeader, writer:&mut W) -> std::io::Result<()> {
    let mut header_byte = header.compression_algo;
    if let Format::MicroFormat { num_operations } = header.format {
        header_byte |= num_operations << 3;
    }else{
        header_byte |= 0b00000100;  // Set format bit
    }
    write_u8(writer, header_byte)
}

pub fn write_window_header<W: std::io::Write>(header: &WindowHeader, writer:&mut W) -> std::io::Result<()> {
    write_u_varint(writer, header.num_operations as u64)?;
    write_u_varint(writer, header.num_add_bytes as u64)?;
    let diff_encoded_output_size = header.output_size - header.num_add_bytes;
    write_u_varint(writer, diff_encoded_output_size as u64)?;
    Ok(())
}


pub fn write_micro_section<W: std::io::Write,A:AddOp>(ops: &[Op<A>], writer: &mut W) -> std::io::Result<()> {
    if ops.len() > 31 {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "MicroFormat must have 31 or fewer operations"));
    }
    let mut cur_d_addr = 0;
    let mut cur_o_addr = 0;
    for op in ops {
        write_op_byte_and_size(writer, &op)?;
        write_op_addtl(writer, &op, &mut cur_d_addr, &mut cur_o_addr)?;
    }
    if cur_o_addr > MAX_WIN_SIZE as u64 {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Output size is greater than MAX_WIN_SIZE"));
    }
    Ok(())
}
/// Writes the window header and then the ops to the writer
pub fn write_win_section<W: std::io::Write,A:AddOp>(ops: &[Op<A>], header:WindowHeader, writer: &mut W) -> std::io::Result<()> {
    // if output size is > MAX_WIN_SIZE, return error
    if header.output_size as usize > MAX_WIN_SIZE{
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Output size is greater than MAX_WIN_SIZE"));
    }
    if ops.len() != header.num_operations as usize {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Number of operations does not match header"));
    }
    write_window_header(&header, writer)?;

    let mut last_d_addr = 0;
    let mut last_o_addr = 0;
    let mut total_content_len = 0;
    let mut add_bytes_written = 0;
    let mut add_bytes_slices = Vec::new();
    for op in ops {
        total_content_len += op.oal() as usize;
        write_op_byte_and_size(writer, &op)?;
        match op {
            Op::Add(a) => {
                let slice = a.bytes();
                add_bytes_written += slice.len();
                add_bytes_slices.push(slice)
            },
            a => write_op_addtl(writer, a, &mut last_d_addr, &mut last_o_addr)?,
        }
    }
    for slice in add_bytes_slices {
        writer.write_all(slice)?;
    }
    if add_bytes_written != header.num_add_bytes as usize{
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Number of add bytes does not match header"));
    }
    if total_content_len as usize != header.output_size as usize {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Total content length {} does not match output size {}", total_content_len, header.output_size)));
    }
    Ok(())
}


fn write_op_byte_and_size<W: std::io::Write,A:AddOp>(writer: &mut W, op: &Op<A>) -> std::io::Result<()> {
    let byte = op.bit_flag();
    let size = size_routine(op.oal());
    assert!(if op.is_run() { op.oal() <= 62 } else { true });
    match size {
        Size::Done(len) => write_u8(writer, byte | len)?,
        Size::U8And62 => {
            assert!(!op.is_run());
            write_u8(writer, byte | SIZE_MASK)?;
            write_u8(writer, (op.oal() - 62) as u8)?
        },
        Size::U16 => {
            assert!(!op.is_run());
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
    Ok(())
}

#[cfg(test)]
mod test_super {
    use smdiff_common::Run;
    struct Add{
        bytes: Vec<u8>,
    }
    impl Add{
        fn new(bytes: Vec<u8>) -> Self {
            Self { bytes }
        }
    }
    impl AddOp for Add{
        fn bytes(&self) -> &[u8] {
            &self.bytes
        }
    }
    use super::*;
    #[test]
    fn test_basic_add_run() {
        // Setup
        let ops= vec![
            Op::Add(Add::new("he".as_bytes().to_vec())),
            Op::Run(Run { byte: b'l', len: 2 }),
            Op::Add(Add::new("o".as_bytes().to_vec())),
        ];
        let header = FileHeader { compression_algo: 0, format: Format::MicroFormat { num_operations: 3 } };
        let mut writer = Vec::new();
        write_file_header(&header, &mut writer).unwrap();
        write_micro_section(&ops, &mut writer).unwrap();

        let answer = vec![
            3, // 0b00_0_00011
            130, //ADD, Size 2 0b10_000010
            104, //'h'
            101, //'e'
            194, //RUN, Size 2 0b11_000010
            108, //'l'
            129, //ADD, Size 1 0b10_000001
            111 //'o'
        ];
        assert_eq!(writer, answer);

    }
    #[test]
    fn test_hello_micro() {
        // Instructions
        // "hello" -> "Hello! Hello!"
        let ops= vec![
            Op::Add(Add::new("H".as_bytes().to_vec())),
            Op::Copy(Copy { src: CopySrc::Dict, addr: 1, len: 4 }),
            Op::Add(Add::new("! ".as_bytes().to_vec())),
            Op::Copy(Copy { src: CopySrc::Output, addr: 0, len: 6 }),
        ];
        let header = FileHeader { compression_algo: 0, format: Format::MicroFormat { num_operations: 4 } };
        let mut writer = Vec::new();
        write_file_header(&header, &mut writer).unwrap();
        write_micro_section(&ops, &mut writer).unwrap();
        let answer = vec![
            4, // 0b00_0_00!00
            129, //ADD, Size 1 0b10_000001
            72, //'H'
            4, //COPY_D, Size 4 0b00_000100
            2, //addr ivar int +1
            130, //ADD, Size 2 0b10_000010
            33, //'!'
            32, //' '
            70, //COPY_O, Size 6 0b01_000110
            0, //addr ivar int 0
        ];
        assert_eq!(writer, answer);
    }
    #[test]
    pub fn test_hello_win(){
        //we need 3 windows, Neither, Src, and Target, in that order.
        //src will be 'hello' and output will be 'Hello! Hello!'
        //we encode just the Add(H) in the Neither window
        //then we encode the COPY(ello) in the Src window
        //then we encode the Copy(Hello!) in the Target window
        let header = FileHeader { compression_algo: 0, format: Format::WindowFormat };
        let mut writer = Vec::new();
        write_file_header(&header, &mut writer).unwrap();
        let win_ops: Vec<Op<Add>>= vec![
            Op::Add(Add::new("H".as_bytes().to_vec())),
        ];
        write_win_section(&win_ops, WindowHeader { num_operations: 1, num_add_bytes: 1, output_size: 1 }, &mut writer).unwrap();

        let win_ops: Vec<Op<Add>>= vec![
            Op::Copy(Copy { src: CopySrc::Dict, addr: 1, len: 4 }),
        ];
        write_win_section(&win_ops, WindowHeader { num_operations: 1, num_add_bytes: 0, output_size: 4 }, &mut writer).unwrap();

        let win_ops: Vec<Op<Add>>= vec![
            Op::Add(Add::new("! ".as_bytes().to_vec())),
        ];
        write_win_section(&win_ops, WindowHeader { num_operations: 1, num_add_bytes: 2, output_size: 2 }, &mut writer).unwrap();

        let win_ops: Vec<Op<Add>>= vec![
            Op::Copy(Copy { src: CopySrc::Output, addr: 0, len: 6 }),
        ];
        write_win_section(&win_ops, WindowHeader { num_operations: 1, num_add_bytes: 0, output_size: 6 }, &mut writer).unwrap();

        let answer = vec![
            32, //File Header 0b00_1_0000

            1, //Num ops uvarint
            1, //Num add bytes uvarint
            0, //Output size uvarint diff encoded from add uvarint
            129, //ADD, Size 1 0b10_000001
            72, //'H'

            1, //Num ops uvarint
            0, //Num add bytes uvarint
            4, //Output size uvarint diff encoded from add uvarint
            4, //COPY_D, Size 4 0b00_000100
            2, //addr ivar int +1

            1, //Num ops uvarint
            2, //Num add bytes uvarint
            0, //Output size uvarint diff encoded from add uvarint
            130, //ADD, Size 2 0b10_000010
            33, //'!'
            32, //' '

            1, //Num ops uvarint
            0, //Num add bytes uvarint
            6, //Output size uvarint diff encoded from add uvarint
            70, //COPY_O, Size 6 0b01_000110
            0, //addr ivar int 0
        ];

        assert_eq!(writer, answer);

    }

    #[test]
    pub fn kitchen_sink_transform(){
        //we need 3 windows, Neither, Src, and Target, in that order.
        //src will be 'hello' and output will be 'Hello! Hello! Hell...'
        //we encode just the Add(H) in the Neither window
        //then we encode the COPY(ello) in the Src window
        //then we encode the Copy(Hello!) in the Target window
        //then we encode the Copy(Hell) in the Target window, referencing the last window
        //then we encode the Add('.') in the Target window
        //then we encode an implicit Copy For the last '..' chars.
        let header = FileHeader { compression_algo: 0, format: Format::WindowFormat };
        let mut writer = Vec::new();
        write_file_header(&header, &mut writer).unwrap();
        let win_ops: Vec<Op<Add>>= vec![
            Op::Add(Add::new("H".as_bytes().to_vec())),
        ];
        write_win_section(&win_ops, WindowHeader { num_operations: 1, num_add_bytes: 1, output_size: 1 }, &mut writer).unwrap();

        let win_ops: Vec<Op<Add>>= vec![
            Op::Copy(Copy { src: CopySrc::Dict, addr: 1, len: 4 }),
        ];
        write_win_section(&win_ops, WindowHeader { num_operations: 1, num_add_bytes: 0, output_size: 4 }, &mut writer).unwrap();

        let win_ops: Vec<Op<Add>>= vec![
            Op::Add(Add::new("! ".as_bytes().to_vec())),
        ];
        write_win_section(&win_ops, WindowHeader { num_operations: 1, num_add_bytes: 2, output_size: 2 }, &mut writer).unwrap();

        let win_ops: Vec<Op<Add>>= vec![
            Op::Copy(Copy { src: CopySrc::Output, addr: 0, len: 6 }),
        ];
        write_win_section(&win_ops, WindowHeader { num_operations: 1, num_add_bytes: 0, output_size: 6 }, &mut writer).unwrap();

        let win_ops: Vec<Op<Add>>= vec![
            Op::Copy(Copy { src: CopySrc::Output, addr: 6, len: 4 }),
        ];
        write_win_section(&win_ops, WindowHeader { num_operations: 1, num_add_bytes: 0, output_size: 4 }, &mut writer).unwrap();

        let win_ops: Vec<Op<Add>>= vec![
            Op::Run(Run { byte: b'.', len: 3 }),
        ];
        write_win_section(&win_ops, WindowHeader { num_operations: 1, num_add_bytes: 0, output_size: 3 }, &mut writer).unwrap();

        //dbg!(&w);
        let answer = vec![
            32, //File Header 0b00_1_0000

            1, //Num ops uvarint
            1, //Num add bytes uvarint
            0, //Output size uvarint diff encoded from add uvarint
            129, //ADD, Size 1 0b10_000001
            72, //'H'

            1, //Num ops uvarint
            0, //Num add bytes uvarint
            4, //Output size uvarint diff encoded from add uvarint
            4, //COPY_D, Size 4 0b00_000100
            2, //addr ivar int +1

            1, //Num ops uvarint
            2, //Num add bytes uvarint
            0, //Output size uvarint diff encoded from add uvarint
            130, //ADD, Size 2 0b10_000010
            33, //'!'
            32, //' '

            1, //Num ops uvarint
            0, //Num add bytes uvarint
            6, //Output size uvarint diff encoded from add uvarint
            70, //COPY_O, Size 6 0b01_000110
            0, //addr ivar int 0

            1, //Num ops uvarint
            0, //Num add bytes uvarint
            4, //Output size uvarint diff encoded from add uvarint
            68, //COPY_O, Size 4 0b01_000100
            12, //addr ivar int +6

            1, //Num ops uvarint
            0, //Num add bytes uvarint
            3, //Output size uvarint diff encoded from add uvarint
            195, //Run, Size 3 0b11_000011
            46, //'.'
        ];

        assert_eq!(writer, answer);



    }

}