use std::ops::Range;

use crate::{hasher::{HasherCusor, LargeHashCursor}, hashmap::{BasicHashTable, HashTable}};


pub(crate) struct SrcMatcher<'a>{
    l_step:usize,
    hash_win_len: usize,
    src_win_size:usize,
    chain_check: usize,
    hasher: LargeHashCursor<'a>,
    table: BasicHashTable,
}

impl<'a> SrcMatcher<'a> {
    pub(crate) fn hash_win_len(&self)->usize{
        self.hash_win_len
    }
    pub(crate) fn chain_check(&self)->usize{
        self.chain_check
    }
    pub(crate) fn advance_and_store(&mut self) {
        let start_pos = self.hasher.pos();
        let end_pos = start_pos + self.l_step;
        if self.l_step > self.hash_win_len{
            self.seek(end_pos)
        }else{
            for _ in 1..self.l_step{
                self.hasher.next(); //roll step amt -1
            }
            if let Some((hash,pos)) = self.hasher.next(){
                assert_eq!(end_pos,pos);
                self.store(hash,pos)
            }
        }
    }
    pub(crate) fn seek(&mut self, pos:usize){
        assert!(self.hasher.pos() <= pos);
        let aligned_pos = self.align_pos(pos);
        if let Some(hash) = self.hasher.seek(aligned_pos) {
            self.store(hash,pos)
        }
    }
    pub(crate) fn center_on(&mut self, cur_o_pos:usize){
        let end = self.align_pos((cur_o_pos + self.src_win_size / 2).min(self.hasher.data_len()));
        let cur_hash_pos = self.hasher.pos();
        if end <= cur_hash_pos{return;}
        let diff = end - cur_hash_pos;
        if diff > self.src_win_size{
            self.seek(end - self.src_win_size);
            return;
        }
        for _ in 0..=diff/self.l_step{
            self.advance_and_store();
        }
        assert!(self.hasher.pos() >= end);
    }
    ///Returns (src_pos, pre_match, post_match) post_match *includes* the hash_win_len.
    pub fn find_best_src_match(&self,src:&[u8],trgt:&[u8],cur_o_pos:usize,hash:usize)->Option<(usize,usize,usize)>{
        let table_pos = self.table.get_last_pos(self.table.calc_index(hash))?;
        let mut iter = std::iter::once(table_pos).chain(self.table.iter_prev_starts(table_pos, cur_o_pos));
        let mut chain = self.chain_check;
        let mut best = None;
        let mut best_len = 0;
        loop {
            if chain == 0{
                break;
            }
            if let Some(table_pos) = iter.next() {
                let src_pos = self.table_to_abs_pos(table_pos);
                if let Some((pre_match,post_match)) = extend_src_match(src, src_pos, trgt, cur_o_pos, self.hash_win_len) {
                    let total_post_match = post_match + self.hash_win_len;
                    if total_post_match+pre_match > best_len{
                        best_len = total_post_match+pre_match;
                        best = Some((src_pos,pre_match,total_post_match));
                    }
                    chain -= 1;
                }
            }else{break;}

        }
        best
    }
    fn align_pos(&self, pos:usize)->usize{
        pos - (pos % self.l_step)
    }
    /// Positions returned from the table are in table space, this converts them to absolute start positions.
    /// In other words, the table_pos is mutliplied by l_step to get the absolute position.
    pub(crate) fn table_to_abs_pos(&self, table_pos:usize)->usize{
        table_pos * self.l_step
    }
    fn abs_to_table_pos(&self, abs_pos:usize)->usize{
        debug_assert!(abs_pos % self.l_step == 0, "abs_pos must be divisible by l_step");
        abs_pos / self.l_step
    }
    fn store(&mut self, hash:usize, abs_pos:usize){
        let idx = self.table.calc_index(hash);
        let table_pos = self.abs_to_table_pos(abs_pos);
        self.table.insert(idx, table_pos);
    }
}

pub const DEFAULT_SRC_WIN_SIZE: usize = 1 << 26;
pub const DEFAULT_PREV_SIZE: usize = 1 << 18;
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
    pub fn new_from_compression_level(level:usize)->Self{
        assert!(level <= 9);
        let l_step = 3 * level % 25;
        //let lazy_escape_len = 6 + (level*81 / 9);
        let chain_check = 1 + level;
        Self { l_step, chain_check, prev_table_capacity: None, max_src_win_size: None, hash_win_len: None}
    }
    pub fn with_table_capacity(mut self, table_capacity:usize)->Self{
        self.prev_table_capacity = Some(table_capacity);
        self
    }
    pub fn build<'a>(&mut self,src:&'a [u8],trgt_start_pos:usize,trgt_len:usize)->SrcMatcher<'a>{
        let Self { l_step, chain_check, prev_table_capacity, max_src_win_size, hash_win_len } = self;
        let trgt_matchable_len = trgt_len - trgt_start_pos;

        max_src_win_size.get_or_insert(DEFAULT_SRC_WIN_SIZE);
        max_src_win_size.map(|x| x.min(src.len())).unwrap();
        let src_win = max_src_win_size.unwrap();
        let advance_matcher_at_trgt_pos = if trgt_matchable_len > src.len() {trgt_start_pos.max(src_win / 2)}else{trgt_len};
        let prev_table_capacity = prev_table_capacity.get_or_insert(DEFAULT_PREV_SIZE);
        let hash_win_len = hash_win_len.unwrap_or(src_hash_len(src_win));
        let hasher = LargeHashCursor::new(src, src_hash_len(trgt_matchable_len));
        let table = BasicHashTable::new(src_win/ *l_step, *prev_table_capacity);
        let mut matcher = SrcMatcher{ l_step:*l_step, hash_win_len, chain_check:*chain_check, hasher, table, src_win_size: advance_matcher_at_trgt_pos };
        //prefill with hash start positions.
        let start = trgt_start_pos.saturating_sub(src_win / 2);
        matcher.seek(start);
        for _ in (start..).take(*prev_table_capacity){
            matcher.advance_and_store();
        }
        matcher
    }
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