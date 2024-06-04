
use crate::{hasher::{HasherCusor, LargeHashCursor}, hashmap::{BasicHashTable, HashTable}, max_unique_substrings_gt_hash_len};

struct InnerConfig{
    l_step:usize,
    hash_win_len: usize,
    src_len:usize,
    trgt_len:usize,
    src_win_size:usize,
    max_end_pos:usize,
    chain_check: usize,
    prev_table_capacity: usize,
}

pub(crate) struct SrcMatcher<'a>{
    pub(crate) l_step:usize,
    pub(crate) hash_win_len: usize,
    src_len:usize,
    trgt_len:usize,
    half_win_size:usize,
    max_end_pos:usize,
    chain_check: usize,
    hasher: LargeHashCursor<'a>,
    table: BasicHashTable,
}

impl<'a> SrcMatcher<'a> {
    pub(crate) fn hash_win_len(&self)->usize{
        self.hash_win_len
    }
    pub(crate) fn advance_and_store(&mut self) {
        let start_pos = self.hasher.peek_next_pos();
        //if start_pos >= self.hasher.data_len() - self.hash_win_len{return}
        debug_assert!(start_pos % self.l_step == 0, "start_pos({}) must be divisible by l_step({})",start_pos,self.l_step);
        let end_pos = start_pos + self.l_step;
        //dbg!(start_pos,end_pos);
        if self.l_step >= self.hash_win_len{
            self.seek(end_pos)
        }else if start_pos + self.l_step < self.hasher.data_len(){
            for _ in 0..self.l_step{
                self.hasher.next();
            }
            debug_assert!(end_pos == self.hasher.peek_next_pos());
            self.store(self.hasher.peek_next_hash(),self.hasher.peek_next_pos());
        }
    }
    pub(crate) fn seek(&mut self, pos:usize){
        debug_assert!(self.hasher.peek_next_pos() <= pos, "self.hasher.peek_next_pos({}) > pos({})",self.hasher.peek_next_pos(),pos);
        let aligned_pos = self.align_pos(pos);
        if let Some(hash) = self.hasher.seek(aligned_pos) {
            self.store(hash,aligned_pos)
        }
    }
    //This is called on every byte, to incrementally roll the src_win and stored hashes forward.
    pub(crate) fn center_on(&mut self, cur_o_pos:usize){
        if let Some((seek_pos, diff_steps)) = calculate_window_advancement(
            cur_o_pos, self.src_len, self.trgt_len, self.half_win_size, self.max_end_pos, self.hasher.peek_next_pos(), self.l_step, self.hasher.peek_next_pos()
        ) {
            if diff_steps > 1 {
                self.seek(seek_pos);
            }
            for _ in 0..diff_steps {
                self.advance_and_store();
            }
        }
    }
    ///Returns (src_pos, pre_match, post_match) post_match *includes* the hash_win_len.
    pub fn find_best_src_match(&self,src:&[u8],trgt:&[u8],cur_o_pos:usize,hash:usize)->Option<(usize,usize,usize)>{
        let table_pos = self.table.get_last_pos(self.table.calc_index(hash))?;
        let mut iter = std::iter::once(table_pos).chain(self.table.iter_prev_starts(table_pos, self.hasher.peek_next_pos()));
        let mut chain = self.chain_check;
        let mut best = None;
        let mut best_len = 0;
        let mut _chain_len = 0;
        let mut _collisions = 0;
        loop {
            if chain == 0{
                break;
            }
            if let Some(table_pos) = iter.next() {
                _chain_len += 1;
                let src_pos = self.table_to_abs_pos(table_pos);
                if let Some((pre_match,post_match)) = extend_src_match(src, src_pos, trgt, cur_o_pos, self.hash_win_len) {
                    let total_post_match = post_match + self.hash_win_len;
                    if total_post_match+pre_match > best_len{
                        best_len = total_post_match+pre_match;
                        best = Some((src_pos,pre_match,total_post_match));
                    }
                    chain -= 1;
                }else{
                    _collisions += 1;
                }
            }else{break;}

        }
        //dbg!(self.chain_check - chain);
        // if _collisions > 0{
        //     dbg!(_chain_len,_collisions,cur_o_pos);
        //     if cur_o_pos > 1000{
        //         panic!();
        //     }
        // }
        best
    }

