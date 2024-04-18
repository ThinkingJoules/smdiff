
/*
This code was from v1 of my Operation layout.
I have since gone away from using Sequence, and just have Run Operation.
*/

#[derive(Debug, PartialEq)]
pub struct Seq {
    pub start: usize,
    pub pat_len: usize,
    pub seq_len: usize,
}
pub fn find_all_sequences(bytes: &[u8], min_pat_len: usize, min_seq_len: usize) -> Vec<Seq> {
    assert!(min_pat_len > 1);
    assert!(min_seq_len < min_pat_len);
    let mut seqs = Vec::new();
    let mut start = 0;
    let min_len = min_pat_len + min_seq_len;
    let max_end = bytes.len() - min_len;
    while start < max_end {
        //we work with largest possible remaining pattern length our min params allow for
        let mut test_pat_len = (bytes.len() - start - min_seq_len) / 2;
        while test_pat_len >= min_pat_len {
            if let Some(seq) = find_sequence_at_position(bytes, start, test_pat_len, min_seq_len) {
                start += seq.seq_len;
                seqs.push(seq);
                break;
            }else{
                test_pat_len -= 1;
            }
        }
        start += 1;
    }
    seqs
}

fn find_sequence_at_position(bytes: &[u8], start: usize, test_pat_len: usize, min_seq_len: usize) -> Option<Seq> {
    debug_assert!(test_pat_len > 1);
    debug_assert!(start + test_pat_len + min_seq_len <= bytes.len());
    let max_cycles = calculate_max_cycles(bytes.len() - start, test_pat_len, min_seq_len);
    let min_cycles = calculate_min_cycles(test_pat_len, min_seq_len);
    let found_cycles = find_most_cycles(bytes, start, test_pat_len, max_cycles)?;
    dbg!(found_cycles, max_cycles, min_cycles);
    if min_cycles > found_cycles {
        return None;
    }
    let pattern = &bytes[start..start+test_pat_len];
    let try_rem = min_cycles == found_cycles && min_seq_len > 0;
    if try_rem && !test_remainder(bytes, pattern, start + test_pat_len * found_cycles, min_seq_len) {
        return None;
    }
    //we can still optionally test for additional remainders forward if max_cycles != found_cycles
    let rem_len = if found_cycles < max_cycles && !try_rem {
        let mut rem_len = test_pat_len -1;
        while rem_len > 0 {
            if test_remainder(bytes, pattern, start + test_pat_len * found_cycles, rem_len) {
                break;
            }
            rem_len -= 1;
        }
        rem_len
    }else if try_rem{
        min_seq_len
    }else{0};
    //we have a sequence per the parameters
    let seq_len =  found_cycles * test_pat_len + rem_len;
    Some(Seq {
        start,
        pat_len: test_pat_len,
        seq_len,
    })
}


fn find_most_cycles(bytes: &[u8], pat_start: usize, pat_len: usize, cycles: usize) -> Option<usize> {
    let end = pat_start + pat_len;
    let slice = &bytes[pat_start..end];
    let cycle_end = pat_len * cycles;
    debug_assert!(cycle_end <= bytes.len()-pat_start);
    let mut matched = 0;
    for (cycle,chunk) in bytes[pat_start..cycle_end].chunks_exact(pat_len).enumerate().rev() {
        if chunk == slice && matched == 0{
            matched = cycle;
        }else if chunk != slice && matched != 0{
            matched = 0;
        }
    }
    if matched != 0 {
        Some(matched +1)
    } else {
        None
    }
}
fn test_remainder(bytes: &[u8], pattern:&[u8], rem_start: usize, rem_len: usize) -> bool {
    debug_assert!(rem_len < pattern.len());
    bytes[rem_start..rem_start+rem_len] == pattern[..rem_len]
}

fn calculate_min_cycles(test_pat_len: usize, min_seq_len: usize) -> usize {
    (test_pat_len + min_seq_len) / test_pat_len
}
fn calculate_max_cycles(remaining_bytes:usize, test_pat_len: usize, min_seq_len: usize) -> usize {
    let cycles = remaining_bytes / test_pat_len;
    let rem_len = calculate_rem_len(test_pat_len, min_seq_len);
    if cycles == 1 && test_pat_len + rem_len >= remaining_bytes {
        cycles - 1
    } else {
        cycles
    }
}
fn calculate_rem_len(test_pat_len: usize, min_seq_len: usize) -> usize {
    (test_pat_len + min_seq_len) % test_pat_len
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_basic_all_functionality() {
        let bytes = b"abcabcabcabc";
        let seqs = find_all_sequences(bytes, 3, 2);
        assert_eq!(seqs.len(), 1);
        assert_eq!(seqs[0], Seq { start: 0, pat_len: 3, seq_len: 12 });
    }

    #[test]
    fn test_no_sequences_all_found() {
        let bytes = b"abcdefg";
        let seqs = find_all_sequences(bytes, 3, 2);
        assert!(seqs.is_empty());
    }

    #[test]
    fn test_all_boundary_conditions() {
        let bytes = b"abcabcabcabc";
        let seqs = find_all_sequences(bytes, 3, 1);
        // Expect sequences at start and possibly at middle if overlapping is not handled
        assert_eq!(seqs.len(), 1);
    }

    #[test]
    fn test_overlapping_sequences() {
        let bytes = b"abcabcabcabc";
        let seqs = find_all_sequences(bytes, 3, 2);
        // Depending on whether the implementation allows for overlapping sequences
        assert_eq!(seqs.len(), 1);
    }

    #[test]
    fn test_edge_case_small_input() {
        let bytes = b"ab";
        let seqs = find_all_sequences(bytes, 2, 0);
        assert!(seqs.is_empty());
    }

    #[test]
    fn test_basic_functionality() {
        let bytes = b"abcabcabc";
        let seq = find_sequence_at_position(bytes, 0, 3, 2);
        assert_eq!(seq, Some(Seq { start: 0, pat_len: 3, seq_len: 9 }));
    }

    #[test]
    fn test_no_valid_sequence() {
        let bytes = b"abcdef";
        let seq = find_sequence_at_position(bytes, 0, 3, 7); // min_seq_len too large
        assert!(seq.is_none());
    }
    #[test]
    fn test_minimum_sequence_length_not_met() {
        let bytes = b"abcafcabc";
        let seq = find_sequence_at_position(bytes, 0, 3, 2); // Sequence length not met
        assert!(seq.is_none());
    }

    #[test]
    fn test_calculate_min() {
        assert_eq!(calculate_min_cycles(3, 0), 1);
        assert_eq!(calculate_min_cycles(3, 2), 1);
        assert_eq!(calculate_min_cycles(3, 3), 2);
        assert_eq!(calculate_min_cycles(6, 3), 1);
    }
    #[test]
    fn test_calculate_max() {
        assert_eq!(calculate_max_cycles(9, 3, 2), 3);
        assert_eq!(calculate_max_cycles(9, 3, 3), 3);
        assert_eq!(calculate_max_cycles(9, 3, 4), 3);
        assert_eq!(calculate_max_cycles(6, 3, 3), 2);
    }

    #[test]
    fn test_calculate_rem_len() {
        assert_eq!(calculate_rem_len(3, 0), 0);
        assert_eq!(calculate_rem_len(3, 2), 2);
        assert_eq!(calculate_rem_len(3, 3), 0);
        assert_eq!(calculate_rem_len(6, 3), 3);
    }
}
