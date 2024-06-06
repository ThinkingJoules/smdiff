//! This lib is used to *construct* valid SMDIFF format delta files.
//! This is *not* an encoder.
//! However, if you did write an encoder this would help you write the ops to a file.
use smdiff_common::{diff_addresses_to_i64, size_routine, write_i_varint, write_u16, write_u8, write_u_varint, AddOp, Copy, CopySrc, Format, Op, SectionHeader, Size, MAX_INST_SIZE, MAX_WIN_SIZE, SECTION_COMPRESSION_RSHIFT, SECTION_CONTINUE_BIT, SECTION_FORMAT_BIT, SIZE_MASK};


/// Used to write the header to the section.
/// * `header` - The header to write.
/// * `writer` - The writer to write to.
pub fn write_section_header<W: std::io::Write>(header: &SectionHeader, writer:&mut W) -> std::io::Result<()> {
    let mut cntl_byte = header.compression_algo << SECTION_COMPRESSION_RSHIFT;
    if let Format::Segregated = header.format {
        cntl_byte |= SECTION_FORMAT_BIT;  // Set format bit
    }
    if header.more_sections{
        cntl_byte |= SECTION_CONTINUE_BIT; // Set continuation bit
    }
    write_u8(writer, cntl_byte)?;
    write_u_varint(writer, header.num_operations as u64)?;
    let output_size = if header.format == Format::Segregated {
        write_u_varint(writer, header.num_add_bytes as u64)?;
        header.output_size - header.num_add_bytes
    } else {
        header.output_size
    };
    write_u_varint(writer, output_size as u64)?;
    Ok(())
}

