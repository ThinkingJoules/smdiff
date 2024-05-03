
use std::cmp::Ordering;

use smdiff_common::{AddOp, FileHeader, Format, MAX_WIN_SIZE, MICRO_MAX_INST_COUNT};
use smdiff_writer::{write_file_header, write_micro_section, write_win_section};

use crate::{add::{make_add_runs, make_adds}, hash::{find_sub_string_in_src, hash_chunk, ChunkHashMap, HashCursor, MULTIPLICATVE}, micro_encoder::encode_one_section, run::handle_run, window_encoder::{encode_window, encode_window_min}};



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
use smdiff_common::{Copy, CopySrc, Run, WindowHeader};

pub fn encode<R: std::io::Read+std::io::Seek, W: std::io::Write>(src: &mut R, trgt: &mut R, writer: &mut W,match_trgt:bool) -> std::io::Result<()> {
    //to test we just read all of src and trgt in to memory.
    let mut src_bytes = Vec::new();
    src.read_to_end(&mut src_bytes)?;
    let mut trgt_bytes = Vec::new();
    trgt.read_to_end(&mut trgt_bytes)?;
    let start = std::time::Instant::now();
    dbg!(src_bytes.len()+trgt_bytes.len());
    if trgt_bytes.len() > MAX_WIN_SIZE {
        //MUST be window format
        //We need to chunk the trgt bytes in max_win_size chunks and encode them.
        write_file_header(&FileHeader{compression_algo:0,format:Format::WindowFormat}, writer)?;
        let num_windows = (trgt_bytes.len()+MAX_WIN_SIZE - 1)/MAX_WIN_SIZE;
        let win_size = (trgt_bytes.len() / num_windows) + 1; //+1 to ensure last window will get the last bytes

        let hash_size = 16;
        let tab_size = 16777215;
        let start_dict_creation = std::time::Instant::now();
        let mut src_chunks = Vec::new();
        for (num,chunk) in src_bytes.chunks(win_size).enumerate(){
            let abs_start_pos = (num * win_size) as u32;
            let map = hash_chunk(chunk, abs_start_pos,hash_size, MULTIPLICATVE, tab_size);
            dbg!(map.num_hashes());
            src_chunks.push(map);
        }
        let mut trgt_chunks = Vec::new();
        if match_trgt{
            for (num,chunk) in trgt_bytes.chunks(win_size).enumerate(){
                let abs_start_pos = (num * win_size) as u32;
                let map = hash_chunk(chunk, abs_start_pos,hash_size, MULTIPLICATVE, tab_size);
                dbg!(map.num_hashes());
                trgt_chunks.push(map);
            }
        }
        let dict_creation_dur = start_dict_creation.elapsed();
        println!("Dict creation took: {:?} size: {} (mb/s: {})", dict_creation_dur,src_bytes.len(), (src_bytes.len())as f64 / 1024.0 / 1024.0 / dict_creation_dur.as_secs_f64());
        let trgt_bytes = trgt_bytes.as_slice();
        for (chunk_num,chunk) in trgt_bytes.chunks(win_size).enumerate() {
            let win_start = chunk_num * win_size;
            let win_end = win_start + chunk.len();
            let (header,ops) = encode_window(&src_chunks,&trgt_chunks, &src_bytes, &trgt_bytes, win_start..win_end, hash_size as usize);
            dbg!(header);
            write_win_section(&ops,header,writer)?;
            println!("% done: {}, elapsed so far: {:?}",((chunk_num+1) as f64 / num_windows as f64)*100.0,start.elapsed());
        }
    }else{
        //MIGHT be window format
        let hash_size = 3;
        let start_dict_creation = std::time::Instant::now();
        let src_dict = hash_chunk(&src_bytes, 0,hash_size, MULTIPLICATVE, src_bytes.len() as u32);
        let trgt_dict = if match_trgt{
            hash_chunk(&trgt_bytes, 0,hash_size, MULTIPLICATVE,trgt_bytes.len() as u32)
        }else{
            ChunkHashMap::new(0)
        };
        let dict_creation_dur = start_dict_creation.elapsed();
        println!("Dict creation took: {:?} size: {} (mb/s: {})", dict_creation_dur,src_bytes.len(), (src_bytes.len())as f64 / 1024.0 / 1024.0 / dict_creation_dur.as_secs_f64());

        let (header,ops) = encode_one_section(&src_dict,&trgt_dict, &src_bytes, &trgt_bytes, hash_size as usize);
        let format = if header.num_operations as usize <= MICRO_MAX_INST_COUNT {Format::MicroFormat{num_operations:header.num_operations as u8}} else {Format::WindowFormat};
        write_file_header(&FileHeader{compression_algo:0,format}, writer)?;
        if matches!(format, Format::MicroFormat{..}) {
            write_micro_section(&ops,writer)?;
        } else {
            write_win_section(&ops,header,writer)?;
        }
    }

    Ok(())
}

