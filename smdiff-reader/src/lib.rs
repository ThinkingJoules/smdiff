//!
//!This library is used to read the underlying smdiff format. It does not handle secondary decompression.
//!
//!If you need a reader for secondary decompression, you can use the smdiff-decoder::reader module. It wraps this lib.
//!
//!The main struct is the `SectionReader`. It reads a section at a time, and returns the ops and the header.
//!
//!The building blocks of that reader are exposed for other users to build their own readers.
//!
use std::io::Read;

use smdiff_common::{diff_addresses_to_u64, read_i_varint, read_u16, read_u8, read_u_varint, size_routine, AddOp, Copy, CopySrc, Format, Run, SectionHeader, Size, ADD, COPY_D, COPY_O, OP_MASK, RUN, SECTION_COMPRESSION_MASK, SECTION_COMPRESSION_RSHIFT, SECTION_CONTINUE_BIT, SECTION_FORMAT_BIT, SIZE_MASK};


/// Op Type alias for the Readers Add type
pub type Op = smdiff_common::Op<Add>;

/// Add Op for the Reader
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
/// Reads a section header from the reader at the current position.
pub fn read_section_header<R: std::io::Read>(reader: &mut R) -> std::io::Result<SectionHeader> {
    let header_byte = read_u8(reader)?;
    let compression_algo = (header_byte & SECTION_COMPRESSION_MASK) >> SECTION_COMPRESSION_RSHIFT;
    let format = if header_byte & SECTION_FORMAT_BIT == SECTION_FORMAT_BIT{Format::Segregated} else {Format::Interleaved};
    let more_sections = (header_byte & SECTION_CONTINUE_BIT) == SECTION_CONTINUE_BIT;
    let num_operations = read_u_varint(reader)? as u32;
    let num_add_bytes = if format.is_segregated() {
        read_u_varint(reader)? as u32
    } else {
        0
    };
    let read_size = read_u_varint(reader)? as u32;
    let output_size = if format.is_segregated(){
        num_add_bytes + read_size
    }else{
        read_size
    };

    Ok(SectionHeader {
        compression_algo,
        format,
        more_sections,
        num_operations,
        num_add_bytes,
        output_size,
    })
}

/// Reads the operations from the reader at the current position. Cannot have secondary compression still applied.
///
/// The mutable reference to the section header is so that the
/// function can update the number of add bytes in the event the format is interleaved.
/// This way the header reflects reality regardless of if it was originally encoded in the header.
pub fn read_ops_no_comp<R: std::io::Read>(reader: &mut R, header:&mut SectionHeader,op_buffer:&mut Vec<Op>)-> std::io::Result<()>{
    let SectionHeader { format, num_operations, output_size, .. } = header;
    //dbg!(&header);
    let mut cur_d_addr = 0;
    let mut cur_o_addr = 0;
    op_buffer.reserve(*num_operations as usize);
    match format {
        Format::Segregated => {
            let buffer_offset = op_buffer.len();
            let mut add_idxs = Vec::new();
            let mut check_size = 0;
            //dbg!(num_operations,output_size,num_add_bytes);
            for i in 0..*num_operations {
                let op = read_op(reader, &mut cur_d_addr, &mut cur_o_addr,false)?;
                let len = op.oal() as u32;
                check_size += len;
                if op.is_add(){
                    header.num_add_bytes += len;
                    add_idxs.push(buffer_offset+i as usize);
                }
                op_buffer.push(op);
            }
            if &check_size != output_size{
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Window Header output size: {} != Sum(ops.oal()) {}",output_size,check_size)));
            }
            //reader should be at the end of the instructions
            //now we go back and fill the add op buffers
            for i in add_idxs{
                let op = op_buffer.get_mut(i).unwrap();
                if let Op::Add(add) = op{
                    reader.read_exact(&mut add.bytes)?;
                }
            }
            Ok(())
        },
        Format::Interleaved => {
            let mut check_size = 0;
            for _ in 0..*num_operations {
                let op = read_op(reader, &mut cur_d_addr, &mut cur_o_addr,true)?;
                check_size += op.oal() as u32;
                op_buffer.push(op);
            }
            if &check_size != output_size{
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Window Header output size: {} != Sum(ops.oal()) {}",output_size,check_size)));
            }
            Ok(())
        }
    }
}

