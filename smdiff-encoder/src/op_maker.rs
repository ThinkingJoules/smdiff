use smdiff_common::{Copy, CopySrc, MAX_INST_SIZE, MAX_RUN_LEN};

use crate::{encoder::{GenericEncoderConfig, InnerOp}, Op};


//this is sort of naive now. It could have better address cost optimization checking.
pub fn translate_inner_ops<'a>(encoder_config:&GenericEncoderConfig,trgt:&'a [u8], mut ops: Vec<InnerOp>)->Vec<Op<'a>>{
    //we are going to do two passes, one for adjusting the ops to be non-overlapping
    //the second for converting them to windows and Op structs for smdiff.
    let src_min_match = encoder_config.match_src.as_ref()
        .map(|a|a.hash_win_len.unwrap()).unwrap_or(9);
    let trgt_min_match = encoder_config.match_trgt.as_ref()
        .map(|a|a.hash_win_len.unwrap()).unwrap_or(4);
    //to avoid another allocation, removed ops will have their length set to 0
    let ops_len = ops.len();
    let mut one = 0;
    for two in 1..ops_len {
        let one_p = *ops[one].o_pos();
        let one_len = *ops[one].len();
        //first see if these two even overlap
        let one_end = one_p + one_len;
        let two_p = *ops[two].o_pos();
        assert!(one_p <= two_p, "Ops are not sorted by position");
        if one_end <= two_p {
            //if they don't, move on
            one = two;
            continue;
        }
        //one and two overlap, we need to decide what to do.
        //see if we have third overlap that can make two irrelevant
        if two+1 < ops_len {
            let three = &ops[two+1];
            if one_end > *three.o_pos() {
                //two is completely redundant, we can just remove it.
                ops.get_mut(two).unwrap().set_len(0);
                //one should remain one, and three should become two in the next iteration
                continue;
            }
        }

        let two_end = two_p + *ops[two].len();
        //now, are one and two nearly the same size? If so we just pick the larger one and remove the other
        //If we were to make them coincident we would end up with two really short copies, neither probably profitable.
        let span = two_end - one_p;
        if span <= src_min_match + trgt_min_match || two_p - one_p <= 4 {
            //two overlapping instructions that are very small or start at nearly the same position
            //if nearly the same start, we just pick the larger one
            if one_len < *ops[two].len() {
                ops.get_mut(one).unwrap().set_len(0);
                one = two;
            } else {
                ops.get_mut(two).unwrap().set_len(0);
                //two should become a new op, and it will be compared to one.
            }
            continue;
        }
        //if we get here, we have two overlapping ops that should be able to be made coincident.
        assert!(two_end >= one_end) ;
        // Calculate the midpoint of the overlapping region
        let midpoint = two_p + (one_end - two_p) / 2;

        ops.get_mut(one).unwrap().set_len(midpoint - one_p);
        ops.get_mut(two).unwrap().set_o_pos(midpoint);
        debug_assert!(ops[one].o_pos() + ops[one].len() == *ops[two].o_pos(), "Ops are not coincident!");
        one = two;
    }

    //now we convert the ops into Op structs
    let mut out_ops = Vec::with_capacity(ops_len);
    let mut out_pos = 0;
    for op in ops.into_iter().filter(|a|*a.len() > 0) {
        let o_pos = *op.o_pos();
        if o_pos > out_pos {
            make_add_ops(&trgt[out_pos..o_pos], &mut out_ops);
            out_pos = o_pos;
        }
        assert!(o_pos == out_pos, "Ops are not sorted by position");
        let len = *op.len();
        match op {
            InnerOp::MatchSrc { start, length, .. } => {
                make_copy_ops(CopySrc::Dict, start, length, &mut out_ops);
            },
            InnerOp::MatchTrgt { start, length, .. } =>{
                assert!(start+length <= out_pos, "MatchTrgt exceeds output position {} >= {}", out_pos, start+length);
                make_copy_ops(CopySrc::Output, start, length, &mut out_ops);
            },
            InnerOp::Run { byte, length, .. } => {
                make_run_ops(byte, length, out_pos, &mut out_ops);
            },
        }
        out_pos += len;
    }
    if trgt.len() > out_pos {
        make_add_ops(&trgt[out_pos..], &mut out_ops);
    }
    out_ops
}


pub fn make_add_ops<'a>(bytes: &'a [u8],output: &mut Vec<Op<'a>>){
    let total_len = bytes.len();
    if total_len == 1{//emit a run of len 1
        output.push(Op::Run(smdiff_common::Run{len: 1, byte: bytes[0]}));
        return;
    }
    let mut processed = 0;
    loop{
        if processed == total_len{
            break;
        }
        let to_add = total_len - processed;
        let chunk_size = to_add.min(MAX_INST_SIZE as usize);
        let op = crate::Add{bytes: &bytes[processed..processed+chunk_size]};
        processed += chunk_size;
        output.push(Op::Add(op));
    }
}

