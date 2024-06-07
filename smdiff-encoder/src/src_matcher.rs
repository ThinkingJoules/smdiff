
use crate::{hasher::*, hashmap::BasicHashTable,Ranger};

struct InnerConfig{
    l_step:usize,
    src_len:usize,
    trgt_len:usize,
    src_win_size:usize,
    max_end_pos:usize,
}

pub(crate) struct SrcMatcher{
    pub(crate) l_step:usize,
    pub(crate) fwd_hash: usize,
    pub(crate) fwd_pos: usize,
    pub(crate) max_fwd_hash_pos:usize,
    table: BasicHashTable,
    //Window calc state
    src_len:usize,
    trgt_len:usize,
    half_win_size:usize,
    max_end_pos:usize,
    cur_window_end:usize,
    pub(crate) next_hash_pos:usize,
    max_match_pos:usize,
}

impl SrcMatcher{

    ///Returns (src_pos, pre_match, post_match) post_match *includes* the hash_win_len.
    pub fn find_best_src_match(&mut self,src:&[u8],trgt:&[u8])->Option<(usize,usize,usize)>{
        let table_pos = self.table.get(self.fwd_hash)?;
        let src_pos = self.table_to_abs_pos(table_pos);
        if let Some((pre_match,post_match)) = extend_src_match(src, src_pos, trgt, self.fwd_pos) {
            let total_post_match = post_match + 9;
            return Some((src_pos,pre_match,total_post_match));
        }
        None
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
        let table_pos = self.abs_to_table_pos(abs_pos);
        match self.table.insert(hash, table_pos){
            Ok(None) => {},
            Ok(Some(_prev)) => {
                //self.chain.insert(hash, table_pos, prev);
            },
            Err((_old_hash,_prev_pos)) => {
                //self.chain.insert(old_hash, table_pos, prev_pos);
            }
        }
    }
}

// I used to add them in forward order, and on the test files actually got better matches
// However xd3 puts them in in reverse order, and logically it makes more sense.
// Since we don't store hash collisions/duplicates we probably want to keep hashes closest to our current position.
// Thus storing the early (closer) positions will ensure that they are not evicted by later positions.
pub(crate) fn add_start_positions_to_matcher(matcher: &mut SrcMatcher, cur_o_pos: usize, src: &[u8]) {
    if cur_o_pos < matcher.next_hash_pos{
        return;
    }
    if let Some(range) = calculate_window_range(cur_o_pos, matcher.src_len, matcher.trgt_len, matcher.half_win_size, matcher.max_end_pos, matcher.cur_window_end, matcher.l_step,matcher.max_match_pos) {
        //Put the new positions in, in reverse order
        //Reverse, because later positions are less likely to be used.
        //The hash table only keeps the last hash for a given hash
        //So hash/pos nearest our current position we want to ensure we keep.
        //By going in reverse, any collisions/duplicate starts will be evicting matches later in the src file.
        //The idea is that similar files will have similar offsets.
        //Very different files will always suffer from poor alignment and missing matches.
        //That is why it is best to use TrgtMatcher as well as secondary compression and not rely on the SrcMatcher alone.
        debug_assert!(range.start % matcher.l_step == 0, "range.start({}) must be divisible by l_step({})",range.start,matcher.l_step);
        if range.end >= matcher.max_end_pos{
            matcher.next_hash_pos = usize::MAX;
        }else{
            //?? Should this be changed?
            //would a 1/4 of the way through our window size make more sense?
            // 3/4 of the way through our window size?
            // Would have to do with the file size differences and aligning between the two files based on matches.
            matcher.next_hash_pos = cur_o_pos + (matcher.half_win_size);
        }

        if matcher.l_step >= 9 {
            for pos in range.step_by(matcher.l_step).rev() {
                let hash = calculate_large_checksum(&src[pos..pos + 9]);
                matcher.store(hash, pos)
            }
        }else{
            let aligned_last_hash = align(range.end-9,matcher.l_step);
            let mut hash = calculate_large_checksum(&src[aligned_last_hash..range.end]);
            for pos in (range.start..aligned_last_hash).rev() {
                hash = update_large_checksum_bwd(hash, src[pos+9], src[pos]);
                if pos % matcher.l_step == 0 {
                    matcher.store(hash, pos);
                }
            }
            // for pos in (0..aligned_last_hash).rev().step_by(matcher.l_step).skip(1) {
            //     for inner_pos in (0..matcher.l_step).rev() {
            //         let current_pos = pos + inner_pos;
            //         hash = update_large_checksum_bwd(hash, src[current_pos + 9], src[current_pos]);
            //     }
            //     matcher.store(hash, pos);
            // }

        }
    }
}
const DEFAULT_SRC_WIN_SIZE: usize = 1 << 26;
const MIN_SRC_WIN_SIZE: usize = 1 << 20;
const _HASH_CHUNK_SIZE: usize = 1 << 23;
//const DEFAULT_PREV_SIZE: usize = 1 << 18;

