

mod run;
mod copy;
mod hash;
mod suffix;

/*
Approach to generating delta operations.
Before we do anything, we need to determine how much dict data we can fit in the available memory.
This should be a compile time constant for available memory.
Once we know how many bytes we can fit in memory, we can start figuring out dict selection.

First find all the Runs in our Target File.
We then need to check for Copys from Src, for the first instruction in Output.
Ideally we can fit all of Src in memory in some sort of suffix structure. (Suffix Array?)
We need to track how much extra we have to start trying to keep track of the Output as well.
If not we should favor the most we can fit in memory at the start of the file.
We can then begin encoding the instructions.

    For each 'step' we need to determine what is the best next move.




Then we generate the Prefix Trie from the Source File (if it exists).
Then we generate a Prefix Trie from the Target File.
We then start at the beginning of the Target File
    We try to find the longest sequence of bytes that matches something extant.
    We need to make sure we only match up to the first Run instruction.
    Before checking the trees we need to make sure the distance we have might not already be best for an ADD op
    If we there is some distance (Add) would never be better, then..
    We check both tries and take the result that gives us the longest sequence.
    If we find a sequence we encode the Copy instruction.
    If we don't, or the sequence is too short, we encode an Add instruction.
    Continue until we reach the end of the target file.
*/
include!(concat!(env!("OUT_DIR"), "/memory_config.rs"));

use std::os::unix::process;

use smdiff_common::{diff_addresses_to_i64, u_varint_encode_size, zigzag_encode, Add, Copy, CopySrc, Op, Run, MAX_INST_SIZE};

use crate::{copy::{optimistic_add, scan_for_next_match}, suffix::SuffixArray};
const MIN_MATCH_BYTES: usize = 2; //two because we are trying to optimize for small files.

