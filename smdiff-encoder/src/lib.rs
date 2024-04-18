

mod run;
mod copy;
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

use smdiff_common::{Add,CopySrc, Op, Run, MAX_INST_SIZE};

use crate::{copy::{find_certain_match, make_copy, scan_for_next_match, use_trgt_result, valid_target, NextMinMatch}, suffix::SuffixArray};
const MIN_MATCH_BYTES: usize = 2; //two because we are trying to optimize for small files.
const MIN_ADD_LEN: usize = 2; //we need to have at least 2 bytes to make an add instruction.
pub fn encode_diff(src: &[u8], target: &[u8]) -> Vec<Op>{

    //We assume that if we can fit the src and trgt in memory, the rest of our overhead is minimal
    //first we find all the runs in the target file.
    let mut trgt_runs = run::find_byte_runs(target);
    // we gen the prefix trie from the target
    let trgt_sa = SuffixArray::new(target);

    let src_sa = SuffixArray::new(src);

    //now we start at the beginning of the target file and start encoding.
    let mut cur_o_pos = 0;
    let max_end_pos = target.len() - MIN_MATCH_BYTES;
    let mut last_d_addr = 0;
    let mut last_o_addr = 0;
    trgt_runs.reverse(); //last should be the first run we would encounter.
    let mut next_run_start = trgt_runs.last().map(|(start,_,_)| *start).unwrap_or(target.len());
    let mut ops = Vec::new();
    let mut next_min_match = scan_for_next_match(&target, &src_sa, &trgt_sa, 0);
    loop{
        if cur_o_pos >= target.len() {
            break;
        }
        if cur_o_pos == next_run_start {
            //we are at a run, we need emit this op
            let (_,len,byte) = trgt_runs.pop().unwrap();
            cur_o_pos += len as usize;
            ops.push(Op::Run(Run{len,byte}));
            next_run_start = trgt_runs.last().map(|(start,_,_)| *start).unwrap_or(target.len());
            //out next_min_match might be 'behind' since our run was applied
            //run always gets precedence as it is the smallest possible op.
            if next_min_match.is_some() && next_min_match.as_ref().unwrap().next_o_pos < cur_o_pos{
                next_min_match = scan_for_next_match(&target, &src_sa, &trgt_sa, cur_o_pos);
            }
            continue;
        }

        //next_min_match should be at or ahead of cur_o_pos
        //if it is at cur_o_pos we should emit the copy and update the next_min_match.
        //if we are not yet to it, we should emit an add so the next iteration will emit the copy.

        if next_min_match.is_some() && next_min_match.as_ref().unwrap().next_o_pos == cur_o_pos{
            //this might not be cheaper than add, so we must check first.
            //if an add is cheaper, we should set find the next_min_match for the next iteration.
            let NextMinMatch{ src_found, trgt_found, .. }= next_min_match.unwrap();
            let src_match = if src_found {
                Some(find_certain_match(target, src, &src_sa, cur_o_pos, next_run_start, max_end_pos))
            }else{
                None
            };
            let trgt_match = if trgt_found {
                Some(find_certain_match(target, target, &trgt_sa, cur_o_pos, next_run_start, max_end_pos))
            }else{
                None
            };
            debug_assert!(if trgt_match.is_some() {valid_target(trgt_match.unwrap(), cur_o_pos as u64)}else{true});
            let use_trgt = trgt_match.is_some() && use_trgt_result(src_match, trgt_match.unwrap(), last_d_addr, last_o_addr);
            let copy_op = if use_trgt{
                make_copy(trgt_match.unwrap(), CopySrc::Output, &mut last_o_addr)
            }else{
                //we must have src match here
                make_copy(src_match.unwrap(), CopySrc::Dict, &mut last_d_addr)
            };
            if let Some(copy) = copy_op {
                //the best copy src is better than an add op
                cur_o_pos += copy.len as usize;
                next_min_match = scan_for_next_match(&target, &src_sa, &trgt_sa, cur_o_pos);
            }else{
                // we need to find the next next_min_match starting at the next_o_pos
                // the cur_o_pos + N: N is the min add inst length.
                next_min_match = scan_for_next_match(&target, &src_sa, &trgt_sa, cur_o_pos + MIN_ADD_LEN);
            }
        }
        //if we are here we are going to emit an add, no checking.
        //If there is not a next_min_match, we are done.
        //otherwise, emit an add up to the start of the next_min_match position.
        match next_min_match.as_ref() {
            Some(NextMinMatch{next_o_pos, ..}) => {
                fit_adds(&target[cur_o_pos..*next_o_pos], &mut ops);
                cur_o_pos = *next_o_pos;
            }
            None => {
                fit_adds(&target[cur_o_pos..], &mut ops);
                break;
            }
        }
    }

    ops
}

fn fit_adds(bytes: &[u8],output: &mut Vec<Op>){
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