    fn align_pos(&self, pos:usize)->usize{
        align(pos, self.l_step)
    }
    /// Positions returned from the table are in table space, this converts them to absolute start positions.
    /// In other words, the table_pos is multiplied by l_step to get the absolute position.
    pub(crate) fn table_to_abs_pos(&self, table_pos:usize)->usize{
        table_pos * self.l_step
    }
    fn abs_to_table_pos(&self, abs_pos:usize)->usize{
        debug_assert!(abs_pos % self.l_step == 0, "abs_pos({}) is !divisible by l_step({})",abs_pos,self.l_step);
        abs_pos / self.l_step
    }
    fn store(&mut self, hash:usize, abs_pos:usize){
        debug_assert!(abs_pos % self.l_step == 0);
        let idx = self.table.calc_index(hash);
        let table_pos = self.abs_to_table_pos(abs_pos);
        self.table.insert(idx, table_pos);
    }
}

const DEFAULT_SRC_WIN_SIZE: usize = 1 << 26;
const MIN_SRC_WIN_SIZE: usize = 1 << 20;
//const DEFAULT_PREV_SIZE: usize = 1 << 18;
#[derive(Debug, Clone)]
pub struct SrcMatcherConfig{
    ///How much to advance the Large Hash between storing a src hash.
    pub l_step: usize,
    /// Max number of entries to check in the chain during matching.
    /// Larger value means more accurate matches but slower.
    pub chain_check: usize,
    ///Advanced setting, leave as None for default.
    pub prev_table_capacity: Option<usize>,
    pub max_src_win_size: Option<usize>,
    pub hash_win_len: Option<usize>,
}

