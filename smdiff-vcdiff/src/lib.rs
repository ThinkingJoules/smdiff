use std::io::Seek;

use smdiff_common::{AddOp, Copy, CopySrc, Run, WindowHeader, MAX_RUN_LEN, MAX_WIN_SIZE, MICRO_MAX_INST_COUNT};
use smdiff_writer::{write_file_header, write_micro_section, write_win_section};
use vcdiff_common::{CopyType, Inst, Instruction, WinIndicator, ADD, RUN};
use vcdiff_reader::{VCDReader, VCDiffReadMsg};

const MAX_INST_SIZE: u32 = u16::MAX as u32;
pub type Op = smdiff_common::Op<Add>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Add{
    pub bytes: Vec<u8>,
}

impl AddOp for Add{
    fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

pub fn convert_vcdiff_to_smdiff<R: std::io::Read+Seek, W: std::io::Write>(reader: R, mut writer: W) -> std::io::Result<()> {
    let mut cur_win = Vec::new();
    let mut reader = VCDReader::new(reader)?;
    let mut ssp = None;
    let mut vcd_trgt = false;
    let mut cur_o_pos = 0;
    let mut cur_win_size = 0;
    let mut num_add_bytes = 0;
    let mut file_header = None;
    loop{
        match reader.next()?{
            VCDiffReadMsg::WindowSummary(ws) => {
                ssp = ws.source_segment_position;
                if ws.win_indicator == WinIndicator::VCD_TARGET{
                    vcd_trgt = true;
                }
            },
            VCDiffReadMsg::Inst { first, second } => {
                for inst in [Some(first), second]{
                    if inst.is_none(){
                        continue;
                    }
                    let inst = inst.unwrap();
                    match inst{
                        Inst::Add(ADD{ len, p_pos }) => {
                            let r = reader.get_reader(p_pos)?;
                            let total: u32 = len;
                            let mut processed: u32 = 0;
                            while processed < total {
                                let remaining = total - processed;
                                let chunk_size = remaining.min(MAX_INST_SIZE);
                                if cur_win_size + chunk_size > MAX_WIN_SIZE as u32{
                                    if file_header.is_none(){
                                        file_header = Some(smdiff_common::FileHeader{ compression_algo: 0, format: smdiff_common::Format::WindowFormat });
                                        smdiff_writer::write_file_header(&file_header.unwrap(),&mut writer)?;
                                    }
                                    write_win_section(&cur_win, WindowHeader{ num_operations: cur_win.len() as u32, num_add_bytes, output_size:cur_win_size },&mut writer)?;
                                    cur_win.clear();
                                    cur_win_size = 0;
                                    num_add_bytes = 0;
                                }
                                let mut bytes = vec![0; chunk_size as usize];
                                r.read_exact(&mut bytes)?;
                                let op = Op::Add(Add{bytes});
                                cur_win.push(op);
                                processed += chunk_size;
                                num_add_bytes += chunk_size;
                                cur_win_size += chunk_size;
                            }
                            cur_o_pos += len as u64;
                        },
                        Inst::Run(RUN{ len, byte }) => {
                            println!("Run @{}: of: {} let: {}",cur_o_pos,byte,len);
                            let total: u32 = len;
                            let mut processed: u32 = 0;
                            while processed < total {
                                let remaining = total - processed;
                                let chunk_size = remaining.min(MAX_RUN_LEN as u32);
                                if cur_win_size + chunk_size > MAX_WIN_SIZE as u32{
                                    if file_header.is_none(){
                                        file_header = Some(smdiff_common::FileHeader{ compression_algo: 0, format: smdiff_common::Format::WindowFormat });
                                        smdiff_writer::write_file_header(&file_header.unwrap(),&mut writer)?;
                                    }
                                    write_win_section(&cur_win, WindowHeader{ num_operations: cur_win.len() as u32, num_add_bytes, output_size:cur_win_size },&mut writer)?;
                                    cur_win.clear();
                                    cur_win_size = 0;
                                    num_add_bytes = 0;
                                }
                                assert!(chunk_size <= MAX_RUN_LEN as u32);
                                let op = Op::Run(Run{byte,len:chunk_size as u8});
                                cur_win.push(op);
                                cur_win_size += chunk_size;
                                processed += chunk_size;
                            }
                            cur_o_pos += len as u64;
                        },
                        Inst::Copy(copy) =>{
                            let (mut addr,src,seq) = match copy.copy_type{
                                CopyType::CopyS => {
                                    let ssp = ssp.expect("SSP not set");
                                    let addr = ssp+copy.u_pos as u64;
                                    let src = if vcd_trgt {CopySrc::Output}else{CopySrc::Dict};
                                    (addr,src,None)
                                },
                                CopyType::CopyT { inst_u_pos_start } => {
                                    let offset = inst_u_pos_start - copy.u_pos;
                                    let addr = cur_o_pos - offset as u64;
                                    let src = CopySrc::Output;
                                    (addr,src,None)
                                },
                                CopyType::CopyQ { len_o } => {
                                    let slice_len = copy.len_in_u() - len_o;
                                    let addr = cur_o_pos - slice_len as u64;
                                    let src = CopySrc::Output;
                                    (addr,src,Some((slice_len,len_o)))
                                },
                            };
                            if let Some((slice_len,seq_len)) = seq {
                                let total: u32 = seq_len;
                                let mut processed: u32 = 0;
                                while processed < total {
                                    let remaining = total - processed;
                                    let chunk_size = remaining.min(slice_len);
                                    dbg!(slice_len,chunk_size);
                                    if cur_win_size + chunk_size > MAX_WIN_SIZE as u32{
                                        if file_header.is_none(){
                                            file_header = Some(smdiff_common::FileHeader{ compression_algo: 0, format: smdiff_common::Format::WindowFormat });
                                            smdiff_writer::write_file_header(&file_header.unwrap(),&mut writer)?;
                                        }
                                        write_win_section(&cur_win, WindowHeader{ num_operations: cur_win.len() as u32, num_add_bytes, output_size:cur_win_size },&mut writer)?;
                                        cur_win.clear();
                                        cur_win_size = 0;
                                        num_add_bytes = 0;
                                    }
                                    let op = Op::Copy(Copy{ src, addr, len: chunk_size as u16 });
                                    cur_win.push(op);
                                    processed += chunk_size;
                                    cur_win_size += chunk_size;

                                };
                                cur_o_pos += seq_len as u64;
                            }else{
                                let total: u32 = copy.len_in_o();
                                let mut processed: u32 = 0;
                                while processed < total {
                                    let remaining = total - processed;
                                    let chunk_size = MAX_INST_SIZE.min(remaining);
                                    if cur_win_size + chunk_size > MAX_WIN_SIZE as u32{
                                        if file_header.is_none(){
                                            file_header = Some(smdiff_common::FileHeader{ compression_algo: 0, format: smdiff_common::Format::WindowFormat });
                                            smdiff_writer::write_file_header(&file_header.unwrap(),&mut writer)?;
                                        }
                                        write_win_section(&cur_win, WindowHeader{ num_operations: cur_win.len() as u32, num_add_bytes, output_size:cur_win_size },&mut writer)?;
                                        cur_win.clear();
                                        cur_win_size = 0;
                                        num_add_bytes = 0;
                                    }
                                    let op = Op::Copy(Copy{ src, addr, len: chunk_size as u16 });
                                    cur_win.push(op);
                                    addr += chunk_size as u64;
                                    processed += chunk_size;
                                    cur_win_size += chunk_size;

                                };
                                cur_o_pos += copy.len_in_u() as u64;
                            }
                        }
                    }
                }
            },
            VCDiffReadMsg::EndOfWindow => {
                ssp = None;
                vcd_trgt = false;
            },
            VCDiffReadMsg::EndOfFile => break,
        }
    }
    dbg!(cur_o_pos,file_header);
    //now we determine what we need to write
    if file_header.is_none() && cur_win.len() <= MICRO_MAX_INST_COUNT{
        write_file_header(&smdiff_common::FileHeader{ compression_algo: 0, format: smdiff_common::Format::MicroFormat{ num_operations: cur_win.len() as u8 } },&mut writer)?;
        write_micro_section(&cur_win,&mut writer)?;
    }else{
        if file_header.is_none(){
            smdiff_writer::write_file_header(&smdiff_common::FileHeader{ compression_algo: 0, format: smdiff_common::Format::WindowFormat },&mut writer)?;
        }
        write_win_section(&cur_win, WindowHeader{ num_operations: cur_win.len() as u32, num_add_bytes, output_size:cur_win_size },&mut writer)?;
    }
    Ok(())
}

#[cfg(test)]
mod test_super {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn test_hello_micro() {
        //'hello' -> 'Hello! Hello!'
        let mut vcd_bytes = Cursor::new(vec![
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
            163, //ADD1 COPY4_mode6
            183, //ADD2 COPY6_mode0
            0,
            4,
        ]);
        let smd_bytes = vec![
            4, // 0b00_0_00100
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
        let mut out = Vec::new();
        convert_vcdiff_to_smdiff(&mut vcd_bytes, &mut out).unwrap();
        assert_eq!(out, smd_bytes);
    }

