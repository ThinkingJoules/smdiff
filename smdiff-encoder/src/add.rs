


use smdiff_common::{Run, MAX_INST_SIZE, MAX_RUN_LEN};

use crate::{run::{filter_runs, find_runs_in_add}, Add, Op};

pub fn make_add_ops<'a>(bytes: &'a [u8],output: &mut Vec<Op<'a>>){
    let total_len = bytes.len();
    if total_len == 1{//emit a run of len 1
        output.push(Op::Run(Run{len: 1, byte: bytes[0]}));
        return;
    }
    let mut processed = 0;
    loop{
        if processed == total_len{
            break;
        }
        let to_add = total_len - processed;
        let chunk_size = to_add.min(MAX_INST_SIZE as usize);
        let op = Add{bytes: &bytes[processed..processed+chunk_size]};
        processed += chunk_size;
        output.push(Op::Add(op));
    }
}

pub fn make_adds<'a>(bytes: &'a [u8],output: &mut Vec<Op<'a>>,num_add_bytes:&mut usize){
    let total_len = bytes.len();
    //let start = std::time::Instant::now();
    if total_len == 1{//emit a run of len 1
        output.push(Op::Run(Run{len: 1, byte: bytes[0]}));
        //let dur = start.elapsed();
        //println!("Add(1) as Run(byte({})) took: {:?}",bytes[0],dur);
        return;
    }
    let mut processed = 0;
    loop{
        if processed == total_len{
            break;
        }
        let to_add = total_len - processed;
        let chunk_size = to_add.min(MAX_INST_SIZE as usize);
        let op = Add{bytes: &bytes[processed..processed+chunk_size]};
        processed += chunk_size;
        *num_add_bytes += chunk_size;
        output.push(Op::Add(op));
    }
    //let dur = start.elapsed();
    //println!("Making ADD(s) of len: {} took: {:?}",total_len,dur);
}
pub fn make_add_runs<'a>(bytes: &'a [u8],output: &mut Vec<Op<'a>>,num_add_bytes:&mut usize){
    //we want to find all the runs we can first, then emit the two.
    let total_len = bytes.len();
    //println!("Making ADD OR RUNS of len: {}",total_len);
    //let start = std::time::Instant::now();
    if total_len == 1{//emit a run of len 1
        output.push(Op::Run(Run{len: 1, byte: bytes[0]}));
        return;
    }
    let mut runs = filter_runs(find_runs_in_add(bytes),4);
    runs.reverse();
    let mut next_run_start = runs.last().map(|(start,_,_)| *start).unwrap_or(bytes.len());
    let mut processed = 0;
    loop{
        if processed == total_len{
            break;
        }
        if next_run_start == processed {
            let (_,len,byte) = runs.pop().unwrap();
            assert!(len <= MAX_RUN_LEN);
            processed += len as usize;
            output.push(Op::Run(Run{len,byte}));
            next_run_start = runs.last().map(|(start,_,_)| *start).unwrap_or(total_len);
            continue;
        }
        //adds one add op, might loop around and add more until we get to the run
        let to_add = next_run_start - processed;
        let chunk_size = to_add.min(MAX_INST_SIZE as usize);
        let op = Add{bytes: &bytes[processed..processed+chunk_size]};
        processed += chunk_size;
        *num_add_bytes += chunk_size;
        output.push(Op::Add(op));
    }
    //let dur = start.elapsed();
    //println!("Making ADD OR RUNS of len: {} took: {:?}",total_len,dur);
}