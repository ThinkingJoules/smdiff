
/*
Approach to generating delta operations.
This encoder requires the encoded portion of a choses src and target slice to be in memory.
This will generate a Suffix Array for each to allow for matching prefixes.
This requires a fair amount of memory:
    Src: 8 * src.len()
    Trgt: 16 * trgt.len()
    In theory trgt has a max len of 2^24, so 16MB. This is the max window size per the spec.
    Src has a max len of 2^32. This is an implementation limit for this encoder.

    Trgt requires extra memory for an index so we know which suffixes to consider for a given encoding start position.

Approach:
- First find all the Runs in our Target File. This might not be optimal, as it can break up a copy instruction.
    - To ensure it is not terrible, a minimum value should be set based on expected address overhead for copy.
    - If a file is small then breaking up a copy is only a couple bytes of overhead.
    - A value of 2 or 3 should be fine. Large files might benefit from a larger value. Max is 62, per the spec.

Then we begin our main loop. The operation precedence is:
    - Run
    - Copy
    - Add

We peek at our first run index,
we also find the first min_match in either file (this will only be src, since we haven't emitted any ops yet).
Each loop iteration:
    If our current output byte is a start of a run, we emit the run and continue. (highest precedence)
    else if our next min_match is at the current output position we emit a copy. (next highest precedence)
    else we emit an add instruction up to the next min_match position. (lowest precedence)

There are other things we do, such as check if a copy is better than an add, and if we should use the src or target match.
This is trying to optimize for the smallest encoded delta size.
*/

use smdiff_common::{Add,CopySrc, Op, Run, MAX_INST_SIZE};