impl Default for SrcMatcherConfig {
    fn default() -> Self {
        Self::comp_level(3)
    }
}
impl SrcMatcherConfig {
    ///Creates a new SrcMatcherConfig with the given parameters.
    /// l_step: How much to advance the Large Hash between storing a src hash.
    /// lazy_escape_len: If the current match is less than lazy_escape_len it steps byte by byte looking for more matches.
    /// l_table: Advanced settings, leave as None for default. See TableConfig for more information.
    pub fn new(l_step: usize, chain_check:usize, prev_table_capacity:Option<usize>,max_src_win_size:Option<usize>,hash_win_len:Option<usize>) -> Self {
        Self { l_step, chain_check, prev_table_capacity,max_src_win_size,hash_win_len}
    }
    ///Creates a new SrcMatcherConfig with the given compression level.
    /// level: The compression level to use. Must be between 0 and 9.
    /// The higher the level the more accurate the matches but slower.
    pub fn comp_level(level:usize)->Self{
        assert!(level <= 9);
        let l_step = 2 + ((24 * (9-level)) / 9); // 26..=2;
        let chain_check = 1 + level;
        //dbg!(level,l_step,chain_check);
        Self { l_step, chain_check, prev_table_capacity: None, max_src_win_size: None, hash_win_len: None}
    }
    pub fn with_table_capacity(mut self, table_capacity:usize)->Self{
        self.prev_table_capacity = Some(table_capacity);
        self
    }
    fn make_inner_config(&mut self, src_len: usize,trgt_len:usize)->InnerConfig{
        self.l_step = self.l_step.max(1);
        self.chain_check = self.chain_check.max(1);

        self.max_src_win_size = Some(self
            .max_src_win_size
            .map(|s| s.next_power_of_two().max(MIN_SRC_WIN_SIZE))
            .unwrap_or(
                calculate_default_win_size(src_len, trgt_len,None)
            ));

        self.hash_win_len = Some(self
            .hash_win_len
            .map(|l| l.clamp(3, 9))
            .unwrap_or_else(|| src_hash_len(self.max_src_win_size.unwrap()).min(src_hash_len(trgt_len))));

        // Calculate prev_table_capacity dynamically
        self.prev_table_capacity = if self.chain_check == 1 {
            Some(0)
        } else {
            Some(self.prev_table_capacity
                .unwrap_or_else(||{
                    let exact = max_unique_substrings_gt_hash_len(self.hash_win_len.unwrap(), self.max_src_win_size.unwrap(), self.l_step);
                    (exact + exact/2).next_power_of_two() >> 1
                }))
        };

        InnerConfig{
            l_step:self.l_step,
            hash_win_len:self.hash_win_len.unwrap(),
            src_len,
            trgt_len,
            src_win_size:self.max_src_win_size.unwrap(),
            max_end_pos:align(src_len-self.hash_win_len.unwrap(),self.l_step),
            chain_check:self.chain_check,
            prev_table_capacity:self.prev_table_capacity.unwrap(),
        }
    }
    pub(crate) fn build<'a>(&mut self,src:&'a [u8],trgt_start_pos:usize,trgt_len:usize)->SrcMatcher<'a>{
        let Self { l_step, chain_check, prev_table_capacity, max_src_win_size, hash_win_len } = self;
        assert!(*l_step % 2 == 0, "l_step({}) must be and even number",l_step);
        let src_len = src.len();
        max_src_win_size.get_or_insert(DEFAULT_SRC_WIN_SIZE);
        *max_src_win_size = Some(max_src_win_size.unwrap().min(src.len()).next_power_of_two());
        let src_win = max_src_win_size.unwrap();
        let prev_capacity = if *chain_check == 1 {0}else{prev_table_capacity.unwrap_or((max_src_win_size.unwrap() / *l_step).next_power_of_two() >> 1)};
        *prev_table_capacity = Some(prev_capacity);
        let hwl = hash_win_len.get_or_insert(src_hash_len(src_win));
        let hasher = LargeHashCursor::new(src, *hwl);
        let table = BasicHashTable::new(src_win/ *l_step, prev_capacity, *hwl<=4);
        let max_end_pos =  align(src_len-*hwl,*l_step);
        let mut matcher = SrcMatcher{
            hasher, table, src_len, trgt_len, max_end_pos,
            l_step:*l_step,
            hash_win_len:*hwl,
            chain_check:*chain_check,
            half_win_size: src_win>>1,
        };
        //prefill with hash start positions.
        matcher.center_on(trgt_start_pos);
        matcher
    }
    pub(crate) fn build2<'a>(&mut self,src:&'a [u8],trgt_start_pos:usize,trgt_len:usize)->SrcMatcher<'a>{
        let InnerConfig { l_step, hash_win_len, src_len, trgt_len, src_win_size, max_end_pos, chain_check, prev_table_capacity } = self.make_inner_config(src.len(),trgt_len);
        let hasher = LargeHashCursor::new(src, hash_win_len);
        let prev_cap = if chain_check == 1 {0}else{prev_table_capacity};
        let table = BasicHashTable::new(src_win_size/l_step, prev_cap, hash_win_len<=4);
        let mut matcher = SrcMatcher{
            hasher, table, src_len, trgt_len, max_end_pos,
            l_step,
            hash_win_len,
            chain_check,
            half_win_size: src_win_size>>1,
        };
        //prefill with hash start positions.
        matcher.center_on(trgt_start_pos);
        matcher
    }
}

#[inline]
fn align(pos:usize,l_step:usize)->usize{
    pos - (pos % l_step)
}

///Returns the (pre_match, post_match) for the given src and trgt data.
///None if the hash was a collision
pub fn extend_src_match(src:&[u8],src_start:usize,trgt:&[u8],trgt_start:usize,initial_len:usize)->Option<(usize,usize)>{
    //first verify hash matches the data
    let initial_match = src[src_start..src_start + initial_len]
        .iter().zip(trgt[trgt_start..trgt_start + initial_len].iter())
        .all(|(a,b)| a == b);
    if !initial_match{
        return None;
    }
    // Extend backward
    let min_offset = src_start.min(trgt_start);
    let pre_match = if min_offset > 1 {
        (1..min_offset).take_while(|&i| {
            src[src_start - i] == trgt[trgt_start - i]
        }).count()
    }else{0};

    // Extend forward
    let src_end = src_start + initial_len;
    let trgt_end = trgt_start + initial_len;
    let src_remain = src.len() - src_end;
    let trgt_remain = trgt.len() - trgt_end;
    let post_match = (0..src_remain.min(trgt_remain)).take_while(|&i| {
        src[src_end + i] == trgt[trgt_end + i]
    }).count();
    Some((pre_match,post_match))
}