fn make_copy_ops(src: CopySrc, start:usize, len:usize, output: &mut Vec<Op>){
    let mut processed = 0;
    let mut addr = start as u64;
    while processed < len {
        let remaining = len - processed;
        let chunk_size = MAX_INST_SIZE.min(remaining);
        let op = Op::Copy(Copy{ src, addr, len: chunk_size as u16 });
        output.push(op);
        addr += chunk_size as u64;
        processed += chunk_size;
    };
}
const RUN_LIMIT: usize = (MAX_RUN_LEN as usize) * 6;
const COPY_LIMIT: usize = RUN_LIMIT/2;
fn make_run_ops(byte:u8, len:usize, run_start_pos:usize, output: &mut Vec<Op>){
    if len < RUN_LIMIT {
        let mut processed = 0;
        while processed < len {
            let remaining = len - processed;
            let chunk_size = (MAX_RUN_LEN as usize).min(remaining);
            let op = Op::Run(smdiff_common::Run{byte, len: chunk_size as u8});
            output.push(op);
            processed += chunk_size;
        };
    }else{
        //we can use one or more copies on 3 runs.
        //we need to emit the three runs, then make the copies from the stack
        output.extend(std::iter::repeat_with(|| Op::Run(smdiff_common::Run{byte, len: MAX_RUN_LEN})).take(3));

        let copy_bytes = len - COPY_LIMIT;
        let mut processed = 0;
        let mut max_copy_size = COPY_LIMIT;
        while processed < copy_bytes{
            let copy_size = max_copy_size.min(copy_bytes - processed).min(MAX_INST_SIZE);
            let op = Op::Copy(Copy{src: CopySrc::Output, addr: run_start_pos as u64, len: copy_size as u16});
            output.push(op);
            processed += copy_size;
            max_copy_size += copy_size;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_small_run() {
        let mut output = Vec::new();
        make_run_ops(0xAA, 120, 0, &mut output);
        assert_eq!(output.len(), 2);
        assert!(matches!(output[0], Op::Run(smdiff_common::Run { byte: 0xAA, len: 62 })));
        assert!(matches!(output[1], Op::Run(smdiff_common::Run { byte: 0xAA, len: 58 })));
    }

    #[test]
    fn test_exact_run_limit() {
        let mut output = Vec::new();
        let run_limit = (MAX_RUN_LEN as usize) * 6;
        make_run_ops(0xBB, run_limit, 0, &mut output);
        assert_eq!(output.len(), 4); // Should fit exactly into 3 maximum-length runs + 1 Copy of all of them.
        for op in &output[..3] {
            assert!(matches!(op, Op::Run(smdiff_common::Run { byte: 0xBB, len: MAX_RUN_LEN })));
        }
        let copy = &output[3];
        assert!(matches!(copy, Op::Copy(Copy { src: CopySrc::Output, addr: 0, len: 186 })),"{:?}",copy);
    }

    #[test]
    fn test_large_run_needing_copies() {
        let mut output = Vec::new();
        let run_limit = (MAX_RUN_LEN as usize) * 6;
        make_run_ops(0xBB, run_limit+100, 0, &mut output);
        assert_eq!(output.len(), 5); // Should fit exactly into 3 maximum-length runs + 2 Copy of all of them.
        for op in &output[..3] {
            assert!(matches!(op, Op::Run(smdiff_common::Run { byte: 0xBB, len: MAX_RUN_LEN })));
        }
        let copy = &output[3];
        assert!(matches!(copy, Op::Copy(Copy { src: CopySrc::Output, addr: 0, len: 186 })),"{:?}",copy);
        let copy = &output[4];
        assert!(matches!(copy, Op::Copy(Copy { src: CopySrc::Output, addr: 0, len: 100 })),"{:?}",copy);
    }

    #[test]
    fn test_large_run_needing_copies2() {
        let mut output = Vec::new();
        make_run_ops(0xBB, 414, 0, &mut output);
        assert_eq!(output.len(), 5); // Should fit exactly into 3 maximum-length runs + 2 Copy of all of them.
        for op in &output[..3] {
            assert!(matches!(op, Op::Run(smdiff_common::Run { byte: 0xBB, len: MAX_RUN_LEN })));
        }
        let copy = &output[3];
        assert!(matches!(copy, Op::Copy(Copy { src: CopySrc::Output, addr: 0, len: 186 })),"{:?}",copy);
        let copy = &output[4];
        assert!(matches!(copy, Op::Copy(Copy { src: CopySrc::Output, addr: 0, len: 42 })),"{:?}",copy);
    }

    #[test]
    fn test_zero_length() {
        let mut output = Vec::new();
        make_run_ops(0xDD, 0, 0, &mut output);
        assert!(output.is_empty());
    }
}