pub fn encode_diff(src: &[u8], target: &[u8])  {
    //src might be empty and we might disregrad it.
    let use_src = src.len() > MIN_MATCH_BYTES;
    //We assume that if we can fit the src and trgt in memory, the rest of our overhead is minimal

    //first we find all the runs in the target file.
    let mut trgt_runs = run::find_byte_runs(target);
    // we gen the prefix trie from the target
    let mut trgt_sa = SuffixArray::new(target);

    let mut src_sa = SuffixArray::new(src);

    //now we start at the beginning of the target file and start encoding.
    let mut cur_o_pos = 0;
    let mut last_d_addr = 0;
    trgt_runs.reverse(); //last should be the first run we would encounter.
    let mut next_run_start = trgt_runs.last().map(|(start,_,_)| *start).unwrap_or(target.len());
    let mut ops = Vec::new();
    let mut hint = scan_for_next_match(&target, &src_sa, &trgt_sa, 0, use_src);
    loop{
        if cur_o_pos >= target.len() {
            break;
        }
        let mut add = false;
        if cur_o_pos == next_run_start {
            //we are at a run, we need emit this op
            let (_,len,byte) = trgt_runs.pop().unwrap();
            cur_o_pos += len as usize;
            ops.push(Op::Run(Run{len,byte}));
            next_run_start = trgt_runs.last().map(|(start,_,_)| *start).unwrap_or(target.len());
            //out hint might be 'behind' since our run was applied
            //run always gets precedence as it is the smallest possible op.
            if hint.is_some() && hint.as_ref().unwrap().1 < cur_o_pos{
                hint = scan_for_next_match(&target, &src_sa, &trgt_sa, cur_o_pos, use_src);
            }
            continue;
        }

        //hint should be at or ahead of cur_o_pos
        //if it is at cur_o_pos we should emit the copy and update the hint.
        //if we are not yet to it, we should emit an add so the next iteration will emit the copy.

        if hint.is_some() && hint.as_ref().unwrap().1 == cur_o_pos{
            let res = hint.unwrap();

        }
        if hint.is_none(){
            //we *will* find a copy here
        }else{
            //use the hint to emit an add.
        }



        // if !optimistic_add(last_d_addr, cur_o_pos, next_run_start - cur_o_pos){
        //     //we need to check the tries
        //     let test_slice = &target[cur_o_pos..];
        //     let test_len = test_slice.len();
        //     let trgt_match = trgt_sa.search(test_slice);
        //     let src_match = if use_src {
        //         src_sa.search(&target[cur_o_pos..])
        //     }else{
        //         Err(None)
        //     };
        //     let use_trgt = valid_target(&trgt_match, test_len, cur_o_pos) &&
        //         use_trgt_result(&src_match, &trgt_match, last_d_addr, cur_o_pos as u64);
        //     if use_trgt {
        //         match trgt_match{
        //             Ok(o_addr) => {
        //                 if use_copy_o(cur_o_pos as u64, o_addr as u64, test_len){
        //                     cur_o_pos += test_len;
        //                     last_d_addr = o_addr as u64;
        //                     fit_copies(CopySrc::Output, o_addr as u64, test_len, &mut ops);
        //                 }else{
        //                     add = true;
        //                 }
        //             }
        //             Err(Some((match_len, start_pos))) => {
        //                 if use_copy_o(cur_o_pos as u64, start_pos as u64, match_len){
        //                     cur_o_pos += match_len;
        //                     last_d_addr = start_pos as u64;
        //                     fit_copies(CopySrc::Output, start_pos as u64, match_len, &mut ops);
        //                 }else{
        //                     add = true;
        //                 }
        //             },
        //             Err(None) => unreachable!()
        //         }
        //     }else if use_src{
        //         match src_match{
        //             Ok(d_addr) => {
        //                 if use_copy_d(last_d_addr, d_addr as u64, test_len){
        //                     cur_o_pos += test_len;
        //                     last_d_addr = d_addr as u64;
        //                     fit_copies(CopySrc::Dict, d_addr as u64, test_len, &mut ops);
        //                 }else{
        //                     add = true;
        //                 }
        //             },
        //             Err(Some((match_len, start_pos))) => {
        //                 if use_copy_d(last_d_addr, start_pos as u64, match_len){
        //                     cur_o_pos += match_len;
        //                     last_d_addr = start_pos as u64;
        //                     fit_copies(CopySrc::Dict, start_pos as u64, match_len, &mut ops);
        //                 }else{
        //                     add = true;
        //                 }
        //             },
        //             Err(None) => {
        //                 add = true;
        //             }
        //         }
        //     }else{
        //         add = true;
        //     }
        // }else{
        //     add = true;
        // }
        // if add {
        //     //first we need to look ahead to see if we can find the next copy or the next run.

        //     let len = next_run_start - cur_o_pos;
        //     let bytes = &target[cur_o_pos..];
        //     fit_adds(bytes, len, &mut ops);
        //     cur_o_pos = next_run_start;
        // }
        // //precedence: Run, Copy, Add
    }

    todo!()
}
fn fit_copies(src:CopySrc,addr:u64,len:usize,output: &mut Vec<Op>){
    //we need to make sure we don't have any copies that are too large.
    //if we do, we need to split them up.
    let mut remaining = len;
    loop{
        let chunk_size = remaining.min(MAX_INST_SIZE as usize);
        let op = Copy{src,addr,len:chunk_size as u16};
        remaining -= chunk_size;
        output.push(Op::Copy(op));
        if remaining == 0{
            break;
        }
    }
}
fn fit_adds(bytes: &[u8], len:usize, output: &mut Vec<Op>){
    let mut remaining = bytes.len();
    let mut processed = 0;
    loop{
        let chunk_size = remaining.min(MAX_INST_SIZE as usize);
        let op = Add{bytes: bytes[processed..processed+chunk_size].to_vec()};
        remaining -= chunk_size;
        processed += chunk_size;
        output.push(Op::Add(op));
        if remaining == 0{
            break;
        }
    }
}



fn calculate_inst_size(address_cost: u8, inst_len: u16) -> u32 {
    let size_indicator_cost = match inst_len {
        0..=62 => 0,
        63..=317 => 1,
        _ => 2,
    };
    let add_cost = if address_cost > 0 { 0 } else { inst_len };
    1 + size_indicator_cost + address_cost as u32 + add_cost as u32
}