/// Used to write just the ops for the section.
/// * `ops` - The operations to write.
/// * `header` - The header for the section. This must match the contents of the ops.
/// * `writer` - The writer to write to.
pub fn write_ops<W: std::io::Write,A:AddOp>(ops: &[Op<A>], header:&SectionHeader, writer: &mut W) -> std::io::Result<()> {
    if header.output_size as usize > MAX_WIN_SIZE{
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Output size is greater than MAX_WIN_SIZE"));
    }
    if ops.len() != header.num_operations as usize {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Number of operations does not match header"));
    }
    let mut last_d_addr = 0;
    let mut last_o_addr = 0;
    let mut total_content_len = 0;
    let mut add_bytes_written = 0;
    let mut add_bytes_slices = Vec::new();
    for op in ops {
        total_content_len += op.oal() as usize;
        write_op_byte_and_size(writer, &op)?;
        match header.format{
            Format::Interleaved => write_op_addtl(writer, &op, &mut last_d_addr, &mut last_o_addr)?,
            Format::Segregated => {
                match op {
                    Op::Add(a) => {
                        let slice = a.bytes();
                        add_bytes_written += slice.len();
                        add_bytes_slices.push(slice)
                    },
                    a => write_op_addtl(writer, a, &mut last_d_addr, &mut last_o_addr)?,
                }
            },
        }
    }
    if header.format == Format::Segregated {
        for slice in add_bytes_slices {
            writer.write_all(slice)?;
        }
        if add_bytes_written != header.num_add_bytes as usize{
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Number of add bytes does not match header"));
        }
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

/// This takes a large list of ops and divides them into sections that are no larger than `max_section_size`
/// * `ops` - The list of ops to divide into sections.
/// * `max_section_size` - The maximum size of each section in output bytes.
/// * Returns a vector of tuples containing the ops for each section and the header for that section.
pub fn make_sections<A:AddOp>(ops: &[Op<A>], max_section_size: usize) -> Vec<(&[Op<A>],SectionHeader)> {
    let max_win_size = max_section_size.clamp(MAX_INST_SIZE, MAX_WIN_SIZE) as u32;
    let mut result = Vec::new();
    let mut output_size = 0;
    let mut num_add_bytes = 0;
    let mut start_index = 0;

    for (end_index, op) in ops.iter().enumerate() {
        // Check if adding the current op exceeds the window size
        let op_size = op.oal() as u32;
        if output_size + op_size > max_win_size {
            result.push((&ops[start_index..end_index],SectionHeader{ num_operations: (end_index-start_index) as u32, num_add_bytes, output_size, compression_algo: 0, format: Format::Interleaved, more_sections: true }));
            start_index = end_index;
            output_size = 0;
            num_add_bytes = 0;
        }
        if op.is_add() {
            num_add_bytes += op_size;
        }
        output_size += op_size;
    }

    // Add the last group
    result.push((&ops[start_index..],SectionHeader{ num_operations: (ops.len()-start_index) as u32, num_add_bytes, output_size, compression_algo: 0, format: Format::Interleaved, more_sections: false }));

    result
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
        let header = SectionHeader { compression_algo: 0, format: Format::Interleaved , num_operations: 3 , num_add_bytes: 3, output_size: 5 , more_sections:false};
        let mut writer = Vec::new();
        write_section_header(&header, &mut writer).unwrap();
        write_ops(&ops, &header,&mut writer).unwrap();

        let answer = vec![
            0, // 0b0_0_000_000
            3, //num_ops uvarint
            5, //output size uvarint
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
        let header = SectionHeader { compression_algo: 0, format: Format::Interleaved  , num_operations: 4 , num_add_bytes: 3, output_size: 13 , more_sections:false };
        let mut writer = Vec::new();
        write_section_header(&header, &mut writer).unwrap();
        write_ops(&ops, &header,&mut writer).unwrap();
        let answer = vec![
            0, // 0b0_0_000_000
            4, //num_ops uvarint
            13, //output size uvarint
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
        let mut writer = Vec::new();
        let win_ops: Vec<Op<Add>>= vec![
            Op::Add(Add::new("H".as_bytes().to_vec())),
        ];
        let header = SectionHeader {
            compression_algo: 0,
            format: Format::Segregated ,
            num_operations: 1 ,
            num_add_bytes: 1,
            output_size: 1 ,
            more_sections:true
        };
        write_section_header(&header, &mut writer).unwrap();
        write_ops(&win_ops, &header,&mut writer).unwrap();

        let win_ops: Vec<Op<Add>>= vec![
            Op::Copy(Copy { src: CopySrc::Dict, addr: 1, len: 4 }),
        ];
        let header = SectionHeader {
            compression_algo: 0,
            format: Format::Segregated ,
            num_operations: 1 ,
            num_add_bytes: 0,
            output_size: 4 ,
            more_sections:true
        };
        write_section_header(&header, &mut writer).unwrap();
        write_ops(&win_ops, &header,&mut writer).unwrap();

        let win_ops: Vec<Op<Add>>= vec![
            Op::Add(Add::new("! ".as_bytes().to_vec())),
        ];
        let header = SectionHeader {
            compression_algo: 0,
            format: Format::Segregated ,
            num_operations: 1 ,
            num_add_bytes: 2,
            output_size: 2 ,
            more_sections:true
        };
        write_section_header(&header, &mut writer).unwrap();
        write_ops(&win_ops, &header,&mut writer).unwrap();
        let win_ops: Vec<Op<Add>>= vec![
            Op::Copy(Copy { src: CopySrc::Output, addr: 0, len: 6 }),
        ];
        let header = SectionHeader {
            compression_algo: 0,
            format: Format::Segregated ,
            num_operations: 1 ,
            num_add_bytes: 0,
            output_size: 6 ,
            more_sections:false
        };
        write_section_header(&header, &mut writer).unwrap();
        write_ops(&win_ops, &header,&mut writer).unwrap();
        let answer = vec![
            192, // 0b1_1_000_000
            1, //Num ops uvarint
            1, //Num add bytes uvarint
            0, //Output size uvarint diff encoded from add uvarint
            129, //ADD, Size 1 0b10_000001
            72, //'H'

            192, // 0b1_1_000_000
            1, //Num ops uvarint
            0, //Num add bytes uvarint
            4, //Output size uvarint diff encoded from add uvarint
            4, //COPY_D, Size 4 0b00_000100
            2, //addr ivar int +1

            192, // 0b1_1_000_000
            1, //Num ops uvarint
            2, //Num add bytes uvarint
            0, //Output size uvarint diff encoded from add uvarint
            130, //ADD, Size 2 0b10_000010
            33, //'!'
            32, //' '

            64, // 0b0_1_000_000
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
        let mut writer = Vec::new();
        let win_ops: Vec<Op<Add>>= vec![
            Op::Add(Add::new("H".as_bytes().to_vec())),
        ];
        let header = SectionHeader {
            compression_algo: 0,
            format: Format::Segregated ,
            num_operations: 1 ,
            num_add_bytes: 1,
            output_size: 1 ,
            more_sections:true
        };
        write_section_header(&header, &mut writer).unwrap();
        write_ops(&win_ops, &header,&mut writer).unwrap();

        let win_ops: Vec<Op<Add>>= vec![
            Op::Copy(Copy { src: CopySrc::Dict, addr: 1, len: 4 }),
        ];
        let header = SectionHeader {
            compression_algo: 0,
            format: Format::Segregated ,
            num_operations: 1 ,
            num_add_bytes: 0,
            output_size: 4 ,
            more_sections:true
        };
        write_section_header(&header, &mut writer).unwrap();
        write_ops(&win_ops, &header,&mut writer).unwrap();

        let win_ops: Vec<Op<Add>>= vec![
            Op::Add(Add::new("! ".as_bytes().to_vec())),
        ];
        let header = SectionHeader {
            compression_algo: 0,
            format: Format::Segregated ,
            num_operations: 1 ,
            num_add_bytes: 2,
            output_size: 2 ,
            more_sections:true
        };
        write_section_header(&header, &mut writer).unwrap();
        write_ops(&win_ops, &header,&mut writer).unwrap();

        let win_ops: Vec<Op<Add>>= vec![
            Op::Copy(Copy { src: CopySrc::Output, addr: 0, len: 6 }),
        ];
        let header = SectionHeader {
            compression_algo: 0,
            format: Format::Segregated ,
            num_operations: 1 ,
            num_add_bytes: 0,
            output_size: 6 ,
            more_sections:true
        };
        write_section_header(&header, &mut writer).unwrap();
        write_ops(&win_ops, &header,&mut writer).unwrap();

        let win_ops: Vec<Op<Add>>= vec![
            Op::Copy(Copy { src: CopySrc::Output, addr: 6, len: 4 }),
        ];
        let header = SectionHeader {
            compression_algo: 0,
            format: Format::Segregated ,
            num_operations: 1 ,
            num_add_bytes: 0,
            output_size: 4 ,
            more_sections:true
        };
        write_section_header(&header, &mut writer).unwrap();
        write_ops(&win_ops, &header,&mut writer).unwrap();

        let win_ops: Vec<Op<Add>>= vec![
            Op::Run(Run { byte: b'.', len: 3 }),
        ];
        let header = SectionHeader {
            compression_algo: 0,
            format: Format::Segregated ,
            num_operations: 1 ,
            num_add_bytes: 0,
            output_size: 3 ,
            more_sections:false
        };
        write_section_header(&header, &mut writer).unwrap();
        write_ops(&win_ops, &header,&mut writer).unwrap();


        //dbg!(&w);
        let answer = vec![
            192, // 0b1_1_000_000
            1, //Num ops uvarint
            1, //Num add bytes uvarint
            0, //Output size uvarint diff encoded from add uvarint
            129, //ADD, Size 1 0b10_000001
            72, //'H'

            192, // 0b1_1_000_000
            1, //Num ops uvarint
            0, //Num add bytes uvarint
            4, //Output size uvarint diff encoded from add uvarint
            4, //COPY_D, Size 4 0b00_000100
            2, //addr ivar int +1

            192, // 0b1_1_000_000
            1, //Num ops uvarint
            2, //Num add bytes uvarint
            0, //Output size uvarint diff encoded from add uvarint
            130, //ADD, Size 2 0b10_000010
            33, //'!'
            32, //' '

            192, // 0b1_1_000_000
            1, //Num ops uvarint
            0, //Num add bytes uvarint
            6, //Output size uvarint diff encoded from add uvarint
            70, //COPY_O, Size 6 0b01_000110
            0, //addr ivar int 0

            192, // 0b1_1_000_000
            1, //Num ops uvarint
            0, //Num add bytes uvarint
            4, //Output size uvarint diff encoded from add uvarint
            68, //COPY_O, Size 4 0b01_000100
            12, //addr ivar int +6

            64, // 0b0_1_000_000
            1, //Num ops uvarint
            0, //Num add bytes uvarint
            3, //Output size uvarint diff encoded from add uvarint
            195, //Run, Size 3 0b11_000011
            46, //'.'
        ];

        assert_eq!(writer, answer);



    }

}