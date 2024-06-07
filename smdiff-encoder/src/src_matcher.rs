
use crate::{hasher::*, hashmap::BasicHashTable,Ranger};

struct InnerConfig{
    l_step:usize,
    hash_win_len: usize,
    src_len:usize,
    trgt_len:usize,
    src_win_size:usize,
    max_end_pos:usize,
    chain_check: usize,
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
    // pub(crate) fn hash_win_len(&self)->usize{
    //     self.hash_win_len
    // }
    // pub(crate) fn advance_and_store(&mut self) {
    //     let start_pos = self.hasher.peek_next_pos();
    //     //if start_pos >= self.hasher.data_len() - self.hash_win_len{return}
    //     debug_assert!(start_pos % self.l_step == 0, "start_pos({}) must be divisible by l_step({})",start_pos,self.l_step);
    //     let end_pos = start_pos + self.l_step;
    //     //dbg!(start_pos,end_pos);
    //     if self.l_step >= self.hash_win_len{
    //         self.seek(end_pos)
    //     }else if start_pos + self.l_step < self.hasher.data_len(){
    //         for _ in 0..self.l_step{
    //             self.hasher.next();
    //         }
    //         debug_assert!(end_pos == self.hasher.peek_next_pos());
    //         self.store(self.hasher.peek_next_hash(),self.hasher.peek_next_pos());
    //     }
    // }
    // pub(crate) fn seek(&mut self, pos:usize){
    //     //debug_assert!(self.hasher.peek_next_pos() <= pos, "self.hasher.peek_next_pos({}) > pos({})",self.hasher.peek_next_pos(),pos);
    //     debug_assert!(pos % self.l_step == 0, "pos({}) must be divisible by l_step({})",pos,self.l_step);
    //     //let aligned_pos = self.align_pos(pos);
    //     if let Some(hash) = self.hasher.seek(pos) {
    //         self.store(hash,pos)
    //     }
    // }
    // pub(crate) fn center_on(&mut self, cur_o_pos:usize){
    //     if let Some((seek_pos, diff_steps)) = calculate_window_advancement(
    //         cur_o_pos, self.src_len, self.trgt_len, self.half_win_size, self.max_end_pos, self.hasher.peek_next_pos(), self.l_step, self.hasher.peek_next_pos()
    //     ) {
    //         if diff_steps > 1 {
    //             self.seek(seek_pos);
    //         }
    //         for _ in 0..diff_steps {
    //             self.advance_and_store();
    //         }
    //     }
    // }

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
pub(crate) fn add_start_positions_to_matcher(matcher: &mut SrcMatcher, cur_o_pos: usize, src: &[u8]) {
    if cur_o_pos < matcher.next_hash_pos{
        return;
    }
    dbg!(matcher.cur_window_end,matcher.next_hash_pos,matcher.max_match_pos,cur_o_pos);
    if let Some(range) = calculate_window_range(cur_o_pos, matcher.src_len, matcher.trgt_len, matcher.half_win_size, matcher.max_end_pos, matcher.cur_window_end, matcher.l_step,matcher.max_match_pos) {
        dbg!(&range);
        //Put the new positions in, in reverse order
        //Reverse, because later positions are less likely to be used.
        //The hash table only keeps the last hash for a given hash
        //So hash/pos nearest our current position we want to ensure we keep.
        //By going in reverse, any collisions/duplicate starts will be evicting matches later in the src file.
        //The idea is that similar files will have similar offsets.
        //Very different files will always suffer from poor alignment and missing matches.
        //That is why it is best to use TrgtMatcher as well as secondary compression and not rely on the SrcMatcher alone.
        debug_assert!(range.start % matcher.l_step == 0, "range.start({}) must be divisible by l_step({})",range.start,matcher.l_step);
        //we will call this fn again after we are a 1/4 of the way through our window size
        if range.end >= matcher.max_end_pos{
            matcher.next_hash_pos = usize::MAX;
        }else{
            matcher.next_hash_pos = cur_o_pos + (matcher.half_win_size >> 1);
        }
        if matcher.l_step >= 9 {
            for pos in range.step_by(matcher.l_step).rev() {
                let hash = calculate_large_checksum(&src[pos..pos + 9]);
                matcher.store(hash, pos)
            }
        }else{
            let mut hash = calculate_large_checksum(&src[range.end - 9..range.end]);
            for pos in range.rev().skip(9) {
                hash = update_large_checksum_bwd(hash, src[pos+9], src[pos]);
                if pos % matcher.l_step == 0 {
                    matcher.store(hash, pos);
                }
            }

            // for pos in (0..range.end).rev().step_by(matcher.l_step) {
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
const HASH_CHUNK_SIZE: usize = 1 << 23;
//const DEFAULT_PREV_SIZE: usize = 1 << 18;

///Configuration for the SrcMatcher.
#[derive(Debug, Clone)]
pub struct SrcMatcherConfig{
    /// How much to advance the Large Hash between storing a src hash.
    /// Larger value means faster, but might miss good matches.
    pub l_step: usize,
    /// Max number of entries to check in the chain during matching.
    /// Larger value means more interrogation of known hashes, but this makes it slower.
    pub chain_check: usize,
    /// The maximum size of the source window.
    /// This is how many bytes to assess and store hashes for.
    /// Larger values consider more matches, but might hash excessively slowing down encoder.
    /// Leave blank for dynamic calculation.
    pub max_src_win_size: Option<usize>,
    /// The length of the hash to use for the source data.
    /// Shorter hashes do not make better matches.
    /// They usually will match decent matches, but will effectively 'chop up' better matches.
    /// Smaller hashes are faster to perform.
    /// Leave blank for dynamic calculation.
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
    /// chain_check: Max number of entries to check in the chain during matching.
    /// max_src_win_size: The maximum size of the source window.
    /// hash_win_len: The length of the hash to use for the source data. (3..=9)
    pub fn new(l_step: usize, chain_check:usize,max_src_win_size:Option<usize>,hash_win_len:Option<usize>) -> Self {
        Self { l_step, chain_check,max_src_win_size,hash_win_len}
    }
    ///Creates a new SrcMatcherConfig with the given compression level.
    /// level: The compression level to use. Must be between 0 and 9.
    /// The higher the level the more accurate the matches but slower.
    pub fn comp_level(level:usize)->Self{
        assert!(level <= 9);
        let l_step = Ranger::new(0..10, 26..=2).map(level);
        let chain_check = level/3 + 1;
        Self { l_step, chain_check, max_src_win_size: None, hash_win_len: None}
    }
    fn make_inner_config(&mut self, src_len: usize,trgt_len:usize)->InnerConfig{
        self.l_step = self.l_step.max(1);
        self.chain_check = self.chain_check.max(1);

        self.max_src_win_size = Some(self
            .max_src_win_size
            .map(|s| s.next_power_of_two().max(MIN_SRC_WIN_SIZE))
            .unwrap_or(
                DEFAULT_SRC_WIN_SIZE
                //calculate_default_win_size(src_len, trgt_len,None)
            ));

        self.hash_win_len = Some(self
            .hash_win_len
            .map(|l| l.clamp(3, 9))
            .unwrap_or_else(|| calculate_default_hash_len(src_len,trgt_len,self.max_src_win_size.unwrap())));

        InnerConfig{
            l_step:self.l_step,
            hash_win_len:self.hash_win_len.unwrap(),
            src_len,
            trgt_len,
            src_win_size:self.max_src_win_size.unwrap(),
            max_end_pos:align(src_len-self.hash_win_len.unwrap(),self.l_step),
            chain_check:self.chain_check,
        }
    }
    pub(crate) fn build(&mut self,src:&[u8],trgt_start_pos:usize,trgt:&[u8])->SrcMatcher{
        let trgt_len = trgt.len();
        let InnerConfig { l_step, hash_win_len, src_len, trgt_len, src_win_size, max_end_pos, chain_check } = self.make_inner_config(src.len(),trgt_len);
        let max_fwd_hash_pos = trgt.len() - 9;
        let (fwd_hash,fwd_pos) = if trgt_start_pos < max_fwd_hash_pos {
            (calculate_large_checksum(&trgt[trgt_start_pos..trgt_start_pos+9]),trgt_start_pos)
        }else{
            (0,max_fwd_hash_pos)
        };
        let table = BasicHashTable::new(src_win_size/l_step, hash_win_len<=4);
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

//Thought I could cut down encoding time, but this misses lots of good matches on dissimilar files of similar length
#[allow(dead_code)]
fn calculate_default_win_size(src_len: usize, trgt_len: usize,max_win_size:Option<usize>) -> usize {
    let mut win_size = (src_len).abs_diff(trgt_len).next_power_of_two();
    if win_size <= MIN_SRC_WIN_SIZE {
        win_size = win_size + MIN_SRC_WIN_SIZE;
    }
    let upper_bound = src_len.next_power_of_two().min(max_win_size.map(|a|a.next_power_of_two()).unwrap_or(DEFAULT_SRC_WIN_SIZE));
    win_size.min(upper_bound)
}

fn calculate_default_hash_len(src_len: usize, trgt_len: usize,src_win_size:usize) -> usize {
    let diff = src_len.abs_diff(trgt_len);
    if diff > src_win_size{return 9} //this suggests a large change on large files.
    let diff_h_len = Ranger::new(MIN_SRC_WIN_SIZE..DEFAULT_SRC_WIN_SIZE, 9..=5).map(diff);
    let s_hash_len = src_hash_len(src_len);
    if src_len >= src_win_size{
        //we have relatively small diff, but a larger src file
        return diff_h_len.max(s_hash_len)
    }
    //if we are here we have a relatively small file
    //we might want a smaller hasher since it is changed significantly from src
    src_hash_len(src_len).min(src_hash_len(trgt_len))

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

#[inline(always)]
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
            (200, 190, Some((1868, 64))),     // Max valid trgt end
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