use crate::{copy::{get_correct_check_len, make_copy, scan_for_next_match, unwrap_search_result, use_trgt_result, valid_target, NextMinMatch}, run::find_byte_runs, suffix::SuffixArray, MAX_WIN_SIZE, MIN_ADD_LEN};
pub fn encode_window(src: &[u8],src_abs_start_pos:u64, target: &[u8],target_abs_start_pos:u64,min_run_len:usize) -> Vec<Op>{
    assert!(target.len() <= MAX_WIN_SIZE);
    if target.is_empty(){
        return Vec::new();
    }
    //We assume that if we can fit the src and trgt in memory, the rest of our overhead is minimal
    //first we find all the runs in the target file.
    let mut trgt_runs = find_byte_runs(target,min_run_len);
    // we gen the prefix trie from the target
    let trgt_sa = SuffixArray::new(target);

    let src_sa = SuffixArray::new(src);
    dbg!(&src_sa, &trgt_sa);
    //now we start at the beginning of the target file and start encoding.
    let mut rel_o = 0;
    let max_end_pos = target.len();
    let mut last_d_addr = 0; //zeroed for each window
    let mut last_o_addr = 0;
    trgt_runs.reverse(); //last should be the first run we would encounter.
    let mut next_run_start = trgt_runs.last().map(|(start,_,_)| *start).unwrap_or(target.len());
    let mut ops = Vec::new();
    let mut next_min_match = scan_for_next_match(&target, &trgt_sa,&src,&src_sa,0);
    loop{
        dbg!(rel_o, next_run_start, &next_min_match);
        if rel_o >= target.len() {
            break;
        }
        if rel_o == next_run_start {
            //we are at a run, we need emit this op
            let (_,len,byte) = trgt_runs.pop().unwrap();
            rel_o += len as usize;
            ops.push(Op::Run(Run{len,byte}));
            next_run_start = trgt_runs.last().map(|(start,_,_)| *start).unwrap_or(target.len());
            //out next_min_match might be 'behind' since our run was applied
            //run always gets precedence as it is the smallest possible op.
            if next_min_match.is_some() && next_min_match.as_ref().unwrap().next_o_pos < rel_o{
                next_min_match = scan_for_next_match(&target, &trgt_sa,&src,&src_sa,  rel_o);
            }
            continue;
        }

        //next_min_match should be at or ahead of cur_o_pos
        //if it is at cur_o_pos we should emit the copy and update the next_min_match.
        //if we are not yet to it, we should emit an add so the next iteration will emit the copy.

        if next_min_match.is_some() && next_min_match.as_ref().unwrap().next_o_pos == rel_o{
            //this might not be cheaper than add, so we must check first.
            //if an add is cheaper, we should set find the next_min_match for the next iteration.
            let NextMinMatch{ src_found, trgt_found, .. } = next_min_match.unwrap();
            dbg!(src_found, trgt_found);
            let check_len = get_correct_check_len(rel_o, next_run_start, max_end_pos);
            let end_pos = rel_o + check_len;
            let next_slice = &target[rel_o..end_pos];
            let src_match = if src_found {
                let found = src_sa.search(src,next_slice);
                Some(unwrap_search_result(&found, check_len,src_abs_start_pos))
            }else{
                None
            };
            let trgt_match = if trgt_found {
                let found = trgt_sa.search_restricted(target,next_slice,rel_o);
                Some(unwrap_search_result(&found, check_len,target_abs_start_pos))
            }else{
                None
            };
            let use_trgt = trgt_match.is_some() && use_trgt_result(src_match, trgt_match.unwrap(), last_d_addr, last_o_addr);
            debug_assert!(if use_trgt && trgt_match.is_some() {valid_target(trgt_match.unwrap(), rel_o as u64)}else{true});
            dbg!(&src_match, &trgt_match,use_trgt);
            let copy_op = if src_match.is_none() || use_trgt{
                //one of these has to be Some
                make_copy(trgt_match.unwrap(), CopySrc::Output, &mut last_o_addr)
            }else{
                //we must have src match here
                make_copy(src_match.unwrap(), CopySrc::Dict, &mut last_d_addr)
            };
            dbg!(&copy_op);
            if let Some(copy) = copy_op {
                //the best copy src is better than an add op
                rel_o += copy.len as usize;
                next_min_match = scan_for_next_match(&target, &trgt_sa,&src,&src_sa,  rel_o);
                ops.push(Op::Copy(copy));
                continue;
            }else{
                // we need to find the next next_min_match starting at the next_o_pos
                // the cur_o_pos + N: N is the min add inst length.
                next_min_match = scan_for_next_match(&target, &trgt_sa,&src,&src_sa,  rel_o + MIN_ADD_LEN);
            }
        }
        //if we are here we are going to emit an add, no checking.
        //If there is not a next_min_match, we are done.
        //otherwise, emit an add up to the start of the next_min_match position.
        match next_min_match.as_ref() {
            Some(NextMinMatch{next_o_pos, ..}) => {
                let min = next_o_pos.min(&next_run_start);
                //next op is either Run or Copy, we need to fill the space between.
                fit_adds(&target[rel_o..*min], &mut ops);
                rel_o = *min;
            }
            None => {
                let min = max_end_pos.min(next_run_start);
                fit_adds(&target[rel_o..min], &mut ops);
                rel_o = min;
                next_min_match = scan_for_next_match(&target, &trgt_sa,&src,&src_sa,  rel_o);

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
        let ops = encode_window(&src, 0,&target,0,2);
        assert_eq!(ops, Vec::<Op>::new());
    }

    #[test]
    fn test_no_common(){
        let src = [1,2,3,4,5];
        let target = [6,7,8,9,10];
        let ops = encode_window(&src, 0,&target,0,2);
        assert_eq!(ops, vec![Op::Add(Add{bytes: target.to_vec()})]);
    }

    #[test]
    fn test_run(){
        let src = [1,2,3,4,5];
        let target = [1,1,1,1,1];
        let ops = encode_window(&src, 0,&target,0,2);
        assert_eq!(ops, vec![Op::Run(Run{len: 5, byte: 1})]);
    }

    #[test]
    fn test_run_copy(){
        let src = [1,2,3,4,5];
        let target = [1,1,1,1,1,2,3,4,5];
        let ops = encode_window(&src, 0,&target,0,2);
        assert_eq!(ops, vec![Op::Run(Run{len: 5, byte: 1}), Op::Copy(Copy{src: CopySrc::Dict, addr: 1, len: 4})]);
    }
    #[test]
    fn test_copy_no_src(){
        let src = [];
        let target = [1,2,3,4,5,6,1,2,3,4,5,6,6,2,3,4,5,7];
        let ops = encode_window(&src, 0,&target,0,2);
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
        let ops = encode_window(&src, 0,&target,0,2);
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
        let ops = encode_window(&src, 0,&target,0,2);
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
        let ops = encode_window(&src, 0,&target,0,2);
        assert_eq!(ops, vec![
            Op::Copy(Copy{src: CopySrc::Dict, addr:6, len: 6}), //2
        ]);
    }
}