///Configuration for the SrcMatcher.
#[derive(Debug, Clone)]
pub struct SrcMatcherConfig{
    /// How much to advance the Large Hash between storing a src hash.
    /// Larger value means faster, but might miss good matches.
    pub l_step: usize,
    /// The maximum size of the source window.
    /// This is how many bytes to assess and store hashes for.
    /// Larger values consider more matches, but might hash excessively slowing down encoder.
    /// Leave blank for dynamic calculation.
    pub max_src_win_size: Option<usize>,
}

impl Default for SrcMatcherConfig {
    fn default() -> Self {
        Self::comp_level(3)
    }
}
impl SrcMatcherConfig {
    ///Creates a new SrcMatcherConfig with the given parameters.
    /// l_step: How much to advance the Large Hash between storing a src hash.
    /// max_src_win_size: The maximum size of the source window.
    pub fn new(l_step: usize,max_src_win_size:Option<usize>) -> Self {
        Self { l_step, max_src_win_size}
    }
    ///Creates a new SrcMatcherConfig with the given compression level.
    /// level: The compression level to use. Must be between 0 and 9.
    /// The higher the level the more accurate the matches but slower.
    pub fn comp_level(level:usize)->Self{
        assert!(level <= 9);
        let l_step = Ranger::new(0..10, 26..=2).map(level);
        Self { l_step, max_src_win_size: None}
    }
    fn make_inner_config(&mut self, src_len: usize,trgt_len:usize)->InnerConfig{
        self.l_step = self.l_step.max(1);

        self.max_src_win_size = Some(self
            .max_src_win_size
            .map(|s| s.next_power_of_two().max(MIN_SRC_WIN_SIZE))
            .unwrap_or(
                DEFAULT_SRC_WIN_SIZE
                //calculate_default_win_size(src_len, trgt_len,None)
            ));
        InnerConfig{
            l_step:self.l_step,
            src_len,
            trgt_len,
            src_win_size:self.max_src_win_size.unwrap(),
            max_end_pos:align(src_len-9,self.l_step),
        }
    }
    pub(crate) fn build(&mut self,src:&[u8],trgt_start_pos:usize,trgt:&[u8])->SrcMatcher{
        let trgt_len = trgt.len();
        let InnerConfig { l_step, src_len, trgt_len, src_win_size, max_end_pos } = self.make_inner_config(src.len(),trgt_len);
        let max_fwd_hash_pos = trgt.len() - 9;
        let (fwd_hash,fwd_pos) = if trgt_start_pos < max_fwd_hash_pos {
            (calculate_large_checksum(&trgt[trgt_start_pos..trgt_start_pos+9]),trgt_start_pos)
        }else{
            (0,max_fwd_hash_pos)
        };
        //regardless of win_size given, we do not need to store more than the entire src file.
        let table_win_effective = (src_len.next_power_of_two() >> 1).min(src_win_size);
        let table = BasicHashTable::new(table_win_effective/l_step, false);
        let mut matcher = SrcMatcher{
            table, src_len, trgt_len, max_end_pos,max_fwd_hash_pos,
            fwd_hash,
            fwd_pos,
            l_step,
            half_win_size: src_win_size>>1,
            cur_window_end: 0,
            next_hash_pos: 0,
            max_match_pos: trgt_start_pos,
        };
        //prefill with hash start positions.
        add_start_positions_to_matcher(&mut matcher, trgt_start_pos, src);
        matcher
    }
}

#[inline]
fn align(pos:usize,l_step:usize)->usize{
    pos - (pos % l_step)
}

///Returns the (pre_match, post_match) for the given src and trgt data.
///None if the hash was a collision
pub(crate) fn extend_src_match(src:&[u8],src_start:usize,trgt:&[u8],trgt_start:usize)->Option<(usize,usize)>{
    //first verify hash matches the data
    let initial_match = src[src_start..src_start + 9]
        .iter().zip(trgt[trgt_start..trgt_start + 9].iter())
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
    let src_end = src_start + 9;
    let trgt_end = trgt_start + 9;
    let src_remain = src.len() - src_end;
    let trgt_remain = trgt.len() - trgt_end;
    let post_match = (0..src_remain.min(trgt_remain)).take_while(|&i| {
        src[src_end + i] == trgt[trgt_end + i]
    }).count();
    Some((pre_match,post_match))
}

