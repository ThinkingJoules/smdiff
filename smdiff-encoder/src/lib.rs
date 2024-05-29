
use std::{cmp::Ordering, io::{Read, Write}};

use add::make_add_runs;
use encoder::EncoderConfig;
use smdiff_common::{write_u_varint, AddOp, Format, SectionHeader, MAX_INST_SIZE, MAX_WIN_SIZE};
use smdiff_writer::{write_section_header, write_ops};

use crate::{hash::{hash_chunk, ChunkHashMap, MULTIPLICATVE}, micro_encoder::encode_one_section, window_encoder::encode_window};



mod run;
mod add;
mod window_encoder;
mod micro_encoder;
//mod scanner;
mod hash;
mod hasher;
mod hashmap;
mod trgt_matcher;
mod src_matcher;
mod encoder;
pub mod zstd{
    pub use zstd::stream::Encoder;
}
pub mod brotli {
    pub use brotlic::{encode::{BrotliEncoderOptions,CompressorWriter},BlockSize,CompressionMode,Quality,WindowSize};
}

const MIN_RUN_LEN: usize = 3;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Add <'a> {
    bytes: &'a [u8],
}
impl AddOp for Add<'_> {
    fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}
pub type Op<'a> = smdiff_common::Op<Add<'a>>;

#[derive(Clone, Debug)]
pub enum SecondaryCompression {
    /// Value of 1..=16
    /// Value represents the number of bytes to advance after a copy match is not found.
    /// 1 will check for a match every byte, 2 every other byte, etc.
    /// Default Value: 4
    Smdiff{copy_miss_step:u8},
    /// Value of 1..=22.
    /// Default Value: 3
    Zstd{level:i32},
    /// Default Value: BrotliEncoderOptions::default()
    Brotli{options: ::brotlic::BrotliEncoderOptions},
}

impl SecondaryCompression {
    pub fn new_smdiff_default() -> Self {
        SecondaryCompression::Smdiff { copy_miss_step: 4 }
    }

    pub fn new_zstd_default() -> Self {
        SecondaryCompression::Zstd { level: 3 }
    }

    pub fn new_lzma_xz_default() -> Self {
        SecondaryCompression::Brotli { options: ::brotlic::BrotliEncoderOptions::default() }
    }
    pub fn algo_value(&self) -> u8 {
        match self {
            SecondaryCompression::Smdiff { .. } => 1,
            SecondaryCompression::Zstd { .. } => 2,
            SecondaryCompression::Brotli { .. } => 3,
        }
    }
}
impl Default for SecondaryCompression {
    fn default() -> Self {
        Self::new_zstd_default()
    }
}

/// Configuration for the encoder.
/// Default values are:
/// - copy_miss_step: 4
/// - sec_comp: None
/// - format: Interleaved
/// - match_target: false
#[derive(Clone, Debug)]
pub struct OldEncoderConfig {
    /// Value of 1..=16
    /// Value represents the number of bytes to advance after a copy match is not found.
    /// 1 will advance 1 byte and attempt to find another Copy match, 2 will advance 2 bytes, etc.
    /// Finding Copy matches is the most expensive operation in the encoding process.
    /// Lower values will take longer to encode but may reduce the delta file size.
    /// Default Value: 4
    pub m_step: u8,
    /// None for no secondary compression.
    /// Default Value: None
    pub sec_comp: Option<SecondaryCompression>,
    /// Whether to interleave or segregate the Add bytes.
    /// Default Value: Interleaved
    pub format: Format,
    /// Whether to use the output file as a dictionary against itself to find additional matches.
    /// This will increase the time to encode but may reduce the delta file size.
    /// Default Value: false
    pub match_target: bool,
}

