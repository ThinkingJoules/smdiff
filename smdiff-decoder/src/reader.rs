//! This module contains the `SectionReader` struct, which is used to read sections from a smdiff delta file.
//! This is just like the `SectionIterator` struct from the `smdiff-reader` crate, but this can read sections that have secondary compression.
use std::io::{BufReader, Cursor, Read, Seek};

use smdiff_common::SectionHeader;
use smdiff_reader::{read_ops_no_comp, read_section_header, Op};

use crate::apply_no_sec_comp;

/// A reader that will keep reading sections until it reaches the terminal section.
pub struct SectionIterator<R>{
    source: BufReader<R>,
    done:bool,
    win_data: Vec<u8>,
    ops: Vec<Op>,
}
impl<R: Read+Seek> SectionIterator<R>{
    pub fn new(patch: R) -> Self {
        Self {
            source:BufReader::new(patch),
            done:false,
            win_data: Vec::new(),
            ops: Vec::new(),
        }
    }
    /// Reads and returns the next section (if it exists).
    ///
    /// This is useful if you don't need the Ops, just need to read them.
    pub fn next_borrowed(&mut self) -> Option<std::io::Result<(&[Op],SectionHeader)>>{
        let mut header = match self.read_header()?{
            Ok(h) => h,
            Err(e) => {
                return Some(Err(e));
            }
        };
        self.ops.clear();
        if let Err(e) = self.read_ops(&mut header){
            return Some(Err(e));
        }
        Some(Ok((&self.ops,header)))
    }
    ///In the event the caller needs to do something to the ops (more than just read them), this avoids the need to clone the slice.
    fn next_owned(&mut self) -> Option<std::io::Result<(Vec<Op>,SectionHeader)>>{
        let mut header = match self.read_header()?{
            Ok(h) => h,
            Err(e) => {
                return Some(Err(e));
            }
        };
        self.ops.clear();
        if let Err(e) = self.read_ops(&mut header){
            return Some(Err(e));
        }
        Some(Ok((std::mem::take(&mut self.ops),header)))
    }
    fn read_header(&mut self) -> Option<std::io::Result<SectionHeader>>{
        if self.done{
            return None;
        }
        match read_section_header(&mut self.source){
            Ok(h) => Some(Ok(h)),
            Err(e) => {
                return Some(Err(e));
            }
        }
    }
    fn read_ops(&mut self,header:&mut SectionHeader) -> std::io::Result<()>{
        self.win_data.clear();
        self.ops.clear();
        if header.compression_algo == 1 {
            let mut crsr = Cursor::new(&mut self.win_data);
            apply_no_sec_comp::<_,R,_>(&mut self.source, None, &mut crsr)?;
            crsr.rewind()?;
            read_ops_no_comp(&mut crsr, header,&mut self.ops)?;
        }else if header.compression_algo == 2 {
            let mut zstd = ruzstd::StreamingDecoder::new(&mut self.source).unwrap();
            read_ops_no_comp(&mut zstd, header,&mut self.ops)?;
        }else if header.compression_algo == 3 {
            let mut brot = brotlic::DecompressorReader::new(&mut self.source);
            read_ops_no_comp(&mut brot, header,&mut self.ops)?;
            let _ = brot.into_inner()?;
        }else{
            read_ops_no_comp(&mut self.source, header,&mut self.ops)?;
        }
        if !header.more_sections{
            self.done = true;
        }
        Ok(())
    }
    pub fn into_inner(self) -> R {
        self.source.into_inner()
    }
}

impl<R: Read+Seek> Iterator for SectionIterator<R> {
    type Item = std::io::Result<(Vec<Op>, SectionHeader)>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_owned()
    }
}