pub fn src_hash_len(len:usize)->usize{
    if len <= 127{
        3
    }else if len <= 16_383{
        4
    }else if len <= 2_097_151{
        5
    }else if len <= 6_998_841{
        6
    }else if len <= 23_541_202{
        7
    }else if len <= 79_182_851{
        8
    }else{
        9
    }
}
fn calculate_default_win_size(src_len: usize, trgt_len: usize,max_win_size:Option<usize>) -> usize {
    let mut win_size = (src_len).abs_diff(trgt_len).next_power_of_two();
    if win_size <= MIN_SRC_WIN_SIZE {
        win_size = win_size + MIN_SRC_WIN_SIZE;
    }
    let upper_bound = src_len.next_power_of_two().min(max_win_size.map(|a|a.next_power_of_two()).unwrap_or(DEFAULT_SRC_WIN_SIZE));
    win_size.min(upper_bound)
}

/// Calculates the advancement of a sliding window within a source file
/// based on the current output position in a target file.
///
/// The goal is two cover the same 'segment' by proportionally advancing the window.
/// If the src_len is 2000 and the trgt_len is 200, the window should advance 10x faster.
///
/// That way the last bytes in src will be considered when matching the last bytes in trgt.
/// This function accomplishes that for any size of src and trgt.
///
/// This function is designed for delta encoding scenarios where you have
/// a source file (`src_len`) and a target file (`trgt_len`), and you're
/// processing the target file to generate output. The goal is to keep a sliding
/// window within the source file that covers the most relevant region
/// based on the current output position.
///
/// The function takes into account:
/// - The current output position (`cur_o_pos`)
/// - The lengths of the source and target files (`src_len`, `trgt_len`)
/// - The size of the sliding window (`half_win_size` is half the window size)
/// - The maximum valid position in the source file (`max_end_pos`)
/// - The current ending position of the window in the source file (`cur_window_end`)
/// - The step size for advancing the window (`l_step`)
/// - The next position the hasher will process (`hasher_next_pos`)
///
/// It returns:
/// - `Some((seek_pos, diff_steps))`:  If the window needs to advance, this contains:
///     - `seek_pos`: The position in the source file to seek to before preloading.
///     - `diff_steps`: The number of steps (`l_step`) to advance after seeking.
/// - `None`: If the window does not need to advance (e.g., out of bounds).
#[inline(always)]
fn calculate_window_advancement(
    cur_o_pos: usize,
    src_len: usize,
    trgt_len: usize,
    half_win_size: usize,
    max_end_pos: usize,
    cur_window_end: usize,
    l_step: usize,
    hasher_next_pos: usize
) -> Option<(usize, usize)> {
    let scaled_mid_point = (cur_o_pos * src_len / trgt_len).max(half_win_size);
    let scaled_end = align((scaled_mid_point + half_win_size).min(max_end_pos),l_step);

    // Early return if window would go out of bounds or hasn't moved
    if scaled_end <= cur_window_end {
        return None;
    }
    let diff = (scaled_end - cur_window_end).min(half_win_size<<1);
    let seek_pos = scaled_end - diff;

    debug_assert!(seek_pos >= cur_window_end, "seek_pos must be >= cur_window_end");
    debug_assert!(hasher_next_pos % l_step == 0, "hasher_next_pos must be divisible by l_step");
    debug_assert!(seek_pos >= hasher_next_pos, "seek_pos must be >= hasher_next_pos");
    debug_assert!(diff / l_step > 0, "diff must be > one l_step cur_window_end={} scaled_end={}", cur_window_end,scaled_end);

    Some((seek_pos, diff / l_step))
}

#[cfg(test)]
mod test_super {
    use super::*;