pub fn _min_encode<R: std::io::Read+std::io::Seek, W: std::io::Write>(src: &mut R, trgt: &mut R, writer: &mut W,match_trgt:bool) -> std::io::Result<()> {
    //to test we just read all of src and trgt in to memory.
    let mut src_bytes = Vec::new();
    src.read_to_end(&mut src_bytes)?;
    let mut trgt_bytes = Vec::new();
    trgt.read_to_end(&mut trgt_bytes)?;
    let start = std::time::Instant::now();

    if trgt_bytes.len() > MAX_WIN_SIZE {
        //MUST be window format
        //We need to chunk the trgt bytes in max_win_size chunks and encode them.
        write_file_header(&FileHeader{compression_algo:0,format:Format::WindowFormat}, writer)?;
        let num_windows = (trgt_bytes.len()+MAX_WIN_SIZE - 1)/MAX_WIN_SIZE;
        let output_win_size = (trgt_bytes.len() / num_windows) + 1; //+1 to ensure last window will get the last bytes
        let scaled_src_win_size = (src_bytes.len() / num_windows) + 1;
        //let start_dict_hashing = std::time::Instant::now();
        //let mut dict_hashes = Vec::with_capacity(num_windows);

        let hash_size = 16;
        //let tab_size = 16777215;
        // let start_dict_creation = std::time::Instant::now();
        // let mut src_chunks = Vec::new();
        // for (num,chunk) in src_bytes.chunks(win_size).enumerate(){
        //     let abs_start_pos = (num * win_size) as u32;
        //     let map = hash_chunk(chunk, abs_start_pos,hash_size, MULTIPLICATVE, tab_size);
        //     dbg!(map.num_hashes());
        //     src_chunks.push(map);
        // }
        // let mut trgt_chunks = Vec::new();
        // if match_trgt{
        //     for (num,chunk) in trgt_bytes.chunks(win_size).enumerate(){
        //         let abs_start_pos = (num * win_size) as u32;
        //         let map = hash_chunk(chunk, abs_start_pos,hash_size, MULTIPLICATVE, tab_size);
        //         dbg!(map.num_hashes());
        //         trgt_chunks.push(map);
        //     }
        // }
        // let dict_creation_dur = start_dict_creation.elapsed();
        // println!("Dict creation took: {:?} size: {} (mb/s: {})", dict_creation_dur,src_bytes.len(), (src_bytes.len())as f64 / 1024.0 / 1024.0 / dict_creation_dur.as_secs_f64());
        let trgt_bytes = trgt_bytes.as_slice();

        for (chunk_num,chunk) in trgt_bytes.chunks(output_win_size).enumerate() {
            let win_start = chunk_num * output_win_size;
            let win_end = win_start + chunk.len();
            let start_dict_creation = std::time::Instant::now();
            let look_back = if match_trgt{1}else{2};
            let look_forward = if match_trgt{2}else{3};
            let src_start = ((chunk_num as isize - look_back)*scaled_src_win_size as isize).max(0) as usize;
            let src_end = ((chunk_num + look_forward)*scaled_src_win_size).min(src_bytes.len());
            dbg!(src_start,src_end,chunk_num);
            let src_dict = hash_chunk(&src_bytes[src_start..src_end], src_start as u32, hash_size,MULTIPLICATVE, (src_end-src_start) as u32);
            //we want trgt to trail itself by the last two windows
            //chunk_num-2*win_size..chunk_num*win_size but we need to ensure we don't go negative
            let trgt_start = if match_trgt {(chunk_num as i32 - 2*output_win_size as i32).max(0) as usize}else{0};
            let trgt_end = if match_trgt{chunk_num*output_win_size}else{0};
            let trgt_dict = hash_chunk(&trgt_bytes[trgt_start..trgt_end], trgt_start as u32, hash_size,MULTIPLICATVE, (trgt_end-trgt_start) as u32);
            let dict_creation_dur = start_dict_creation.elapsed();
            println!("Dict creation took: {:?} size: {} (mb/s: {})", dict_creation_dur,(trgt_end-trgt_start)+(src_end-src_start), ((trgt_end-trgt_start)+(src_end-src_start))as f64 / 1024.0 / 1024.0 / dict_creation_dur.as_secs_f64());
            //let (header,ops) = encode_window(&src_chunks,&trgt_chunks, &src_bytes, &trgt_bytes, win_start..win_end, hash_size as usize);
            let (header,ops) = encode_window_min(&src_dict,&trgt_dict, &src_bytes, &trgt_bytes, win_start..win_end, hash_size as usize);
            dbg!(header);
            write_win_section(&ops,header,writer)?;
            println!("% done: {}, elapsed so far: {:?}",((chunk_num+1) as f64 / num_windows as f64)*100.0,start.elapsed());
        }
    }else{
        //MIGHT be window format
        unimplemented!();
        // let (header,ops) = encode_window(&dict, 0, &trgt_bytes, 0, 2);
        // let format = if header.num_operations as usize <= MICRO_MAX_INST_COUNT {Format::MicroFormat{num_operations:header.num_operations as u8}} else {Format::WindowFormat};
        // write_file_header(&FileHeader{compression_algo:0,format}, writer)?;
        // if matches!(format, Format::MicroFormat{..}) {
        //     write_micro_section(&ops,writer)?;
        // } else {
        //     write_win_section(&ops,header,writer)?;
        // }
    }

    Ok(())
}
fn _encode_inlined<R: std::io::Read+std::io::Seek, W: std::io::Write>(src: &mut R, trgt: &mut R, writer: &mut W,_match_trgt:bool) -> std::io::Result<()> {
    //to test we just read all of src and trgt in to memory.
    let mut src_bytes = Vec::new();
    src.read_to_end(&mut src_bytes)?;
    let mut trgt_bytes = Vec::new();
    trgt.read_to_end(&mut trgt_bytes)?;
    let start = std::time::Instant::now();

    if trgt_bytes.len() > MAX_WIN_SIZE {
        //MUST be window format
        //We need to chunk the trgt bytes in max_win_size chunks and encode them.
        write_file_header(&FileHeader{compression_algo:0,format:Format::WindowFormat}, writer)?;
        let num_windows = (trgt_bytes.len()+MAX_WIN_SIZE - 1)/MAX_WIN_SIZE;
        let win_size = (trgt_bytes.len() / num_windows) + 1; //+1 to ensure last window will get the last bytes
        let hash_size = 16;
        let multiplicative = 16777213;
        let tab_size = 16777259;
        let start_dict_creation = std::time::Instant::now();
        let mut src_chunks = Vec::new();
        for (num,chunk) in src_bytes.chunks(win_size).enumerate(){
            let abs_start_pos = (num * win_size) as u32;
            let map = hash_chunk(chunk, abs_start_pos,hash_size, multiplicative, tab_size);
            dbg!(map.num_hashes());
            src_chunks.push(map);
        }
        let dict_creation_dur = start_dict_creation.elapsed();
        println!("Dict creation took: {:?} size: {} (mb/s: {})", dict_creation_dur,src_bytes.len(), (src_bytes.len())as f64 / 1024.0 / 1024.0 / dict_creation_dur.as_secs_f64());
        let trgt_bytes = trgt_bytes.as_slice();
        for (chunk_num,chunk) in trgt_bytes.chunks(win_size).enumerate() {

            //let trgt_hashes = hash_chunk(chunk, hash_size as u32, multiplicative, 1<<24);
            let mut trgt_hashes = HashCursor::new(chunk, hash_size as u32, multiplicative);
            // let mut trgt_hash = RollingHash::new(&chunk[..hash_size as usize],multiplicative,1<<24);
            // trgt_hashes.insert(rolling_hash.hash(), 0);
            let mut rel_o = 0;
            let mut run_pos = 0;
            let mut copy_pos = 0;
            let mut add_bytes = 0;
            let max_end_pos = chunk.len();
            let mut last_d_addr = 0; //zeroed for each window
            let mut _last_o_addr = 0;
            let mut ops: Vec<Op> = Vec::new();
            // let mut miss_avg = AverageDuration::new();
            // let mut hit_avg = AverageDuration::new();
            // let mut run_avg = AverageDuration::new();
            loop{
                // if chunk_num == 0 && run_pos < 500  {
                //     dbg!(ops.last());
                //     println!("rel_o {} run_pos {} copy_pos {} last_d {} %: {}",rel_o,run_pos,copy_pos, last_d_addr,run_pos as f32/win_size as f32);
                // }else{panic!()}

                //let start_loop = std::time::Instant::now();
                if run_pos.max(copy_pos) >= max_end_pos {
                    break;
                }
                if copy_pos+hash_size as usize >= max_end_pos{
                    //we need to emit the last add op
                    //copy_pos = max_end_pos;
                    break;
                }
                if let Some((byte,mut len)) = handle_run(&chunk[run_pos..]){
                    assert!(run_pos <= copy_pos);
                    if run_pos > rel_o{//emit non-run, non-matched bytes
                        make_adds(&chunk[rel_o..run_pos], &mut ops,&mut add_bytes);
                        //dbg!(add_bytes);
                        assert!(add_bytes<=win_size);
                        rel_o = run_pos;
                    }
                    assert_eq!(run_pos,rel_o);
                    //dbg!(byte,len);
                    //we need to chop this len up to max 62 len long per op
                    while len > 62 {
                        ops.push(Op::Run(Run{byte, len: 62}));
                        rel_o += 62;
                        len -= 62;
                    }
                    ops.push(Op::Run(Run{byte, len: len as u8}));
                    rel_o += len;
                    run_pos = rel_o;
                    copy_pos = copy_pos.max(rel_o); //our run might have ran in to our next copy start
                    //run_avg.add_sample(start_loop.elapsed());
                    continue;
                }
                if run_pos < copy_pos {//try another run before we get to our next copy
                    run_pos += 1;
                    continue;
                }
                //if we are here run_pos == copy_pos
                assert_eq!(run_pos,copy_pos);
                if let Some(CopyScore { size, start,.. }) = find_sub_string_in_src(&src_bytes,&src_chunks, &chunk,trgt_hashes.seek(copy_pos).unwrap(), hash_size as usize, copy_pos,last_d_addr){
                    assert!(run_pos>=rel_o);
                    assert_eq!(run_pos,copy_pos);

                    if copy_pos > rel_o{
                        make_adds(&chunk[rel_o..copy_pos], &mut ops,&mut add_bytes);
                        //dbg!(add_bytes);
                        assert!(add_bytes<=win_size);
                        rel_o = copy_pos;
                    }
                    let start = start as u64;
                    let len = size as u16;
                    //trgt_scanner.update_pos(start);
                    rel_o += size;
                    copy_pos = rel_o;
                    run_pos = rel_o;
                    last_d_addr = start as u64;
                    //println!("Using Src match, len: {} start: {}",size,start);
                    ops.push(Op::Copy(Copy{src: CopySrc::Dict, addr: last_d_addr, len}));
                    //let dur = start_loop.elapsed();
                    //hit_avg.add_sample(dur);
                    continue;
                }else{
                    //let dur = start_loop.elapsed();
                    //miss_avg.add_sample(dur);
                    copy_pos += hash_size as usize;
                }
            }
            if rel_o < max_end_pos{
                //we need to emit the last add op
                make_add_runs(&chunk[rel_o..], &mut ops,&mut add_bytes);
                assert!(add_bytes<=win_size, "Add bytes: {} Win Size: {} rel_o",add_bytes,win_size);
                rel_o = max_end_pos;
            }
            let header = WindowHeader{
                num_operations: ops.len() as u32,
                output_size: rel_o as u32,
                num_add_bytes: add_bytes as u32,
            };

            //dbg!(header,miss_avg.average(),hit_avg.average(),run_avg.average());
            write_win_section(&ops,header,writer)?;
            println!("% done: {}, elapsed so far: {:?}",((chunk_num+1) as f64 / num_windows as f64)*100.0,start.elapsed());
        }
    }else{
        //MIGHT be window format
        unimplemented!();
        // let (header,ops) = encode_window(&dict, 0, &trgt_bytes, 0, 2);
        // let format = if header.num_operations as usize <= MICRO_MAX_INST_COUNT {Format::MicroFormat{num_operations:header.num_operations as u8}} else {Format::WindowFormat};
        // write_file_header(&FileHeader{compression_algo:0,format}, writer)?;
        // if matches!(format, Format::MicroFormat{..}) {
        //     write_micro_section(&ops,writer)?;
        // } else {
        //     write_win_section(&ops,header,writer)?;
        // }
    }

    Ok(())
}

// struct AverageDuration {
//     total_duration: Duration,
//     count: usize,
// }

// impl AverageDuration {
//     /// Constructs a new `AverageDuration`.
//     pub fn new() -> Self {
//         AverageDuration {
//             total_duration: Duration::new(0, 0),
//             count: 0,
//         }
//     }

//     /// Adds a duration to the running total and increments the count.
//     pub fn add_sample(&mut self, duration: Duration) {
//         self.total_duration += duration;
//         self.count += 1;
//     }

//     /// Returns the current average of durations.
//     pub fn average(&self) -> Option<Duration> {
//         if self.count > 0 {
//             Some(self.total_duration / self.count as u32)
//         } else {
//             None
//         }
//     }
// }



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