//Thought I could cut down encoding time (which it does), but this misses lots of good matches on dissimilar files of similar length
#[allow(dead_code)]
fn calculate_default_win_size(src_len: usize, trgt_len: usize,max_win_size:Option<usize>) -> usize {
    let mut win_size = (src_len).abs_diff(trgt_len).next_power_of_two();
    if win_size <= MIN_SRC_WIN_SIZE {
        win_size = win_size + MIN_SRC_WIN_SIZE;
    }
    let upper_bound = src_len.next_power_of_two().min(max_win_size.map(|a|a.next_power_of_two()).unwrap_or(DEFAULT_SRC_WIN_SIZE));
    win_size.min(upper_bound)
}

#[inline]
fn calculate_window_range(
    cur_o_pos: usize,
    src_len: usize,
    trgt_len: usize,
    half_win_size: usize,
    max_end_pos: usize,
    cur_window_end: usize,
    l_step: usize,
    max_match_end:usize,
) -> Option<std::ops::Range<usize>> {
    //min and max cases
    if cur_o_pos < half_win_size {
        return Some(0..(half_win_size<<1).min(max_end_pos));
    }else if cur_window_end >= max_end_pos{
        return None;
    }
    //else we are going to slide the window based on the current output position

    //First find the scaled mid point, or the 'equivalent position in the src file'
    //We use this as our 'input position' to calculate the window position.
    //Note on `.min(max_match_end)`:
    // If our longest match has exceeded even this scaled position, use that instead.
    // We do not store already matched start positions.
    // That is, we our greedy at moving the src matcher fwd.
    let scaled_src_pos = cur_o_pos * src_len / trgt_len;

    //We add our half window to the equivalent position.
    let scaled_end = align((scaled_src_pos + half_win_size).min(max_end_pos),l_step);
    if scaled_end <= cur_window_end || scaled_end <= max_match_end {//nothing more to hash yet
        //this will be encountered often when the trgt file is (significantly) larger than the src file.
        return None;
    }
    //The max amt we logically could hash
    //We need this in case we move an entire window size forward.
    //We re-center our window based on the scaled src position.
    let max_diff_start = scaled_end.saturating_sub(half_win_size<<1);
    let scaled_start = align(cur_window_end.max(max_match_end).max(max_diff_start),l_step);
    Some(scaled_start..scaled_end)
}

#[cfg(test)]
mod test_super {
    use super::*;

    #[test]
    fn test_calculate_window_range_lrg_trgt() {
        let half_win_size = 256;
        let l_step = 2;
        let src_len = 1000;
        let trgt_len = 2000;
        let max_end_pos = 994;

        // Initial fast forward
        let result = calculate_window_range(10, src_len, trgt_len, half_win_size, max_end_pos, 0, l_step, 0);
        assert_eq!(result.unwrap(), 0..512, "Initial fast forward: Incorrect range");

        let result = calculate_window_range(10, src_len, trgt_len, 512, max_end_pos, 0, l_step, 0);
        assert!(result.is_some(), "Initial fast forward: Expected Some(0..1024), but got {:?}", result);
        assert_eq!(result.unwrap(), 0..max_end_pos, "Initial fast forward: Incorrect range");

        // End of file
        let result = calculate_window_range(900, src_len, trgt_len, half_win_size, max_end_pos, 1000, l_step, 0);
        assert!(result.is_none(), "End of file: Expected None, but got {:?}", result);

        // Nothing more to hash
        let result = calculate_window_range(500, src_len, trgt_len, half_win_size, max_end_pos, 700, l_step, 0);
        assert!(result.is_none(), "Nothing to hash: Expected None, but got {:?}", result);

        // Normal advancement
        let result = calculate_window_range(1250, src_len, trgt_len, half_win_size, max_end_pos, 512, l_step, 0);
        assert_eq!(result.unwrap(), 512..880, "Normal advancement: Incorrect range");

        // Scaled end too far
        let result = calculate_window_range(400, src_len, trgt_len, half_win_size, max_end_pos, 300, l_step, 1000);
        assert!(result.is_none(), "Scaled end too far: Expected None, but got {:?}", result);

        // Match is ahead of position
        let result = calculate_window_range(400, src_len, trgt_len, half_win_size, max_end_pos, 300, l_step, 700);
        assert!(result.is_none(), "Max Match Exceeds Position: Expected None, but got {:?}", result);
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