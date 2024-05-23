use std::{fmt::Debug, io::{BufReader, Read, Seek, Write}};

use smdiff_common::SectionHeader;
use smdiff_reader::read_ops;

pub mod zstd{
    pub use ruzstd::streaming_decoder::StreamingDecoder;
}
pub mod brotli{
    pub use brotlic::DecompressorReader;
}
///Applies a SMDiff patch to a source buffer
/// # Arguments
/// * `patch` - A Read object that contains the SMDiff patch data
/// * `src` - An optional mutable reference to a Read+Seek object that contains the source (dictionary) data
/// * `sink` - A Write object that will receive the patched data
/// # Errors
/// Returns an error if there is an issue reading from the patch or source data, or writing to the sink
pub fn apply_patch<P:Read+Debug,R:Read+Seek,W:Write>(patch:&mut P,mut src:Option<&mut R>,sink:&mut W) -> std::io::Result<()> {
    //to avoid the Read+Seek bound on sink,
    //we need to scan the whole patch file so we can cache the TargetSourced windows
    let mut cur_o = Vec::new();
    let mut cur_o_pos = 0;
    //let mut stats = Stats::default();
    let mut win_data = Vec::new();
    let mut patch_reader = BufReader::new(patch);
    loop {
        let header = smdiff_reader::read_section_header(&mut patch_reader)?;
        //dbg!(&header);
        if header.compression_algo == 1 {
            apply_no_sec_comp::<_,R,_>(&mut patch_reader, None, &mut win_data)?;
            let mut crsr = std::io::Cursor::new(&win_data);
            read_ops_no_comp::<_,_,W>(&mut crsr,&mut src, &header,&mut cur_o, &mut cur_o_pos)?;
        }else if header.compression_algo == 2 {
            let mut zstd = ruzstd::StreamingDecoder::new(&mut patch_reader).unwrap();
            read_ops_no_comp::<_,_,W>(&mut zstd,&mut src, &header,&mut cur_o, &mut cur_o_pos)?;
        }else if header.compression_algo == 3 {
            let mut brot = brotlic::DecompressorReader::new(patch_reader);
            read_ops_no_comp::<_,_,W>(&mut brot,&mut src, &header,&mut cur_o, &mut cur_o_pos)?;
            patch_reader = brot.into_inner().unwrap();
        }else{
            read_ops_no_comp::<_,_,W>(&mut patch_reader,&mut src, &header,&mut cur_o, &mut cur_o_pos)?;
        };
        if !header.more_sections{
            break;
        }
        win_data.clear();
    }
    sink.write_all(&cur_o)?;
    Ok(())
}

fn apply_no_sec_comp<P:Read,R:Read+Seek,W:Write>(patch:&mut P,mut src:Option<&mut R>,sink:&mut W) -> std::io::Result<()> {
    let mut cur_o = Vec::new();
    let mut cur_o_pos = 0;
    loop {
        let header = smdiff_reader::read_section_header(patch)?;
        read_ops_no_comp::<_,_,W>(patch, &mut src, &header,&mut cur_o, &mut cur_o_pos)?;
        if !header.more_sections{
            break;
        }
    }
    sink.write_all(&cur_o)?;
    Ok(())
}

fn read_ops_no_comp<P:Read,R:Read+Seek,W:Write>(patch:&mut P,src:&mut Option<&mut R>,header:&SectionHeader,cur_o:&mut Vec<u8>, cur_o_pos: &mut usize) -> std::io::Result<()> {
    //let mut stats = Stats::default();
    let ops =  read_ops(patch, &header)?;
    let out_size = header.output_size as usize;
    cur_o.reserve_exact(out_size);
    cur_o.resize(cur_o.len() + out_size, 0);
    for op in ops {
        match op {
            smdiff_common::Op::Add(add) => {
                cur_o[*cur_o_pos..*cur_o_pos+add.bytes.len()].copy_from_slice(&add.bytes);
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
                        let start_pos = *cur_o_pos;
                        src.read_exact(&mut cur_o[start_pos..start_pos+copy.len as usize])?;
                        *cur_o_pos += copy.len as usize;
                    },
                    smdiff_common::CopySrc::Output => {
                        let start_pos = *cur_o_pos;
                        cur_o.copy_within(copy.addr as usize..copy.addr as usize+copy.len as usize,start_pos);
                        *cur_o_pos += copy.len as usize;
                    },
                }
                //stats.copy();
            },
            smdiff_common::Op::Run(run) => {
                //stats.run();
                cur_o[*cur_o_pos..*cur_o_pos+run.len as usize].fill(run.byte);
                *cur_o_pos += run.len as usize;
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
        let mut sink = Vec::new();
        apply_patch(&mut patch,Some(&mut src),&mut sink).unwrap();
        assert_eq!(sink, "Hello! Hello!".as_bytes());
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
        let mut sink = Vec::new();
        apply_patch(&mut patch,Some(&mut src),&mut sink).unwrap();
        assert_eq!(sink, "Hello! Hello!".as_bytes());
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
        let mut sink = Vec::new();
        apply_patch(&mut patch,Some(&mut src),&mut sink).unwrap();
        assert_eq!(sink, "Hello! Hello! Hell...".as_bytes());

    }

    // #[test]
    // fn test_kitchen_sink2(){
    //     // "hello world!" -> "Hello! Hello! Hello. "
    //     let mut src = Cursor::new("hello world!".as_bytes().to_vec());

    //     //from encoder tests
    //     let patch = vec![
    //         192, // 0b1_1_000_000
    //         1, //Num ops uvarint
    //         1, //Num add bytes uvarint
    //         0, //Output size uvarint diff encoded from add uvarint
    //         129, //ADD, Size 1 0b10_000001
    //         72, //'H'

    //         192, // 0b1_1_000_000
    //         1, //Num ops uvarint
    //         0, //Num add bytes uvarint
    //         4, //Output size uvarint diff encoded from add uvarint
    //         4, //COPY_D, Size 4 0b00_000100
    //         2, //addr ivar int +1

    //         192, // 0b1_1_000_000
    //         1, //Num ops uvarint
    //         2, //Num add bytes uvarint
    //         0, //Output size uvarint diff encoded from add uvarint
    //         130, //ADD, Size 2 0b10_000010
    //         33, //'!'
    //         32, //' '

    //         192, // 0b1_1_000_000
    //         1, //Num ops uvarint
    //         0, //Num add bytes uvarint
    //         6, //Output size uvarint diff encoded from add uvarint
    //         70, //COPY_O, Size 6 0b01_000110
    //         0, //addr ivar int 0

    //         192, // 0b1_1_000_000
    //         1, //Num ops uvarint
    //         0, //Num add bytes uvarint
    //         4, //Output size uvarint diff encoded from add uvarint
    //         68, //COPY_O, Size 4 0b01_000100
    //         12, //addr ivar int +6

    //         64, // 0b0_1_000_000
    //         1, //Num ops uvarint
    //         0, //Num add bytes uvarint
    //         3, //Output size uvarint diff encoded from add uvarint
    //         195, //Run, Size 3 0b11_000011
    //         46, //'.'
    //     ];
    //     let mut patch = Cursor::new(patch);
    //     let mut sink = Vec::new();
    //     apply_patch(&mut patch,Some(&mut src),&mut sink).unwrap();
    //     let str = std::str::from_utf8(&sink).unwrap();
    //     assert_eq!(str, "Hello! Hello! Hello. ");

    // }
}