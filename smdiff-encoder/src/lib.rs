
use std::cmp::Ordering;

use smdiff_common::{AddOp, Format, MAX_WIN_SIZE};
use smdiff_writer::{write_section_header, write_ops};

use crate::{hash::{hash_chunk, ChunkHashMap, MULTIPLICATVE}, micro_encoder::encode_one_section, window_encoder::encode_window};



mod run;
mod add;
mod window_encoder;
mod micro_encoder;
//mod scanner;
mod hash;

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

pub fn encode<R: std::io::Read+std::io::Seek, W: std::io::Write>(src: &mut R, trgt: &mut R, writer: &mut W,match_trgt:bool,max_copy_step_size:u8,sec_comp:bool,format:Format) -> std::io::Result<()> {
    //to test we just read all of src and trgt in to memory.
    let mut src_bytes = Vec::new();
    src.read_to_end(&mut src_bytes)?;
    let mut trgt_bytes = Vec::new();
    trgt.read_to_end(&mut trgt_bytes)?;
    //let start = std::time::Instant::now();
    //dbg!(src_bytes.len()+trgt_bytes.len());
    let mut win_data = Vec::new();
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
        if match_trgt{
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
            let (mut header,ops) = encode_window(&src_chunks,&trgt_chunks, &src_bytes, &trgt_bytes, win_start..win_end, hash_size as usize,max_copy_step_size);
            output_tot += header.output_size;
            header.format = format;
            header.more_sections = chunk_num < num_windows - 1;

            //dbg!(header);
            if sec_comp {
                header.compression_algo = 1;
                write_section_header(&header, writer)?;
                write_ops(&ops,&header,&mut win_data)?;
                let mut crsr = std::io::Cursor::new(&mut win_data);
                encode(&mut std::io::Cursor::new(&mut Vec::new()), &mut crsr, writer, true, 1,false,Format::Interleaved)?;
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
        let trgt_dict = if match_trgt{
            hash_chunk(&trgt_bytes, 0,hash_size, MULTIPLICATVE,trgt_bytes.len() as u32)
        }else{
            ChunkHashMap::new(0)
        };
        //let dict_creation_dur = start_dict_creation.elapsed();
        //println!("Dict creation took: {:?} size: {} (mb/s: {})", dict_creation_dur,src_bytes.len(), (src_bytes.len())as f64 / 1024.0 / 1024.0 / dict_creation_dur.as_secs_f64());

        let (mut header,ops) = encode_one_section(&src_dict,&trgt_dict, &src_bytes, &trgt_bytes, hash_size as usize);
        header.format = format;
        if sec_comp {
            header.compression_algo = 1;
            write_section_header(&header, writer)?;
            write_ops(&ops,&header,&mut win_data)?;
            let mut crsr = std::io::Cursor::new(&mut win_data);
            encode(&mut std::io::Cursor::new(&mut Vec::new()), &mut crsr, writer, true, 1,false,Format::Interleaved)?;
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