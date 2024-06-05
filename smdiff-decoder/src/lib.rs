use std::io::{Read, Seek, Write};

use smdiff_common::MAX_INST_SIZE;

pub mod zstd{
    //! Re-exports the zstd streaming decoder used
    pub use ruzstd::streaming_decoder::StreamingDecoder;
}
pub mod brotli{
    //! Re-exports the brotli decompressor reader used
    pub use brotlic::DecompressorReader;
}
pub mod reader;
///Applies an SMDiff patch to a source buffer
/// # Arguments
/// * `patch` - A Read object that contains the SMDiff patch data
/// * `src` - An optional mutable reference to a Read+Seek object that contains the source (dictionary) data
/// * `sink` - A Write object that will receive the patched data
/// # Errors
/// Returns an error if there is an issue reading from the patch or source data, or writing to the sink
///
/// Note: To enable patch application to large files, we require Read+Seek on the Sink to handle CopySrc::Output operations
pub fn apply_patch<P:Read+Seek,R:Read+Seek,W:Write+Read+Seek>(patch:&mut P,mut src:Option<&mut R>,sink:&mut W) -> std::io::Result<()> {
    let mut cur_o_pos: usize = 0;
    //let mut stats = Stats::default();
    let mut reader = crate::reader::SectionIterator::new(patch);
    while let Some(res) = reader.next_borrowed(){
        let (ops,_header) = res?;
        apply_ops(ops, &mut src, sink, &mut cur_o_pos)?;
    }
    Ok(())
}


fn apply_no_sec_comp<P:Read,R:Read+Seek,W:Write+Read+Seek>(patch:&mut P,mut src:Option<&mut R>,sink:&mut W) -> std::io::Result<()> {
    //To avoid Seek on write, we must write all the output data to a Vec<u8> first
    let mut cur_o_pos = 0;
    let mut reader = smdiff_reader::SectionIterator::new(patch);
    while let Some(res) = reader.next_borrowed(){
        let (ops,_header) = res?;
        apply_ops(ops, &mut src, sink, &mut cur_o_pos)?;

    }
    Ok(())
}

/// Applies a series of operations to a buffer
/// Here `cur_o` represents the output buffer.
/// We could replace it with W:Write+Read+Seek if we didn't want to allocate the entire output buffer in memory
/// So... maybe TODO?
fn apply_ops<R:Read+Seek,W:Write+Read+Seek>(ops:&[smdiff_reader::Op],src:&mut Option<&mut R>,cur_o:&mut W, cur_o_pos: &mut usize) -> std::io::Result<()> {
    //let mut stats = Stats::default();
    //let out_size = header.output_size as usize;
    cur_o.seek(std::io::SeekFrom::Start(*cur_o_pos as u64))?;
    // cur_o.reserve_exact(out_size);
    // cur_o.resize(cur_o.len() + out_size, 0);
    let mut copy_buffer = vec![0u8;MAX_INST_SIZE];
    for op in ops {
        match op {
            smdiff_common::Op::Add(add) => {
                cur_o.write_all(&add.bytes)?;
                *cur_o_pos += add.bytes.len();
                //stats.add();
            },
            smdiff_common::Op::Copy(copy) => {
                match copy.src{
                    smdiff_common::CopySrc::Dict => {
                        let src = match src.as_mut(){
                            Some(s) => s,
                            None => panic!("Copy operation without source data"),
                        };
                        src.seek(std::io::SeekFrom::Start(copy.addr))?;
                        let len = copy.len as usize;
                        src.read_exact(&mut copy_buffer[..len])?;
                        cur_o.write_all(&copy_buffer[..len])?;
                        *cur_o_pos += len;
                    },
                    smdiff_common::CopySrc::Output => {
                        let start_pos = *cur_o_pos;
                        cur_o.seek(std::io::SeekFrom::Start(copy.addr as u64))?;
                        let len = copy.len as usize;
                        cur_o.read_exact(&mut copy_buffer[..len])?;
                        cur_o.seek(std::io::SeekFrom::Start(start_pos as u64))?;
                        cur_o.write_all(&copy_buffer[..len])?;
                        *cur_o_pos += len;
                    },
                }
                //stats.copy();
            },
            smdiff_common::Op::Run(run) => {
                //stats.run();
                let len = run.len as usize;
                copy_buffer[..len].fill(run.byte);
                cur_o.write_all(&copy_buffer[..len])?;
                *cur_o_pos += len;
            },
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    #[test]
    fn test_src_apply(){
        // "hello" -> "Hello! Hello!"
        let mut src = Cursor::new("hello".as_bytes().to_vec());

        //from encoder tests
        let patch = vec![
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
        let mut patch = Cursor::new(patch);
        let mut sink = Cursor::new(Vec::new());
        apply_patch(&mut patch,Some(&mut src),&mut sink).unwrap();
        assert_eq!(sink.into_inner(), "Hello! Hello!".as_bytes());
    }
    #[test]
    fn test_complex_apply(){
        // "hello" -> "Hello! Hello!"
        let mut src = Cursor::new("hello".as_bytes().to_vec());

        //from encoder tests
        let patch = vec![
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
        let mut patch = Cursor::new(patch);
        let mut sink = Cursor::new(Vec::new());
        apply_patch(&mut patch,Some(&mut src),&mut sink).unwrap();
        assert_eq!(sink.into_inner(), "Hello! Hello!".as_bytes());
    }

    #[test]
    fn test_kitchen_sink(){
        // "hello" -> "Hello! Hello! Hell..."
        let mut src = Cursor::new("hello".as_bytes().to_vec());

        //from encoder tests
        let patch = vec![
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
        let mut patch = Cursor::new(patch);
        let mut sink = Cursor::new(Vec::new());
        apply_patch(&mut patch,Some(&mut src),&mut sink).unwrap();
        assert_eq!(sink.into_inner(), "Hello! Hello! Hell...".as_bytes());

    }
}