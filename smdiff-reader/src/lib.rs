use std::io::Read;

use smdiff_common::{diff_addresses_to_u64, read_i_varint, read_u16, read_u8, read_u_varint, size_routine, AddOp, Copy, CopySrc, FileHeader, Format, Run, Size, WindowHeader, ADD, COPY_D, COPY_O, OP_MASK, RUN, SIZE_MASK};



pub type Op = smdiff_common::Op<Add>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Add{
    pub bytes: Vec<u8>,
}
impl Add{
    pub fn new(bytes: Vec<u8>) -> Self {
        Add { bytes }
    }
}

impl AddOp for Add{
    fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

pub fn decode_file_header(header_byte: u8) -> FileHeader {
    let compression_algo = header_byte & 0b00000011;
    let format_bit = header_byte & 0b00000100;
    let operations_bits = (header_byte & 0b1111_100) >> 3;

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
pub fn read_file_header<R: std::io::Read>(reader: &mut R) -> std::io::Result<FileHeader> {
    let header_byte = read_u8(reader)?;
    Ok(decode_file_header(header_byte))
}

pub fn read_window_header<R: std::io::Read>(reader: &mut R) -> std::io::Result<WindowHeader> {
    let num_operations = read_u_varint(reader)?;
    let num_add_bytes = read_u_varint(reader)?;
    let diff_encoded_output_size = read_u_varint(reader)?;
    let output_size = num_add_bytes + diff_encoded_output_size;
    //return err if output size is great MAX_WIN_SIZE
    if output_size > smdiff_common::MAX_WIN_SIZE as u64 {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Output size is greater than MAX_WIN_SIZE"));
    }


    Ok(WindowHeader {
        num_operations: num_operations as u32,
        output_size: output_size as u32,
        num_add_bytes: num_add_bytes as u32,
    })
}

///Returns the ops and the output size
pub fn read_section<R: std::io::Read>(reader: &mut R,header:FileHeader) -> std::io::Result<(Vec<Op>,usize)> {
    let mut cur_d_addr = 0;
    let mut cur_o_addr = 0;
    //dbg!(&header);
    match header.format {
        Format::WindowFormat => {
            let WindowHeader { num_operations, output_size, ..  } = read_window_header(reader)?;
            let mut output = Vec::with_capacity(num_operations as usize);
            let mut add_idxs = Vec::new();
            let mut check_size = 0;
            //dbg!(num_operations,output_size,num_add_bytes);
            for i in 0..num_operations {
                let op = read_op(reader, &mut cur_d_addr, &mut cur_o_addr,false)?;
                check_size += op.oal() as u32;
                if op.is_add(){
                    add_idxs.push(i as usize);
                }
                output.push(op);
            }
            if check_size != output_size{
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Window Header output size: {} != Sum(ops.oal()) {}",output_size,check_size)));
            }
            //reader should be at the end of the instructions
            //now we go back and fill the add op buffers
            for i in add_idxs{
                let op = output.get_mut(i).unwrap();
                if let Op::Add(add) = op{
                    reader.read_exact(&mut add.bytes)?;
                }
            }
            Ok((output, output_size as usize))
        },
        Format::MicroFormat{num_operations} => {
            let mut output = Vec::with_capacity(num_operations as usize);
            let mut out_size = 0;
            for _ in 0..num_operations {
                let op = read_op(reader, &mut cur_d_addr, &mut cur_o_addr,true)?;
                out_size += op.oal() as u32;
                output.push(op);
            }
            Ok((output, out_size as usize))
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum OpType{
    Copy{src:CopySrc},
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
    let op_type = byte & OP_MASK;

    let size = size_routine(size_indicator as u16);
    match op_type {
        COPY_D => Ok(OpByte{op:OpType::Copy { src: CopySrc::Dict },size}),
        COPY_O => Ok(OpByte{op:OpType::Copy { src: CopySrc::Output },size}),
        ADD => Ok(OpByte{op:OpType::Add,size}),
        RUN => Ok(OpByte{op:OpType::Run,size}),
        _ => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid op type")),
    }
}
fn read_op<R: std::io::Read>(reader: &mut R,cur_d_addr:&mut u64,cur_o_addr:&mut u64,is_micro_fmt:bool) -> std::io::Result<Op> {
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
        OpType::Copy { src } => {
            let addr = read_i_varint(reader)?;
            let len = size;
            let addr = if src == CopySrc::Dict {
                *cur_d_addr = diff_addresses_to_u64(*cur_d_addr, addr);
                *cur_d_addr
            } else {
                *cur_o_addr = diff_addresses_to_u64(*cur_o_addr, addr);
                *cur_o_addr
            };
            Op::Copy(Copy{src,addr,len})
        },
        OpType::Add => {
            let mut bytes = vec![0u8;size as usize];
            if is_micro_fmt{
                reader.read_exact(&mut bytes)?;
            }
            Op::Add(Add{bytes})
        },
        OpType::Run => {
            Op::Run(Run{len:size as u8,byte:read_u8(reader)?})
        }
    };
    Ok(op)
}

pub struct SectionReader<R>{
    source: R,
    header: FileHeader,
}
impl<R: Read> SectionReader<R>{
    pub fn new(mut source: R) -> std::io::Result<Self> {
        let header: FileHeader = read_file_header(&mut source)?;
        Ok(Self {
            source,
            header,
        })
    }
    pub fn next(&mut self) -> std::io::Result<Option<(Vec<Op>,usize)>> {
        let (ops,output_size) = match read_section(&mut self.source, self.header){
            Ok(v) => v,
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Ok(None);
            },
            Err(e) => return Err(e),

        };
        Ok(Some((ops,output_size)))
    }
    pub fn into_inner(self) -> R {
        self.source
    }
}


#[cfg(test)] // Include this section only for testing
mod tests {
    use std::io::Cursor;

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
            compression_algo: 1,
            format: Format::WindowFormat,
        };

        assert_eq!(result, expected);
    }

    #[test]
    fn test_basic_add_run() {
        // Setup
        let ops= vec![
            Op::Add(Add::new("he".as_bytes().to_vec())),
            Op::Run(Run { byte: b'l', len: 2 }),
            Op::Add(Add::new("o".as_bytes().to_vec())),
        ];
        let header = FileHeader { compression_algo: 0, format: Format::MicroFormat { num_operations: 3 } };
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
        let mut reader = Cursor::new(answer);
        let read_header = read_file_header(&mut reader).unwrap();
        assert_eq!(header, read_header);
        for (op,answer) in read_section(&mut reader, read_header).unwrap().0.into_iter().zip(ops) {
            assert_eq!(op, answer);
        }

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
        let mut reader = Cursor::new(answer);
        let read_header = read_file_header(&mut reader).unwrap();
        assert_eq!(header, read_header);
        for (op,answer) in read_section(&mut reader, read_header).unwrap().0.into_iter().zip(ops) {
            assert_eq!(op, answer);
        }
    }
    #[test]
    pub fn test_hello_win(){
        //we need 3 windows, Neither, Src, and Target, in that order.
        //src will be 'hello' and output will be 'Hello! Hello!'
        //we encode just the Add(H) in the Neither window
        //then we encode the COPY(ello) in the Src window
        //then we encode the Copy(Hello!) in the Target window
        let ops = [
            vec![
                Op::Add(Add::new("H".as_bytes().to_vec())),
            ],
            vec![
                Op::Copy(Copy { src: CopySrc::Dict, addr: 1, len: 4 }),
            ],
            vec![
                Op::Add(Add::new("! ".as_bytes().to_vec())),
            ],
            vec![
                Op::Copy(Copy { src: CopySrc::Output, addr: 0, len: 6 }),
            ]
        ];
        let header = FileHeader { compression_algo: 0, format: Format::WindowFormat };

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
        let mut reader = Cursor::new(answer);
        let read_header = read_file_header(&mut reader).unwrap();
        assert_eq!(header, read_header);
        for i in 0..4{
            let (read_ops,_) = read_section(&mut reader, read_header).unwrap();
            for (op,answer) in read_ops.into_iter().zip(ops[i].clone()) {
                assert_eq!(op, answer);
            }
        }

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
        let ops = [
            vec![
                Op::Add(Add::new("H".as_bytes().to_vec())),
            ],
            vec![
                Op::Copy(Copy { src: CopySrc::Dict, addr: 1, len: 4 }),
            ],
            vec![
                Op::Add(Add::new("! ".as_bytes().to_vec())),
            ],
            vec![
                Op::Copy(Copy { src: CopySrc::Output, addr: 0, len: 6 }),
            ],
            vec![
                Op::Copy(Copy { src: CopySrc::Output, addr: 6, len: 4 }),
            ],
            vec![
                Op::Run(Run { byte: b'.', len: 3 }),
            ],
        ];
        let header = FileHeader { compression_algo: 0, format: Format::WindowFormat };

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

        let mut reader = Cursor::new(answer);
        let read_header = read_file_header(&mut reader).unwrap();
        assert_eq!(header, read_header);
        for i in 0..6{
            let (read_ops,_) = read_section(&mut reader, read_header).unwrap();
            for (op,answer) in read_ops.into_iter().zip(ops[i].clone()) {
                assert_eq!(op, answer);
            }
        }
    }
}

