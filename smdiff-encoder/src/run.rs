use crate::MIN_RUN_LEN;
use smdiff_common::MAX_RUN_LEN;

/// If there is a run it returns the byte and the TOTAL length of the run
/// The length might exceed the MAX_RUN_LEN
pub fn handle_run(cur_pos_buffer:&[u8])->Option<(u8,usize)>{
    //if the next MIN_RUN_LEN bytes are the same, we have a run
    //see how long it is and return Some((byte, length))
    if cur_pos_buffer.len() < MIN_RUN_LEN {
        return None;
    }
    let min_run = [cur_pos_buffer[0]; MIN_RUN_LEN];
    if cur_pos_buffer[..MIN_RUN_LEN] == min_run {
        //we have a run, just how long is it
        let mut length = MIN_RUN_LEN;
        let count = cur_pos_buffer[MIN_RUN_LEN..].iter().take_while(|&&x| x == min_run[0]).count();
        length += count;
        Some((min_run[0], length))
    } else {
        None
    }
}


/// This finds valid Runs per the spec. That is, no longer than 62 bytes long.
/// Each element is (start, length, byte)
pub fn find_runs_in_add(buffer: &[u8]) -> Vec<(usize, u8, u8)> {
    if buffer.len() < MIN_RUN_LEN {
        return Vec::new();
    }
    let end_len = buffer.len();
    let mut result = Vec::new();
    let mut i = 0;
    loop {
        if i+MIN_RUN_LEN > end_len {
            break;
        }
        let win = &buffer[i..i+MIN_RUN_LEN];
        let min_run = [win[0]; MIN_RUN_LEN];
        if win == min_run {
            //we have a run, just how long is it
            //this is max 62 bytes long
            let start = i;
            let mut length = MIN_RUN_LEN;
            i += MIN_RUN_LEN;
            while i < end_len && buffer[i] == win[0] && length < MAX_RUN_LEN as usize {
                length += 1;
                i += 1;
            }
            result.push((start, length as u8, win[0]));

        }else if win[1] == win[2]{
            i += 1;
        }else{
            i += 2;
        }
    }
    result
}
/// Filter the runs to ensure the distance between runs is greater than a given value
pub fn filter_runs(runs: Vec<(usize, u8, u8)>, min_distance: usize) -> Vec<(usize, u8, u8)> {
    let mut filtered_runs = Vec::new();

    if runs.is_empty() {
        return filtered_runs;
    }

    // Add the first run since there's no previous one to compare with
    filtered_runs.push(runs[0]);

    for i in 1..runs.len() {
        let prev_run = &filtered_runs.last().unwrap();
        let current_run = &runs[i];

        let prev_end = prev_run.0 + prev_run.1 as usize;
        let current_start = current_run.0;

        // If the gap is greater than min_distance, add the current run
        // If it is 0, allow it though as it is a contiguous run
        let gap = current_start - prev_end;
        if gap == 0 || gap > min_distance {
            filtered_runs.push(*current_run);
        }
    }

    filtered_runs
}