
//I thought this might be fast for the really small matches, since it is dependent on the last addr
//The thought was to cut down on the number of hashes we need to keep (longer hash window)
//however, it seems slower than the hash based approach
//leaving it here to maybe revisit.

pub struct Scanner<'a> {
    buffer: &'a [u8],
    cur_pos: usize,

}

impl<'a> Scanner<'a> {
    const MIN_MATCH: usize = 2;
    pub fn new(buffer: &'a [u8]) -> Self {
        Scanner {
            buffer,
            cur_pos: 0,
        }
    }
    pub fn scan(&self, value: &[u8],max_end_pos:usize) -> Option<(u32, usize)> {
        if self.buffer.len() < Self::MIN_MATCH {
            return None;
        }
        let ranges: [(i32, i32, usize); 3] = [
            (-64, 63, Self::MIN_MATCH),           // min 3 byte match
            (-8192,-64, Self::MIN_MATCH+1), (64, 8192, Self::MIN_MATCH+1), // Separate ranges for min 4 byte match
            //(-1048576, -8192, Self::MIN_MATCH+2), (8192, 1048576, Self::MIN_MATCH+2), // Separate ranges for min 5 byte match
            //(-134217728, -1048576, Self::MIN_MATCH+3), (1048576, 134217727, Self::MIN_MATCH+3), // Separate ranges for min 6 byte match
        ];

        let mut longest_match: Option<(usize, usize, usize)> = None;

        for &(start_offset, end_offset, min_match) in ranges.iter() {
            if value.len() < min_match {
                break;
            }
            let scan_slice_start = self.cur_pos.saturating_sub(start_offset.abs() as usize);
            if scan_slice_start > max_end_pos {
                continue;
            }
            let scan_slice_end = std::cmp::min(self.buffer.len(), self.cur_pos + end_offset as usize).min(max_end_pos);
            if scan_slice_end < scan_slice_start+min_match {
                continue;
            }
            let mut src = &self.buffer[scan_slice_start..scan_slice_end];
            let trgt = &value[..Self::MIN_MATCH];

            while let Some((src_pos, _)) = find_first_substring_match(src, trgt, Self::MIN_MATCH) {
                let abs_start = scan_slice_start + src_pos;
                let substring_match_end = abs_start + Self::MIN_MATCH;
                // let min_match_end = abs_start + (min_match-Self::MIN_MATCH);
                // if min_match_end > scan_slice_end {
                //     continue;
                // }

                //we can potentially match up to the min_match length
                let rest = self.buffer[substring_match_end..].iter().zip(&value[Self::MIN_MATCH..]).take_while(|(a,b)| a == b).count();
                let adjusted_end = (abs_start + Self::MIN_MATCH + rest).min(max_end_pos);
                let match_len = adjusted_end - abs_start;
                if let Some((_, len,min)) = longest_match {
                    if match_len-min_match > len-min {
                        longest_match = Some((abs_start, match_len,min_match));
                    }
                } else {
                    longest_match = Some((abs_start, match_len,min_match));
                }
                src = &src[src_pos+1..];
            }
        }

        longest_match.map(|(start,len,_)|(start as u32,len))
    }

    pub fn update_pos(&mut self, pos: usize) {
        self.cur_pos = pos;
    }
}


pub fn find_first_substring_match(src: &[u8], trgt: &[u8], len: usize) -> Option<(usize, usize)> {
    for (i, trgt_sub) in trgt.windows(len).enumerate() {
        for (j, src_sub) in src.windows(len).enumerate() {
            if trgt_sub == src_sub {
                return Some((j, i));
            }
        }
    }

    None
}