impl OldEncoderConfig {
    pub fn new(match_target:bool,m_step: u8, sec_comp: Option<SecondaryCompression>, format: Format) -> Self {
        Self { m_step, sec_comp, format, match_target }
    }
    pub fn set_copy_miss_step(mut self, copy_miss_step: u8) -> Self {
        assert!(copy_miss_step > 0, "copy_miss_step must be greater than 0");
        self.m_step = copy_miss_step;
        self
    }
    pub fn set_sec_comp(mut self, sec_comp: SecondaryCompression) -> Self {
        self.sec_comp = Some(sec_comp);
        self
    }
    pub fn format_interleaved(mut self) -> Self {
        self.format = Format::Interleaved;
        self
    }
    pub fn format_segregated(mut self) -> Self {
        self.format = Format::Segregated;
        self
    }
    pub fn set_match_target(mut self, match_target: bool) -> Self {
        self.match_target = match_target;
        self
    }
}
impl Default for OldEncoderConfig {
    fn default() -> Self {
        Self {
            m_step: 4,
            sec_comp: None,
            format: Format::Interleaved,
            match_target: false,
        }
    }
}
pub fn test_encode<W: std::io::Write>(src: &[u8], trgt: &[u8], mut writer: &mut W) -> std::io::Result<()>  {
    let mut ops = Vec::new();
    let mut cur_len = 0;
    let mut num_add_bytes = 0;
    let segments = encoder::encode_inner(&EncoderConfig::default(), src, trgt);
    dbg!(segments.len());
    let mut cur_o_pos = 0;
    for segment in segments{
        match segment{
            encoder::InnerSegment::NoMatch{length}=>{
                let mut remaining_len = length;
                while remaining_len > 0 {
                    let max_len = remaining_len.min(MAX_WIN_SIZE as usize - cur_len);
                    make_add_runs(&trgt[cur_o_pos..cur_o_pos+max_len], &mut ops, &mut num_add_bytes);
                    cur_len += max_len;
                    if cur_len == MAX_WIN_SIZE{
                        let header = SectionHeader::new(ops.len() as u32, num_add_bytes as u32, cur_len as u32).set_more_sections(true);
                        dbg!(&header);
                        write_segment(&ops, &header, &mut writer)?;
                        ops.clear();
                        cur_len = 0;
                        num_add_bytes = 0;
                    }
                    cur_o_pos += max_len;
                    remaining_len -= max_len;
                }
            },
            encoder::InnerSegment::MatchSrc{start,length}=>{
                let mut remaining_len = length;
                let mut addr = start as u64;
                while remaining_len > 0 {
                    let max_len = remaining_len.min(MAX_WIN_SIZE as usize - cur_len).min(MAX_INST_SIZE);
                    let op = Op::Copy(smdiff_common::Copy{ src: smdiff_common::CopySrc::Dict, addr, len: max_len as u16 });
                    ops.push(op);
                    remaining_len -= max_len;
                    addr += max_len as u64;
                    cur_o_pos += max_len;
                    cur_len += max_len;
                    if cur_len == MAX_WIN_SIZE{
                        let header = SectionHeader::new(ops.len() as u32, num_add_bytes as u32, cur_len as u32).set_more_sections(true);
                        dbg!(&header);
                        write_segment(&ops, &header, &mut writer)?;
                        ops.clear();
                        cur_len = 0;
                        num_add_bytes = 0;
                    }
                }
            },
            encoder::InnerSegment::MatchTrgt{start,length}=>{
                unimplemented!()
            }
        }

    }
    write_segment(&ops, &SectionHeader::new(ops.len() as u32, num_add_bytes as u32, cur_len as u32).set_more_sections(false), &mut writer)?;

    Ok(())
}

