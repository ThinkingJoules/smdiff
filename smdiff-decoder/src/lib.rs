
use std::io::{Read,Seek,Write};

use smdiff_reader::read_ops;

///Applies a SMDiff patch to a source buffer
/// # Arguments
/// * `patch` - A Read object that contains the SMDiff patch data
/// * `src` - An optional mutable reference to a Read+Seek object that contains the source (dictionary) data
/// * `sink` - A Write object that will receive the patched data
/// # Errors
/// Returns an error if there is an issue reading from the patch or source data, or writing to the sink
pub fn apply_patch<P:Read,R:Read+Seek,W:Write>(patch:&mut P,mut src:Option<&mut R>,sink:&mut W) -> std::io::Result<()> {
    //to avoid the Read+Seek bound on sink,
    //we need to scan the whole patch file so we can cache the TargetSourced windows
    let mut cur_o = Vec::new();
    let mut cur_o_pos = 0;
    //let mut stats = Stats::default();
    let mut win_data = Vec::new();
    loop {
        let header = smdiff_reader::read_section_header(patch)?;
        let ops = if header.compression_algo == 1 {
            apply_patch::<_,R,_>(patch, None, &mut win_data)?;
            let mut crsr = std::io::Cursor::new(&win_data);
            match read_ops(&mut crsr, &header) {
                Ok(a) => a,
                Err(e) if matches!(e.kind(), std::io::ErrorKind::UnexpectedEof) => {
                    break;
                }
                Err(e) => {
                    return Err(e);
                },
            }
        }else{
            match read_ops(patch, &header) {
                Ok(a) => a,
                Err(e) if matches!(e.kind(), std::io::ErrorKind::UnexpectedEof) => {
                    break;
                }
                Err(e) => {
                    dbg!(&e);
                    return Err(e);
                },
            }
        };
        let out_size = header.output_size as usize;
        cur_o.reserve_exact(out_size);
        cur_o.resize(cur_o.len() + out_size, 0);
        for op in ops {
            match op {
                smdiff_common::Op::Add(add) => {
                    cur_o[cur_o_pos..cur_o_pos+add.bytes.len()].copy_from_slice(&add.bytes);
                    cur_o_pos += add.bytes.len();
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
                            let start_pos = cur_o_pos;
                            src.read_exact(&mut cur_o[start_pos..start_pos+copy.len as usize])?;
                            cur_o_pos += copy.len as usize;
                        },
                        smdiff_common::CopySrc::Output => {
                            let start_pos = cur_o_pos;
                            cur_o.copy_within(copy.addr as usize..copy.addr as usize+copy.len as usize,start_pos);
                            cur_o_pos += copy.len as usize;
                        },
                    }
                    //stats.copy();
                },
                smdiff_common::Op::Run(run) => {
                    //stats.run();
                    cur_o[cur_o_pos..cur_o_pos+run.len as usize].fill(run.byte);
                    cur_o_pos += run.len as usize;
                },
            }
        }
        if !header.more_sections{
            break;
        }
        win_data.clear();
    }
    sink.write_all(&cur_o)?;
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
            214,195,196,0, //magic
            0, //hdr_indicator
            1, //win_indicator VCD_SOURCE
            4, //SSS
            1, //SSP
            12, //delta window size
            13, //target window size
            0, //delta indicator
            3, //length of data for ADDs and RUNs
            2, //length of instructions and sizes
            2, //length of addresses for COPYs
            72,33,32, //'H! ' data section
            235, //ADD1 COPY4_mode6
            183, //ADD2 COPY6_mode0
            0,
            4,
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
            214,195,196,0, //magic
            0, //hdr_indicator
            0, //win_indicator Neither
            7, //delta window size
            1, //target window size
            0, //delta indicator
            1, //length of data for ADDs and RUN/
            1, //length of instructions and size
            0, //length of addr
            72, //data section 'H
            2, //ADD1 (i = 13)
            1, //win_indicator VCD_SOURCE
            4, //SSS
            1, //SSP
            8, //delta window size
            5, //target window size
            0, //delta indicator
            1, //length of data for ADDs and RUN/
            1, //length of instructions and size
            1, //length of addr
            33, //data section '!'
            253, //COPY4_mode5 ADD1
            0, //addr 0
            2, //win_indicator VCD_TARGET
            6, //SSS
            0, //SSP
            9, //delta window size
            7, //target window size
            0, //delta indicator
            1, //length of data for ADDs and RUN/
            2, //length of instructions and size
            1, //length of addr
            32, //data section ' '
            2, //ADD1 NOOP
            118, //COPY6_mode6 NOOP
            0, //addr 0
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
            214,195,196,0, //magic
            0, //hdr_indicator
            0, //win_indicator Neither
            7, //delta window size
            1, //target window size
            0, //delta indicator
            1, //length of data for ADDs and RUN/
            1, //length of instructions and size
            0, //length of addr
            72, //data section 'H
            2, //ADD1
            1, //win_indicator VCD_SOURCE
            4, //SSS
            1, //SSP
            8, //delta window size
            5, //target window size
            0, //delta indicator
            1, //length of data for ADDs and RUN/
            1, //length of instructions and size
            1, //length of addr
            33, //data section '!'
            253, //COPY4_mode5 ADD1
            0, //addr 0
            2, //win_indicator VCD_TARGET
            6, //SSS
            0, //SSP
            9, //delta window size
            7, //target window size
            0, //delta indicator
            1, //length of data for ADDs and RUN/
            2, //length of instructions and size
            1, //length of addr
            32, //data section ' '
            2, //ADD1 NOOP
            118, //COPY6_mode6 NOOP
            0, //addr 0
            2, //win_indicator VCD_TARGET
            5, //SSS
            6, //SSP
            12, //delta window size
            8, //target window size
            0, //delta indicator
            1, //length of data for ADDs and RUN/
            4, //length of instructions and size
            2, //length of addr
            46, //data section '.'
            117, //ADD1 COPY5_mode6
            2, //Add1 NOOP
            35, //COPY0_mode1
            3, //...size
            0, //addr 0
            1, //addr 1
        ];
        let mut patch = Cursor::new(patch);
        let mut sink = Vec::new();
        apply_patch(&mut patch,Some(&mut src),&mut sink).unwrap();
        assert_eq!(sink, "Hello! Hello! Hell...".as_bytes());

    }

