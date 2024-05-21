
/*
There are two parts to delta encoding a large file:
 - The pre-selection of 'dictionary' data
 - Finding matches to generate Copy, otherwise emitting Add and Run operations

The latter part is basically identical for the micro encoder.
The former, is not needed for micro, as we consider the entire Src file as the dictionary.

This module has to do with the pre-selection of dictionary data.
It will also use a modified encoder that is faster and less fine-grained than the micro encoder.
*/

use std::ops::Range;

use smdiff_common::{Copy, CopySrc, Run, WindowHeader, MAX_INST_SIZE, MAX_WIN_SIZE};

use crate::{add::{make_add_runs, make_adds}, hash::{find_sub_string_in_src, find_sub_string_in_trgt, find_substring_in_src, find_substring_in_trgt, ChunkHashMap, HashCursor, MULTIPLICATVE}, run::handle_run, CopyScore, Op};

//Use with full src or trgt dict only
pub fn encode_window<'a>(src_dict: &[ChunkHashMap], trgt_dict: &[ChunkHashMap], src_bytes:&[u8], target: &'a [u8], window_range:Range<usize>,hash_size:usize,max_copy_step_size:u8) -> (WindowHeader,Vec<Op<'a>>){
    if target.is_empty(){
        return (WindowHeader{ num_operations: 0, num_add_bytes: 0, output_size: 0 },Vec::new());
    }
    let win_size = window_range.end - window_range.start;
    assert!(win_size <= MAX_WIN_SIZE);
    let mut rel_o = 0;
    let mut run_pos = 0;
    let mut copy_pos = 0;
    let mut add_bytes = 0;
    let max_end_pos = window_range.end-window_range.start;
    let trgt_abs_start = window_range.start;
    let chunk = &target[window_range];
    let mut last_d_addr = 0; //zeroed for each window
    let mut last_o_addr = 0;
    let mut ops: Vec<Op> = Vec::new();
    let mut trgt_hashes = HashCursor::new(chunk, hash_size as u32, MULTIPLICATVE);

    // let mut miss_avg = AverageDuration::new();
    // let mut hit_avg = AverageDuration::new();
    // let mut run_avg = AverageDuration::new();
    loop{
        // if run_pos < 500  {
        //     dbg!(ops.last());
        //     println!("rel_o {} run_pos {} copy_pos {} last_d {} %: {}",rel_o,run_pos,copy_pos, last_d_addr,run_pos as f32/win_size as f32);
        // }else{
        //     dbg!(ops.last());
        //     println!("rel_o {} run_pos {} copy_pos {} last_d {} %: {}",rel_o,run_pos,copy_pos, last_d_addr,run_pos as f32/win_size as f32);
        //     panic!()
        // }
        // if ops.iter().map(|op|op.oal() as usize).sum::<usize>() != rel_o{
        //     dbg!(ops_len,rel_o,ops.last());
        //     panic!();
        // }
        //let start_loop = std::time::Instant::now();
        if run_pos.max(copy_pos) >= max_end_pos {
            //dbg!(run_pos,copy_pos,max_end_pos,rel_o);
            break;
        }
        if copy_pos+hash_size as usize >= max_end_pos{
            //we need to emit the last add op
            //copy_pos = max_end_pos;
            //dbg!(copy_pos,hash_size,max_end_pos,rel_o);
            break;
        }
        if let Some((byte,mut len)) = handle_run(&chunk[run_pos..]){
            assert!(run_pos <= copy_pos);
            if run_pos > rel_o{//emit non-run, non-matchable bytes
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
        let cur_hash = trgt_hashes.seek(copy_pos).unwrap();
        let src_match = find_sub_string_in_src(&src_bytes,&src_dict, chunk,cur_hash, hash_size as usize, copy_pos,last_d_addr);
        let trgt_match = find_sub_string_in_trgt(&trgt_dict, &target, cur_hash, hash_size as usize, copy_pos+trgt_abs_start,last_o_addr);
        if src_match.is_some() || trgt_match.is_some() {
            assert!(run_pos>=rel_o);
            assert_eq!(run_pos,copy_pos);
            if copy_pos > rel_o{
                make_adds(&chunk[rel_o..copy_pos], &mut ops,&mut add_bytes);
                //dbg!(add_bytes);
                assert!(add_bytes<=win_size);
                rel_o = copy_pos;
            }
            assert_eq!(copy_pos,rel_o);
            let use_trgt = trgt_match > src_match;
            if use_trgt{
                let CopyScore { size, start,.. } = trgt_match.unwrap();
                assert!(size <= MAX_INST_SIZE);
                let start = start as u64;
                let len = if rel_o + size > max_end_pos{
                    (max_end_pos - rel_o) as u16
                }else{
                    size as u16
                };
                rel_o += len as usize;
                assert!(rel_o <= max_end_pos);
                copy_pos = rel_o;
                run_pos = rel_o;
                last_o_addr = start;
                //println!("Using Trgt match, len: {} start: {}",size,start);
                ops.push(Op::Copy(Copy{src: CopySrc::Output, addr: last_o_addr, len}));
                //let dur = start_loop.elapsed();
                //hit_avg.add_sample(dur);
            }else{
                let CopyScore { size, start,.. } = src_match.unwrap();
                assert!(size <= MAX_INST_SIZE);
                let start = start as u64;
                let len = if rel_o + size > max_end_pos{
                    (max_end_pos - rel_o) as u16
                }else{
                    size as u16
                };
                rel_o += len as usize;
                assert!(rel_o <= max_end_pos);
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
            copy_pos += hash_size.min(max_copy_step_size as usize);
            //copy_pos += 1;
        }
    }
    if rel_o < max_end_pos{
        //we need to emit the last add op
        make_add_runs(&chunk[rel_o..], &mut ops,&mut add_bytes);
        assert!(add_bytes<=win_size, "Add bytes: {} Win Size: {} rel_o",add_bytes,win_size);
        rel_o = max_end_pos;
    }
    assert_eq!(rel_o,max_end_pos);
    let header = WindowHeader{
        num_operations: ops.len() as u32,
        output_size: rel_o as u32,
        num_add_bytes: add_bytes as u32,
    };
    (header,ops)
}

//WIP Doesn't encode properly yet.
pub fn encode_window_min<'a>(src_dict: &ChunkHashMap, trgt_dict: &ChunkHashMap, src_bytes:&[u8], target: &'a [u8], window_range:Range<usize>,hash_size:usize) -> (WindowHeader,Vec<Op<'a>>){
    if target.is_empty(){
        return (WindowHeader{ num_operations: 0, num_add_bytes: 0, output_size: 0 },Vec::new());
    }
    let win_size = window_range.end - window_range.start;
    assert!(win_size <= MAX_WIN_SIZE);
    let mut rel_o = 0;
    let mut run_pos = 0;
    let mut copy_pos = 0;
    let mut add_bytes = 0;
    let max_end_pos = window_range.end-window_range.start;
    let chunk = &target[window_range];
    let mut last_d_addr = 0; //zeroed for each window
    let mut last_o_addr = 0;
    let mut ops: Vec<Op> = Vec::new();
    let mut trgt_hashes = HashCursor::new(chunk, hash_size as u32, MULTIPLICATVE);

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
        let cur_hash = trgt_hashes.seek(copy_pos).unwrap();
        let src_match = find_substring_in_src(&src_bytes,&src_dict, &chunk,cur_hash, hash_size as usize, copy_pos,last_d_addr);
        let trgt_match = find_substring_in_trgt(&trgt_dict, &target, cur_hash, hash_size as usize, copy_pos,last_o_addr);
        if src_match.is_some() || trgt_match.is_some() {
            assert!(run_pos>=rel_o);
            assert_eq!(run_pos,copy_pos);
            if copy_pos > rel_o{
                make_adds(&chunk[rel_o..copy_pos], &mut ops,&mut add_bytes);
                //dbg!(add_bytes);
                assert!(add_bytes<=win_size);
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
    (header,ops)
}


#[cfg(test)]
mod tests {
    use crate::{hash::hash_chunk, Add};

    use super::*;
    #[test]
    fn test_empty_files(){
        let target = [];
        let (_,ops) = encode_window(&[], &[],&[],&target,0..0,3,1);
        assert_eq!(ops, Vec::<Op>::new());
    }

    #[test]
    fn test_no_common(){
        let src = [1,2,3,4,5];
        let target = [6,7,8,9,10];
        let s_dict = &[hash_chunk(&src, 0,3, MULTIPLICATVE, 15)];
        let t_dict = &[hash_chunk(&target, 0,3, MULTIPLICATVE, 15)];
        let (_,ops) = encode_window(s_dict,t_dict,&src, &target,0..5,3,1);
        assert_eq!(ops, vec![Op::Add(Add{bytes: &target})]);
    }

    #[test]
    fn test_run(){
        let src = [1,2,3,4,5];
        let target = [1,1,1,1,1];
        let s_dict = &[hash_chunk(&src, 0,3, MULTIPLICATVE, 15)];
        let t_dict = &[hash_chunk(&target, 0,3, MULTIPLICATVE, 15)];
        let (_,ops) = encode_window(s_dict,t_dict,&src, &target,0..5,3,1);
        assert_eq!(ops, vec![Op::Run(Run{len: 5, byte: 1})]);
    }

    #[test]
    fn test_run_copy(){
        let src = [1,2,3,4,5];
        let target = [1,1,1,1,1,2,3,4,5];
        let s_dict = &[hash_chunk(&src, 0,3, MULTIPLICATVE, 15)];
        let t_dict = &[hash_chunk(&target, 0,3, MULTIPLICATVE, 15)];
        let (_,ops) = encode_window(s_dict,t_dict,&src, &target,0..9,3,1);
        assert_eq!(ops, vec![Op::Run(Run{len: 5, byte: 1}), Op::Copy(Copy{src: CopySrc::Dict, addr: 1, len: 4})]);
    }
    #[test]
    fn test_copy_no_src(){
        let src = [];
        let target = [1,2,3,4,5,6,1,2,3,4,5,6,6,2,3,4,5,7];
        let s_dict = &[hash_chunk(&src, 0,3, MULTIPLICATVE, 20)];
        let t_dict = &[hash_chunk(&target, 0,3, MULTIPLICATVE, 20)];
        let (_,ops) = encode_window(s_dict,t_dict,&src, &target,0..18,3,1);
        assert_eq!(ops, vec![
            Op::Add(Add{bytes: &target[0..6]}), //4
            Op::Copy(Copy { src: CopySrc::Output, addr: 0, len: 6 }), //2
            Op::Run(Run{len: 1, byte: 6}), //2
            Op::Copy(Copy { src: CopySrc::Output, addr: 1, len: 4 }), //2
            Op::Run(Run{len: 1, byte: 7}), //2
        ]);
    }
    #[test]
    fn test_copy(){
        let src = [1,2,3];
        let target = [1,2,3,4,5,6,1,2,3,4,5,6,6,2,3,4,5,7];
        let s_dict = &[hash_chunk(&src, 0,3, MULTIPLICATVE, 20)];
        let t_dict = &[hash_chunk(&target, 0,3, MULTIPLICATVE, 20)];
        let (_,ops) = encode_window(s_dict,t_dict,&src, &target,0..18,3,1);
        assert_eq!(ops, vec![
            Op::Copy(Copy{src: CopySrc::Dict, addr:0, len: 3}), //2
            Op::Add(Add{bytes: &target[3..6]}), //4
            Op::Copy(Copy { src: CopySrc::Output, addr: 0, len: 6 }), //2
            Op::Run(Run{len: 1, byte: 6}), //2
            Op::Copy(Copy { src: CopySrc::Output, addr: 1, len: 4 }), //2
            Op::Run(Run{len: 1, byte: 7}), //2
        ]);
    }
    #[test]
    fn test_copy_favor_src(){
        let src = [1,2,3,4,5,6];
        let target = [1,2,3,4,5,6,1,2,3,4,5,6,6,2,3,4,5,7];
        let s_dict = &[hash_chunk(&src, 0,3, MULTIPLICATVE, 20)];
        let t_dict = &[hash_chunk(&target, 0,3, MULTIPLICATVE, 20)];
        let (_,ops) = encode_window(s_dict,t_dict,&src, &target,0..18,3,1);
        assert_eq!(ops, vec![
            Op::Copy(Copy{src: CopySrc::Dict, addr:0, len: 6}), //2
            Op::Copy(Copy { src: CopySrc::Dict, addr: 0, len: 6 }), //2
            Op::Run(Run{len: 1, byte: 6}), //2
            Op::Copy(Copy { src: CopySrc::Dict, addr: 1, len: 4 }), //2
            Op::Run(Run{len: 1, byte: 7}), //2
        ]);
    }

    #[test]
    fn test_copy_larger_src(){
        let src = [1,2,3,4,5,6,1,2,3,4,5,6,6,2,3,4,5,7];
        let target = [1,2,3,4,5,6];
        let s_dict = &[hash_chunk(&src, 0,3, MULTIPLICATVE, 20)];
        let t_dict = &[hash_chunk(&target, 0,3, MULTIPLICATVE, 20)];
        let (_,ops) = encode_window(s_dict,t_dict,&src, &target,0..6,3,1);
        assert_eq!(ops, vec![
            Op::Copy(Copy{src: CopySrc::Dict, addr:0, len: 6}), //2
        ]);
    }
}