fn write_segment<W: std::io::Write>(ops:&Vec<Op>,header:&SectionHeader, mut writer: &mut W)->std::io::Result<()> {
    write_section_header(&header, writer)?;
    write_ops(&ops,&header,writer)?;
    Ok(())
}
pub fn encode<R: std::io::Read+std::io::Seek, W: std::io::Write>(src: &mut R, trgt: &mut R, mut writer: &mut W,config:&OldEncoderConfig) -> std::io::Result<()> {
    //to test we just read all of src and trgt in to memory.
    let mut src_bytes = Vec::new();
    src.read_to_end(&mut src_bytes)?;
    let mut trgt_bytes = Vec::new();
    trgt.read_to_end(&mut trgt_bytes)?;
    //let start = std::time::Instant::now();
    //dbg!(src_bytes.len()+trgt_bytes.len());
    let mut win_data = Vec::new();
    let OldEncoderConfig { m_step: copy_miss_step, sec_comp, format, match_target } = config;
    if trgt_bytes.len() > MAX_WIN_SIZE {
        //Larger file
        let num_windows = (trgt_bytes.len()+MAX_WIN_SIZE - 1)/MAX_WIN_SIZE;
        let win_size = (trgt_bytes.len() / num_windows) + 1; //+1 to ensure last window will get the last bytes

        let hash_size = (win_size as u32/1024/1024).max(3); //win size in mb for hash size in bytes.
        let tab_size = 16777215;
        //let start_dict_creation = std::time::Instant::now();
        let mut src_chunks = Vec::new();
        //let mut dict_size = 0;
        for (num,chunk) in src_bytes.chunks(win_size).enumerate(){
            //dict_size += chunk.len();
            let abs_start_pos = (num * win_size) as u32;
            let src_map = hash_chunk(chunk, abs_start_pos,hash_size, MULTIPLICATVE, tab_size);
            //dbg!(src_map.num_hashes());
            src_chunks.push(src_map);
        }
        let mut trgt_chunks = Vec::new();
        if *match_target{
            for (num,chunk) in trgt_bytes.chunks(win_size).enumerate(){
                //dict_size += chunk.len();
                let abs_start_pos = (num * win_size) as u32;
                let trgt_map = hash_chunk(chunk, abs_start_pos,hash_size, MULTIPLICATVE, tab_size);
                //dbg!(trgt_map.num_hashes());
                trgt_chunks.push(trgt_map);
            }
        }
        //let dict_creation_dur = start_dict_creation.elapsed();
        //println!("Dict creation took: {:?} size: {} (mb/s: {}) hash_size:{}", dict_creation_dur,dict_size, (dict_size)as f64 / 1024.0 / 1024.0 / dict_creation_dur.as_secs_f64(),hash_size);
        let trgt_bytes = trgt_bytes.as_slice();
        let mut output_tot = 0;
        for (chunk_num,chunk) in trgt_bytes.chunks(win_size).enumerate() {
            let win_start = chunk_num * win_size;
            let win_end = win_start + chunk.len();
            let (mut header,ops) = encode_window(&src_chunks,&trgt_chunks, &src_bytes, &trgt_bytes, win_start..win_end, hash_size as usize,*copy_miss_step);
            output_tot += header.output_size;
            header = header.set_format(*format).set_more_sections(chunk_num < num_windows - 1);

            if sec_comp.is_some() {
                let comp = sec_comp.clone().unwrap();
                header.compression_algo = comp.algo_value();
                //dbg!(&header);
                write_section_header(&header, writer)?;
                write_ops(&ops,&header,&mut win_data)?;
                match comp{
                    SecondaryCompression::Smdiff { copy_miss_step } => {
                        let mut crsr = std::io::Cursor::new(&mut win_data);
                        let inner_config = OldEncoderConfig::new(true, copy_miss_step, None, Format::Interleaved);
                        encode(&mut std::io::Cursor::new(&mut Vec::new()), &mut crsr, writer, &inner_config)?;
                    },
                    SecondaryCompression::Zstd { level } => {
                        let mut a = ::zstd::Encoder::new(writer, level)?;
                        a.set_pledged_src_size(Some(win_data.len() as u64))?;
                        a.include_contentsize(true)?;
                        a.write_all(&win_data)?;
                        writer = a.finish()?;
                    },
                    SecondaryCompression::Brotli { mut options }=> {
                        options.size_hint(win_data.len() as u32);
                        let mut a = ::brotlic::CompressorWriter::with_encoder(options.build().unwrap(), writer);
                        a.write_all(&win_data)?;
                        writer = a.into_inner()?;
                    },
                }
                win_data.clear();
            }else{
                write_section_header(&header, writer)?;
                write_ops(&ops,&header,writer)?;
            }
            //println!("% done: {}, elapsed so far: {:?}",((chunk_num+1) as f64 / num_windows as f64)*100.0,start.elapsed());
        }
        assert_eq!(output_tot as usize,trgt_bytes.len());
    }else{
        let hash_size = 3;
        //let start_dict_creation = std::time::Instant::now();
        let src_dict = hash_chunk(&src_bytes, 0,hash_size, MULTIPLICATVE, src_bytes.len() as u32);
        let trgt_dict = if *match_target{
            hash_chunk(&trgt_bytes, 0,hash_size, MULTIPLICATVE,trgt_bytes.len() as u32)
        }else{
            ChunkHashMap::new(0)
        };
        //let dict_creation_dur = start_dict_creation.elapsed();
        //println!("Dict creation took: {:?} size: {} (mb/s: {})", dict_creation_dur,src_bytes.len(), (src_bytes.len())as f64 / 1024.0 / 1024.0 / dict_creation_dur.as_secs_f64());

        let (mut header,ops) = encode_one_section(&src_dict,&trgt_dict, &src_bytes, &trgt_bytes, hash_size as usize);
        header.format = *format;
        if sec_comp.is_some() {
            let comp = sec_comp.clone().unwrap();
            header.compression_algo = comp.algo_value();
            write_section_header(&header, writer)?;
            write_ops(&ops,&header,&mut win_data)?;
            match comp{
                SecondaryCompression::Smdiff { copy_miss_step } => {
                    let mut crsr = std::io::Cursor::new(&mut win_data);
                    let inner_config = OldEncoderConfig::new(true, copy_miss_step, None, Format::Interleaved);
                    encode(&mut std::io::Cursor::new(&mut Vec::new()), &mut crsr, writer, &inner_config)?;
                },
                SecondaryCompression::Zstd { level } => {
                    let mut a = zstd::Encoder::new(writer, level)?;
                    a.set_pledged_src_size(Some(win_data.len() as u64))?;
                    a.include_contentsize(true)?;
                    a.write_all(&win_data)?;
                    writer = a.finish()?;
                },
                SecondaryCompression::Brotli { mut options }=> {
                    options.size_hint(win_data.len() as u32);
                    ::brotlic::CompressorWriter::with_encoder(options.build().unwrap(), writer).write_all(&win_data)?;
                },
            }
            win_data.clear();
        }else{
            write_section_header(&header, writer)?;
            write_ops(&ops,&header,writer)?;
        }
    }

    Ok(())
}