    #[test]
    fn test_window_advancement_lrg_src() {
        let src_len = 2000;
        let trgt_len = 200;
        let window_size = 128;
        let half_win_size = window_size / 2;
        let l_step = 2;
        let max_end_pos = src_len - 4;

        // Test various output positions and initial window positions
        let test_cases = [
            (0, 0, Some((0, 64))),   // Start at beginning
            (50, 128, Some((436, 64))),  // Start partway through trgt on init
            (60, 436+128, Some((564, 50))), // Overlapping windows
            (190, 150, Some((1836, 64))),  // Near the end of trgt
            (200, 190, None),     // Exceeds trgt_len
            (50, 1900, None),    // Window out of bounds in src
        ];

        for (cur_o_pos, cur_window_end, expected) in test_cases {
            let result = calculate_window_advancement(
                cur_o_pos,
                src_len,
                trgt_len,
                half_win_size,
                max_end_pos,
                cur_window_end,
                l_step,
                cur_window_end // Assuming hasher_next_pos is at the current end for simplicity
            );
            println!("cur_o_pos={}, cur_src_window_end={}, result={:?}", cur_o_pos, cur_window_end, result);
            assert_eq!(result, expected, "Failed for cur_o_pos={}, cur_window_end={}", cur_o_pos, cur_window_end);
        }
    }
    #[test]
    fn test_window_advancement_lrg_trgt() {
        let src_len = 200;
        let trgt_len = 2000;
        let window_size = 128;
        let half_win_size = window_size / 2;
        let l_step = 2;
        let max_end_pos = src_len - 4; // Adjusted for 0-based indexing

        let test_cases = [
            (0, 0, Some((0, 64))),        // Start at beginning
            (500, 128, None),      // no need to advance yet
            (1000, 128, Some((128, 18))),   //Continue where we left off, but add 18 more steps.
            (1900, 128+36, Some((164, 16))), //Continue where we left off, but add 16 more steps.
            (2000, 128+36+32, None),         // Exceeds trgt_len
        ];

        for (cur_o_pos, cur_window_end, expected) in test_cases {
            let result = calculate_window_advancement(
                cur_o_pos,
                src_len,
                trgt_len,
                half_win_size,
                max_end_pos,
                cur_window_end,
                l_step,
                cur_window_end // Assuming hasher_next_pos is at the current end
            );
            println!("cur_o_pos={}, cur_src_window_end={}, result={:?}", cur_o_pos, cur_window_end, result);

            assert_eq!(result, expected, "Failed for cur_o_pos={}, cur_window_end={}", cur_o_pos, cur_window_end);
        }
    }

    #[test]
    fn test_calculate_default_win_size_various_conditions() {
        // Testing when source length is much larger than target length
        let src_len = 2_000_000;
        let trgt_len = 100;
        assert_eq!(calculate_default_win_size(src_len, trgt_len,None), src_len.next_power_of_two(), "test_large_source_small_target");

        // Testing when target length is much larger than source length
        let src_len = 100;
        let trgt_len = 2_000_000;
        assert_eq!(calculate_default_win_size(src_len, trgt_len,None), src_len.next_power_of_two(), "test_small_source_large_target");

        // Testing when source and target lengths are the same
        let src_len = 100_000_000;
        let trgt_len = 90_000_000;
        assert_eq!(calculate_default_win_size(src_len, trgt_len,None), 10_000_000usize.next_power_of_two(), "test_different_lengths");

        // Testing when the difference is below MIN_SRC_WIN_SIZE
        let src_len = 100_000_000;
        let trgt_len = (src_len - MIN_SRC_WIN_SIZE) + 10;
        assert_eq!(calculate_default_win_size(src_len, trgt_len,None), (src_len - trgt_len).next_power_of_two() + MIN_SRC_WIN_SIZE, "test_diff_below_min_src_win_size");

        // Very large difference
        let src_len = 100_000_000;
        let trgt_len = 25_000_000;
        assert_eq!(calculate_default_win_size(src_len, trgt_len,None), DEFAULT_SRC_WIN_SIZE, "test_source_below_min_src_win_size");
    }

}