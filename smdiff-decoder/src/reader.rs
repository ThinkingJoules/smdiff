use std::io::{BufReader, Read, Seek};

use smdiff_common::SectionHeader;
use smdiff_reader::{read_ops_no_comp, read_section_header, Op};

use crate::apply_no_sec_comp;
pub struct SectionReader<R>{
    source: BufReader<R>,
    done:bool,
    win_data: Vec<u8>,
    ops: Vec<Op>,
}
impl<R: Read+Seek> SectionReader<R>{
    pub fn new(patch: R) -> Self {
        Self {
            source:BufReader::new(patch),
            done:false,
            win_data: Vec::new(),
            ops: Vec::new(),
        }
    }
    pub fn next(&mut self) -> Option<std::io::Result<(&[Op],SectionHeader)>>{
        if self.done{
            return None;
        }
        self.win_data.clear();
        self.ops.clear();
        let header = match read_section_header(&mut self.source){
            Ok(h) => h,
            Err(e) => {
                return Some(Err(e));
            }
        };
        if header.compression_algo == 1 {
            if let Err(e) = apply_no_sec_comp::<_,R,_>(&mut self.source, None, &mut self.win_data){
                return Some(Err(e));
            }
            let mut crsr = std::io::Cursor::new(&self.win_data);
            if let Err(e) = read_ops_no_comp(&mut crsr, &header,&mut self.ops){
                return Some(Err(e));
            }
        }else if header.compression_algo == 2 {
            let mut zstd = ruzstd::StreamingDecoder::new(&mut self.source).unwrap();
            if let Err(e) = read_ops_no_comp(&mut zstd, &header,&mut self.ops){
                return Some(Err(e));
            }
        }else if header.compression_algo == 3 {
            let mut brot = brotlic::DecompressorReader::new(&mut self.source);
            if let Err(e) = read_ops_no_comp(&mut brot, &header,&mut self.ops){
                return Some(Err(e));
            }
            if let Err(e) = brot.into_inner(){
                return Some(Err(e.into_error()));
            }
        }else{
            if let Err(e) = read_ops_no_comp(&mut self.source, &header,&mut self.ops){
                return Some(Err(e));
            }
        }
        if !header.more_sections{
            self.done = true;
        }
        Some(Ok((self.ops.as_slice(),header)))
    }
    pub fn into_inner(self) -> R {
        self.source.into_inner()
    }
}