/// Returns the cost of the next address based on the absolute distance from `cur_addr`.
fn addr_cost(cur_addr: u64, next_addr: u64) -> isize {
    let mut diff = cur_addr as i64 - next_addr as i64;
    if diff<0 {
        diff += 1 //to get the correct range for negative values
    }
    let diff = diff.abs() as u64;

    match diff {
        0..=63 => -1,            // closest range
        64..=8191 => -2,         // second closest range
        8192..=1048575 => -3,    // third closest range
        1048576..=134217727 => -4, // fourth closest range
        _ => -5,                // beyond expected range
    }
}


#[derive(Copy, Clone, Debug, Default, Ord, Eq)]
struct CopyScore{
    pub score: isize,
    pub size: usize,
    pub addr_cost: isize,
    pub size_cost: isize,
    pub start: usize,
}
impl PartialEq for CopyScore {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score
            && self.size == other.size
            && self.addr_cost == other.addr_cost
            && self.size_cost == other.size_cost
    }
}

impl PartialOrd for CopyScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let score_cmp = self.score.partial_cmp(&other.score);
        if score_cmp != Some(Ordering::Equal) {
            return score_cmp;
        }

        let size_cmp = self.size.partial_cmp(&other.size);
        if size_cmp != Some(Ordering::Equal) {
            return size_cmp;
        }

        let addr_cost_cmp = self.addr_cost.partial_cmp(&other.addr_cost);
        if addr_cost_cmp != Some(Ordering::Equal) {
            return addr_cost_cmp;
        }

        let size_cost_cmp = self.size_cost.partial_cmp(&other.size_cost);
        if size_cost_cmp != Some(Ordering::Equal) {
            return size_cost_cmp;
        }

        Some(Ordering::Equal)
    }
}

impl CopyScore {
    fn new(addr_cost:isize, size:usize,start:usize)->Self{
        let size_cost = size_cost(size);
        let score = addr_cost.saturating_add(size_cost).saturating_add(size as isize).saturating_sub(-1);
        CopyScore{score,addr_cost,size_cost,size,start}
    }
    // fn compression_ratio(&self)->f64{
    //     self.size as f64 / ((self.addr_cost + self.size_cost -1)*-1) as f64
    // }
}
fn size_cost(size: usize) -> isize {
    -((size > 62) as isize) - (size > 317) as isize
}