///Returns the ops and the output size. Cannot have secondary compression still applied.
///
/// This is just a wrapper that completely reads a section from the reader.
pub fn read_section<R: std::io::Read>(reader: &mut R, op_buffer:&mut Vec<Op>) -> std::io::Result<SectionHeader> {
    let mut header = read_section_header(reader)?;
    op_buffer.reserve(header.num_operations as usize);
    read_ops_no_comp(reader, &mut header, op_buffer)?;
    Ok(header)
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
/// Reads an operation from the reader at the given position.
/// * `reader` - The reader to read from.
/// * `cur_d_addr` - The last used copy dictionary address.
/// * `cur_o_addr` - The last used copy output address.
/// * `is_interleaved` - If the format is interleaved.
///
/// If this is segregated format, the Add ops will just be initialized to all zeros in the bytes field.
/// The caller will need to fill in the bytes later.
pub fn read_op<R: std::io::Read>(reader: &mut R,cur_d_addr:&mut u64,cur_o_addr:&mut u64,is_interleaved:bool) -> std::io::Result<Op> {
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
            if is_interleaved{
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

/// A reader that will keep reading sections until it reaches the terminal section.
pub struct SectionIterator<R>{
    source: R,
    done:bool,
    op_buffer: Vec<Op>,
}
impl<R: Read> SectionIterator<R>{
    pub fn new(source: R) -> Self {
        Self {
            source,
            done:false,
            op_buffer: Vec::new(),
        }
    }
    ///Reads and returns the next section (if it exists).
    ///
    /// This is useful if you don't need the Ops, just need to read them.
    pub fn next_borrowed(&mut self) -> Option<std::io::Result<(&[Op],SectionHeader)>> {
        if self.done{
            return None;
        }
        self.op_buffer.clear();
        let header = match read_section(&mut self.source,&mut self.op_buffer){
            Ok(v) => v,
            Err(e) => return Some(Err(e)),

        };
        if !header.more_sections{
            self.done = true;
        }
        Some(Ok((self.op_buffer.as_slice(),header)))
    }
    ///In the event the caller needs to do something to the ops (more than just read them), this avoids the need to clone the slice.
    fn next_owned(&mut self) -> Option<std::io::Result<(Vec<Op>,SectionHeader)>> {
        if self.done{
            return None;
        }
        let mut op_buffer = Vec::new();
        let header = match read_section(&mut self.source,&mut op_buffer){
            Ok(v) => v,
            Err(e) => return Some(Err(e)),

        };
        if !header.more_sections{
            self.done = true;
        }
        Some(Ok((op_buffer,header)))
    }
    pub fn into_inner(self) -> R {
        self.source
    }
}
impl<R: Read> Iterator for SectionIterator<R> {
    type Item = std::io::Result<(Vec<Op>, SectionHeader)>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_owned()
    }
}
#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;


    #[test]
    fn test_basic_add_run() {
        // Setup
        let ops= vec![
            Op::Add(Add::new("he".as_bytes().to_vec())),
            Op::Run(Run { byte: b'l', len: 2 }),
            Op::Add(Add::new("o".as_bytes().to_vec())),
        ];
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
        let mut reader = SectionIterator::new(Cursor::new(answer));
        while let Some(Ok((read_ops,_))) = reader.next_borrowed(){
            for (op,answer) in read_ops.iter().zip(ops.clone()) {
                assert_eq!(op, &answer);
            }
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
        let mut reader = SectionIterator::new(Cursor::new(answer));
        while let Some(Ok((read_ops,_))) = reader.next_borrowed(){
            for (op,answer) in read_ops.iter().zip(ops.clone()) {
                assert_eq!(op, &answer);
            }
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
        let mut reader = SectionIterator::new(Cursor::new(answer));
        let mut ops_iter = ops.iter();
        while let Some(Ok((read_ops,_))) = reader.next_borrowed(){
            let ans_ops = ops_iter.next().unwrap();
            for (op,answer) in read_ops.iter().zip(ans_ops.clone()) {
                assert_eq!(op, &answer);
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
                Op::Copy(Copy { src: CopySrc::Output, addr: 6, len: 5 }),
            ],
            vec![
                Op::Run(Run { byte: b'.', len: 3 }),
            ],
        ];

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
            5, //Output size uvarint diff encoded from add uvarint
            69, //COPY_O, Size 5 0b01_000100
            12, //addr ivar int +6

            64, // 0b0_1_000_000
            1, //Num ops uvarint
            0, //Num add bytes uvarint
            3, //Output size uvarint diff encoded from add uvarint
            195, //Run, Size 3 0b11_000011
            46, //'.'
        ];

        let mut reader = SectionIterator::new(Cursor::new(answer));
        let mut ops_iter = ops.iter();
        while let Some(Ok((read_ops,_))) = reader.next_borrowed(){
            let ans_ops = ops_iter.next().unwrap();
            for (op,answer) in read_ops.iter().zip(ans_ops.clone()) {
                assert_eq!(op, &answer);
            }
        }
    }
}

