

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

use crate::{copy::{get_correct_check_len, make_copy, scan_for_next_match, unwrap_search_result, use_trgt_result, valid_target, NextMinMatch}, suffix::SuffixArray};
const MIN_MATCH_BYTES: usize = 2; //two because we are trying to optimize for small files.
const MIN_ADD_LEN: usize = 2; //we need to have at least 2 bytes to make an add instruction.
pub fn encode_diff(src: &[u8], target: &[u8]) -> Vec<Op>{
    if target.is_empty(){
        return Vec::new();
    }
    //We assume that if we can fit the src and trgt in memory, the rest of our overhead is minimal
    //first we find all the runs in the target file.
    let mut trgt_runs = run::find_byte_runs(target);
    // we gen the prefix trie from the target
    let trgt_sa = SuffixArray::new(target);

    let src_sa = SuffixArray::new(src);
    dbg!(&src_sa, &trgt_sa);
    //now we start at the beginning of the target file and start encoding.
    let mut cur_o_pos = 0;
    let max_end_pos = target.len();
    let mut last_d_addr = 0;
    let mut last_o_addr = 0;
    trgt_runs.reverse(); //last should be the first run we would encounter.
    let mut next_run_start = trgt_runs.last().map(|(start,_,_)| *start).unwrap_or(target.len());
    let mut ops = Vec::new();
    let mut next_min_match = scan_for_next_match(&target, &trgt_sa,&src,&src_sa,0);
    loop{
        dbg!(cur_o_pos, next_run_start, &next_min_match);
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
                next_min_match = scan_for_next_match(&target, &trgt_sa,&src,&src_sa,  cur_o_pos);
            }
            continue;
        }

        //next_min_match should be at or ahead of cur_o_pos
        //if it is at cur_o_pos we should emit the copy and update the next_min_match.
        //if we are not yet to it, we should emit an add so the next iteration will emit the copy.

        if next_min_match.is_some() && next_min_match.as_ref().unwrap().next_o_pos == cur_o_pos{
            //this might not be cheaper than add, so we must check first.
            //if an add is cheaper, we should set find the next_min_match for the next iteration.
            let NextMinMatch{ src_found, trgt_found, .. } = next_min_match.unwrap();
            dbg!(src_found, trgt_found);
            let check_len = get_correct_check_len(cur_o_pos, next_run_start, max_end_pos);
            let end_pos = cur_o_pos + check_len;
            let next_slice = &target[cur_o_pos..end_pos];
            let src_match = if src_found {
                let found = src_sa.search(src,next_slice);
                Some(unwrap_search_result(&found, check_len))
            }else{
                None
            };
            let trgt_match = if trgt_found {
                let found = trgt_sa.search_restricted(target,next_slice,cur_o_pos);
                Some(unwrap_search_result(&found, check_len))
            }else{
                None
            };
            let use_trgt = trgt_match.is_some() && use_trgt_result(src_match, trgt_match.unwrap(), last_d_addr, last_o_addr);
            debug_assert!(if use_trgt && trgt_match.is_some() {valid_target(trgt_match.unwrap(), cur_o_pos as u64)}else{true});
            dbg!(&src_match, &trgt_match,use_trgt);
            let copy_op = if src_match.is_none() || use_trgt{
                //one of these has to be a match
                make_copy(trgt_match.unwrap(), CopySrc::Output, &mut last_o_addr)
            }else{
                //we must have src match here
                make_copy(src_match.unwrap(), CopySrc::Dict, &mut last_d_addr)
            };
            dbg!(&copy_op);
            if let Some(copy) = copy_op {
                //the best copy src is better than an add op
                cur_o_pos += copy.len as usize;
                next_min_match = scan_for_next_match(&target, &trgt_sa,&src,&src_sa,  cur_o_pos);
                ops.push(Op::Copy(copy));
                continue;
            }else{
                // we need to find the next next_min_match starting at the next_o_pos
                // the cur_o_pos + N: N is the min add inst length.
                next_min_match = scan_for_next_match(&target, &trgt_sa,&src,&src_sa,  cur_o_pos + MIN_ADD_LEN);
            }
        }
        //if we are here we are going to emit an add, no checking.
        //If there is not a next_min_match, we are done.
        //otherwise, emit an add up to the start of the next_min_match position.
        match next_min_match.as_ref() {
            Some(NextMinMatch{next_o_pos, ..}) => {
                let min = next_o_pos.min(&next_run_start);
                //next op is either Run or Copy, we need to fill the space between.
                fit_adds(&target[cur_o_pos..*min], &mut ops);
                cur_o_pos = *min;
            }
            None => {
                let min = max_end_pos.min(next_run_start);
                fit_adds(&target[cur_o_pos..min], &mut ops);
                cur_o_pos = min;
                next_min_match = scan_for_next_match(&target, &trgt_sa,&src,&src_sa,  cur_o_pos);

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

#[cfg(test)]
mod tests {
    use super::*;
    use smdiff_common::{Copy, Op};
    #[test]
    fn test_empty_files(){
        let src = [];
        let target = [];
        let ops = encode_diff(&src, &target);
        assert_eq!(ops, Vec::<Op>::new());
    }

    #[test]
    fn test_no_common(){
        let src = [1,2,3,4,5];
        let target = [6,7,8,9,10];
        let ops = encode_diff(&src, &target);
        assert_eq!(ops, vec![Op::Add(Add{bytes: target.to_vec()})]);
    }

    #[test]
    fn test_run(){
        let src = [1,2,3,4,5];
        let target = [1,1,1,1,1];
        let ops = encode_diff(&src, &target);
        assert_eq!(ops, vec![Op::Run(Run{len: 5, byte: 1})]);
    }

    #[test]
    fn test_run_copy(){
        let src = [1,2,3,4,5];
        let target = [1,1,1,1,1,2,3,4,5];
        let ops = encode_diff(&src, &target);
        assert_eq!(ops, vec![Op::Run(Run{len: 5, byte: 1}), Op::Copy(Copy{src: CopySrc::Dict, addr: 1, len: 4})]);
    }
    #[test]
    fn test_copy_no_src(){
        let src = [];
        let target = [1,2,3,4,5,6,1,2,3,4,5,6,6,2,3,4,5,7];
        let ops = encode_diff(&src, &target);
        assert_eq!(ops, vec![
            Op::Add(Add{bytes: vec![1,2,3,4,5,6]}), //4
            Op::Copy(Copy { src: CopySrc::Output, addr: 0, len: 5 }), //2
            Op::Run(Run{len: 2, byte: 6}), //2
            Op::Copy(Copy { src: CopySrc::Output, addr: 1, len: 4 }), //2
            Op::Add(Add{bytes: vec![7]}), // 2
        ]);
    }
    #[test]
    fn test_copy(){
        let src = [1,2,3];
        let target = [1,2,3,4,5,6,1,2,3,4,5,6,6,2,3,4,5,7];
        let ops = encode_diff(&src, &target);
        assert_eq!(ops, vec![
            Op::Copy(Copy{src: CopySrc::Dict, addr:0, len: 3}), //2
            Op::Add(Add{bytes: vec![4,5,6]}), //4
            Op::Copy(Copy { src: CopySrc::Output, addr: 0, len: 5 }), //2
            Op::Run(Run{len: 2, byte: 6}), //2
            Op::Copy(Copy { src: CopySrc::Output, addr: 1, len: 4 }), //2
            Op::Add(Add{bytes: vec![7]}), // 2
        ]);
    }
    #[test]
    fn test_copy_favor_src(){
        let src = [1,2,3,4,5,6];
        let target = [1,2,3,4,5,6,1,2,3,4,5,6,6,2,3,4,5,7];
        let ops = encode_diff(&src, &target);
        assert_eq!(ops, vec![
            Op::Copy(Copy{src: CopySrc::Dict, addr:0, len: 6}), //2
            Op::Copy(Copy { src: CopySrc::Dict, addr: 0, len: 5 }), //2
            Op::Run(Run{len: 2, byte: 6}), //2
            Op::Copy(Copy { src: CopySrc::Dict, addr: 1, len: 4 }), //2
            Op::Add(Add{bytes: vec![7]}), // 2
        ]);
    }

    #[test]
    fn test_copy_larger_src(){
        let src = [1,2,3,4,5,6,1,2,3,4,5,6,6,2,3,4,5,7];
        let target = [1,2,3,4,5,6];
        let ops = encode_diff(&src, &target);
        assert_eq!(ops, vec![
            Op::Copy(Copy{src: CopySrc::Dict, addr:6, len: 6}), //2
        ]);
    }
}