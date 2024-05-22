

use smdiff_common::{Copy, CopySrc, Format, Run, SectionHeader, MAX_WIN_SIZE};

use crate::{add::{make_add_runs, make_adds}, hash::{find_substring_in_src, find_substring_in_trgt, ChunkHashMap, HashCursor, MULTIPLICATVE}, run::handle_run, CopyScore, Op};



pub fn encode_one_section<'a>(src_dict: &ChunkHashMap, trgt_dict: &ChunkHashMap, src_bytes:&[u8], target: &'a [u8], hash_size:usize) -> (SectionHeader,Vec<Op<'a>>){
    assert!(target.len() <= MAX_WIN_SIZE);
    if target.is_empty(){
        return (SectionHeader{ num_operations: 0, num_add_bytes: 0, output_size: 0, compression_algo: 0, format: Format::Interleaved, more_sections: false },Vec::new());
    }
    // let mut src_scanner = Scanner::new(&src_bytes);
    // let mut trgt_scanner = if match_trgt{Scanner::new(&target)}else{Scanner::new(&[])};

    let mut rel_o = 0;
    let mut run_pos = 0;
    let mut copy_pos = 0;
    let mut add_bytes = 0;
    let max_end_pos = target.len();
    let chunk = &target;
    let mut last_d_addr=0 ; //zeroed for each window
    let mut last_o_addr=0 ;
    let mut ops: Vec<Op> = Vec::new();
    let mut trgt_hashes = HashCursor::new(chunk, hash_size as u32, MULTIPLICATVE);

    // let mut miss_avg = AverageDuration::new();
    // let mut hit_avg = AverageDuration::new();
    // let mut run_avg = AverageDuration::new();
    loop{
        // if true  {
        //     dbg!(ops.last());
        //     println!("rel_o {} run_pos {} copy_pos {} last_d {} %: {}",rel_o,run_pos,copy_pos, last_d_addr,run_pos as f32/max_end_pos as f32);
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
        let cur_hash = trgt_hashes.seek(copy_pos).unwrap();
        let src_match = find_substring_in_src(&src_bytes,&src_dict, &chunk,cur_hash, hash_size as usize, copy_pos,last_d_addr);
        let trgt_match = find_substring_in_trgt(&trgt_dict, &target, cur_hash, hash_size as usize, copy_pos,last_o_addr);
        // src_match.or_else(||src_scanner.scan(&chunk[rel_o..max_end_pos.min(rel_o+hash_size)],usize::MAX));
        // if src_match.is_none() {
        //     trgt_match.or_else(||trgt_scanner.scan(&chunk[rel_o..max_end_pos.min(rel_o+hash_size)],rel_o));
        // }
        if src_match.is_some() || trgt_match.is_some() {
            assert!(run_pos>=rel_o);
            assert_eq!(run_pos,copy_pos);
            if copy_pos > rel_o{
                make_adds(&chunk[rel_o..copy_pos], &mut ops,&mut add_bytes);
                //dbg!(add_bytes);
                rel_o = copy_pos;
            }
            let use_trgt = trgt_match > src_match;
            if use_trgt{
                let CopyScore { size, start,.. } = trgt_match.unwrap();
                let start = start as u64;
                let len = size as u16;
                rel_o += size;
                copy_pos = rel_o;
                run_pos = rel_o;
                last_o_addr = start;
                //println!("Using Trgt match, len: {} start: {}",size,start);
                ops.push(Op::Copy(Copy{src: CopySrc::Output, addr: last_o_addr, len}));
                //let dur = start_loop.elapsed();
                //hit_avg.add_sample(dur);
            }else{
                let CopyScore { size, start,.. }= src_match.unwrap();
                let start = start as u64;
                let len = size as u16;
                rel_o += size;
                copy_pos = rel_o;
                run_pos = rel_o;
                last_d_addr = start;
                //println!("Using Src match, len: {} start: {}",size,start);
                ops.push(Op::Copy(Copy{src: CopySrc::Dict, addr: last_d_addr, len}));
                //let dur = start_loop.elapsed();
                //hit_avg.add_sample(dur);
            }
            continue;
        }else{
            //let dur = start_loop.elapsed();
            //miss_avg.add_sample(dur);
            copy_pos += 1 as usize;
        }
    }
    if rel_o < max_end_pos{
        //we need to emit the last add op
        make_add_runs(&chunk[rel_o..], &mut ops,&mut add_bytes);
        rel_o = max_end_pos;
    }

    let header = SectionHeader{
        num_operations: ops.len() as u32,
        output_size: rel_o as u32,
        num_add_bytes: add_bytes as u32,
        compression_algo: 0,
        format: Format::Interleaved,
        more_sections: false,

    };
    (header,ops)
}