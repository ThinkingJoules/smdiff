use std::ops::Range;



/// This finds valid Runs per the spec. That is, no longer than 63 bytes long.
/// Each element is (start, length, byte)
pub fn find_byte_runs(buffer: &[u8]) -> Vec<(usize, u8, u8)> {
    if buffer.len() < 2 {
        return Vec::new();
    }
    let end_len = buffer.len();
    let mut result = Vec::new();

    let mut start = 0;
    let mut current_byte = buffer[0];
    let mut length = 1usize;
    let mut i = 1;
    loop {
        if i < end_len && buffer[i] == current_byte{
            length += 1;
            i += 1;
            continue;
        }else if length > 1 {//we only store runs > 1, otherwise we would store all the bytes..
            while length > 0 {
                let run_length = usize::min(length, 63) as u8;
                result.push((start, run_length, current_byte));
                start += run_length as usize;
                length -= run_length as usize;
            }
        }
        if i >= buffer.len() {
            break;
        }
        current_byte = buffer[i];
        start = i;
        length = 1;
        i += 1;
    }

    result
}
pub fn byte_runs_to_ranges(runs: &[(usize, u8, u8)]) -> Vec<Range<usize>> {
    runs.iter().map(|(start, len, byte)| {
        *start..*start + *len as usize
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_buffer() {
        assert_eq!(find_byte_runs(&[]), Vec::<(usize, u8, u8)>::new());
    }

    #[test]
    fn test_no_repeats() {
        let buffer = [1, 2, 3, 4, 5];
        assert_eq!(find_byte_runs(&buffer), vec![]);
    }

    #[test]
    fn test_single_byte_repeated() {
        let buffer = [7; 64]; // Exceeds max_len when split
        let result = find_byte_runs(&buffer);
        assert_eq!(result, vec![(0, 63, 7), (63, 1, 7)]);
    }

    #[test]
    fn test_mixed_runs() {
        let buffer = [1, 1, 2, 2, 2, 3];
        let result = find_byte_runs(&buffer);
        assert_eq!(result, vec![(0, 2, 1), (2, 3, 2)]);
    }
}