    #[test]
    fn test_seq(){
        // Instructions -> "" -> "tererest'
        let mut vcd_bytes = Cursor::new(vec![
            214, 195, 196, 0,  //magic
            0,  //hdr_indicator
            0, //win_indicator
            13, //size_of delta window
            8, //size of target window
            0, //delta indicator
            5, //length of data for ADDs and RUNs
            2, //length of instructions and sizes
            1, //length of addresses for COPYs
            116, 101, 114, 115, 116, //data section b"terst" 12..17
            200, //ADD size3 & COPY5_mode0
            3, //ADD size 2
            1, //addr for copy
        ]);
        let smd_bytes = vec![ //should be Add(ter), Copy(1,2), Copy(1,1),Add(st)
            4, // 0b00_0_00100
            131, //ADD, Size 3 0b10_000011
            116, //'t'
            101, //'e'
            114, //'r'
            66, //COPY_O, Size 2 0b01_000010
            2, //addr ivar int +1
            65, //COPY_O, Size 1 0b01_000011
            0, //addr ivar int 0
            130, //ADD, Size 2 0b10_000010
            115, //'s'
            116, //'t'
        ];

        let mut out = Vec::new();
        convert_vcdiff_to_smdiff(&mut vcd_bytes, &mut out).unwrap();
        assert_eq!(out, smd_bytes);

    }

    #[test]
    fn test_run(){
        // Instructions -> "" -> "r' x 128 long
        let mut vcd_bytes = Cursor::new(vec![
            214, 195, 196, 0,  //magic
            0,  //hdr_indicator
            0, //win_indicator
            9, //size_of delta window
            128, //size of target window
            0, //delta indicator
            1, //length of data for ADDs and RUNs
            1, //length of instructions and sizes
            1, //length of addresses for COPYs
            114, //data section b"terst" 12..17
            0, //RUN
            129,0, //len 128
        ]);
        let smd_bytes = vec![ //should be Add(ter), Copy(1,2), Copy(1,1),Add(st)
            3, // 0b00_0_00011
            254, //RUN, Size 62 0b11_111110
            114, //'r'
            254, //RUN, Size 62 0b11_111110
            114, //'r'
            196, //RUN, Size 4 0b10_000100
            114, //'r'
        ];

        let mut out = Vec::new();
        convert_vcdiff_to_smdiff(&mut vcd_bytes, &mut out).unwrap();
        assert_eq!(out, smd_bytes);

    }
}