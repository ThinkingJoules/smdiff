use std::io::Seek;

use smdiff_common::{Add, Copy, CopySrc, Op, Run, MAX_RUN_LEN};
use smdiff_writer::write_section;
use vcdiff_common::{CopyType, Inst, Instruction, WinIndicator, ADD, RUN};
use vcdiff_reader::{VCDReader, VCDiffReadMsg};

const MAX_INST_SIZE: u32 = u16::MAX as u32;


pub fn convert_vcdiff_to_smdiff<R: std::io::Read+Seek, W: std::io::Write>(reader: R, mut writer: W) -> std::io::Result<()> {
    let mut cur_win = Vec::new();
    let mut reader = VCDReader::new(reader)?;
    let mut ssp = None;
    let mut vcd_trgt = false;
    let mut cur_o_pos = 0;
    let mut input_inst = 0;
    //eventually we will limit output windows by their output size. Something between u16::MAX and u32::MAX
    //for now we will write everything to a single window
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
                    input_inst += 1;
                    let inst = inst.unwrap();
                    match inst{
                        Inst::Add(ADD{ len, p_pos }) => {
                            let r = reader.get_reader(p_pos)?;
                            let total: u32 = len;
                            let mut processed: u32 = 0;
                            while processed < total {
                                let remaining = total - processed;
                                let chunk_size = if remaining > MAX_INST_SIZE { MAX_INST_SIZE } else { remaining };
                                let mut bytes = vec![0; chunk_size as usize];
                                r.read_exact(&mut bytes)?;
                                let op = Op::Add(Add{bytes});
                                cur_win.push(op);
                                processed += chunk_size;
                            }
                            cur_o_pos += len as u64;
                        },
                        Inst::Run(RUN{ len, byte }) => {
                            let total: u32 = len;
                            let mut processed: u32 = 0;
                            while processed < total {
                                let remaining = total - processed;
                                let chunk_size = if remaining > MAX_RUN_LEN as u32 { MAX_RUN_LEN as u32 } else { remaining };
                                let op = Op::Run(Run{byte,len:chunk_size as u8});
                                cur_win.push(op);
                                processed += chunk_size;
                            }
                            cur_o_pos += len as u64;
                        },
                        Inst::Copy(copy) =>{
                            let ssp = ssp.expect("SSP not set");
                            let (mut addr,src,seq) = match copy.copy_type{
                                CopyType::CopyS => {
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
                                    let chunk_size = if remaining > slice_len { slice_len } else { remaining };
                                    let op = Op::Copy(Copy{ src, addr, len: slice_len as u16 });
                                    cur_win.push(op);
                                    processed += chunk_size;
                                };
                                cur_o_pos += seq_len as u64;
                            }else{
                                let total: u32 = copy.len_in_o();
                                let mut processed: u32 = 0;
                                while processed < total {
                                    let remaining = total - processed;
                                    let chunk_size = if remaining > MAX_INST_SIZE { MAX_INST_SIZE } else { remaining };
                                    let op = Op::Copy(Copy{ src, addr, len: chunk_size as u16 });
                                    cur_win.push(op);
                                    addr += chunk_size as u64;
                                    processed += chunk_size;
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
    //now we determine what we need to write
    dbg!(input_inst,cur_win.len());
    write_section(cur_win,Some(cur_o_pos as u32),&mut writer)?;
    Ok(())
}