#[test]
    fn test_kitchen_sink2(){
        // "hello world!" -> "Hello! Hello! Hello. "
        let mut src = Cursor::new("hello world!".as_bytes().to_vec());

        //from encoder tests
        let patch = vec![
            214,195,196,0, //magic
            0, //hdr_indicator
            1, //win_indicator Src
            11, //SSS
            1, //SSP
            14, //delta window size
            7, //target window size
            0, //delta indicator
            1, //length of data for ADDs and RUN/
            5, //length of instructions and size
            3, //length of addr
            72, //data section 'H'
            163, //ADD1 COPY4_mode0
            19, //COPY0_mode0
            1, //..size
            19, //COPY0_mode0
            1, //..size
            0, //addr 0
            10, //addr 1
            4, //addr 2
            2, //win_indicator VCD_TARGET
            7, //SSS
            0, //SSP
            14, //delta window size
            14, //target window size
            0, //delta indicator
            1, //length of data for ADDs and RUN/
            5, //length of instructions and size
            3, //length of addr
            46, //data section '.'
            23, //COPY0_mode0 noop
            28, //..size
            2, //Add1 NOOP
            19, //COPY0_mode0
            1, //..size
            0, //addr 0
            7, //addr 1
            13, //addr 2
        ];
        let mut patch = Cursor::new(patch);
        let mut sink = Vec::new();
        apply_patch(&mut patch,Some(&mut src),&mut sink).unwrap();
        let str = std::str::from_utf8(&sink).unwrap();
        assert_eq!(str, "Hello! Hello! Hello. ");

    }
}