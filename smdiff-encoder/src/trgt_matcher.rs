use crate::{hasher::{HasherCusor, SmallHashCursor}, hashmap::{BasicHashTable, HashTable}};



pub(crate) struct TrgtMatcher<'a>{
    compress_early_exit:usize,
    chain_check: usize,
    hasher: SmallHashCursor<'a>,
    table: BasicHashTable,
}

impl<'a> TrgtMatcher<'a> {
    pub(crate) fn new(compress_early_exit: usize, chain_check: usize, hasher: SmallHashCursor<'a>, table: BasicHashTable) -> Self {
        Self { compress_early_exit, chain_check, hasher, table }
    }

    pub(crate) fn hash_win_len(&self)->usize{
        self.hasher.win_size()
    }
    pub(crate) fn advance_and_store(&mut self) {
        if let Some((hash,pos)) = self.hasher.next(){
            self.store(hash,pos)
        }
    }
    pub(crate) fn seek(&mut self, pos:usize){
        assert!(self.hasher.pos() <= pos);
        if let Some(hash) = self.hasher.seek(pos) {
            self.store(hash,pos)
        }
    }
    ///Returns (trgt_pos, post_match) post_match *includes* the hash_win_len.
    pub fn find_best_trgt_match(&self,trgt:&[u8],cur_o_pos:usize,hash:usize)->Option<(usize,usize)>{
        let table_pos = self.table.get_last_pos(self.table.calc_index(hash))?;
        let mut iter = std::iter::once(table_pos).chain(self.table.iter_prev_starts(table_pos, cur_o_pos));
        let mut chain = self.chain_check;
        let mut best = None;
        let mut best_len = 0;
        let initial_len = self.hash_win_len();
        loop {
            if chain == 0{
                break;
            }
            if let Some(start_pos) = iter.next() {
                //first verify hash matches the data
                let initial_match = trgt[start_pos..start_pos + initial_len]
                .iter().zip(trgt[cur_o_pos..cur_o_pos + initial_len].iter())
                .all(|(a,b)| a == b);
                if !initial_match{
                    continue;
                }

                // Extend forward
                let match_end = start_pos + initial_len;
                let trgt_end = cur_o_pos + initial_len;
                let src_remain = cur_o_pos - match_end;
                let trgt_remain = trgt.len() - trgt_end;
                let post_match = (0..src_remain.min(trgt_remain)).take_while(|&i| {
                    trgt[match_end + i] == trgt[trgt_end + i]
                }).count();
                let total_post_match = post_match + initial_len;
                if total_post_match > best_len{
                    best_len = total_post_match;
                    best = Some((start_pos,total_post_match));
                }
                chain -= 1;
            }else{break;}

        }
        best
    }
    fn store(&mut self, hash:usize, pos:usize){
        let idx = self.table.calc_index(hash);
        self.table.insert(idx, pos);
    }
}





pub const DEFAULT_TRGT_WIN_SIZE: usize = 1 << 23;
pub const DEFAULT_PREV_SIZE: usize = 1 << 18;
#[derive(Debug, Clone)]
pub struct TrgtMatcherConfig{
    /// If the small match (in trgt) is >= than this we stop searching for better matches and emit this one.
    pub compress_early_exit:usize,
    /// Max number of entries to check in the chain during matching.
    /// Larger value means more accurate matches but slower.
    pub chain_check: usize,
    ///Advanced setting, leave as None for default.
    pub prev_table_capacity: Option<usize>,
    pub hash_win_len: Option<usize>
}
impl TrgtMatcherConfig {
    ///Creates a new TrgtMatcherConfig with the given parameters.
    /// compress_early_exit: If the small match (in trgt) is >= than this we stop searching for better matches and emit this one.
    /// chain_check: Max number of entries to check in the chain during matching.
    /// table_capacity: Advanced setting, leave as None for default.
    pub fn new(compress_early_exit: usize, chain_check: usize, prev_table_capacity: Option<usize>,hash_win_len:Option<usize>) -> Self {
        Self { compress_early_exit, chain_check, prev_table_capacity,hash_win_len}
    }

    ///Creates a new TrgtMatcherConfig with the given compression level.
    /// level: The compression level to use. Must be between 0 and 9.
    /// The higher the level the more accurate the matches but slower.
    pub fn new_from_compression_level(level:usize)->Self{
        assert!(level <= 9);
        let compress_early_exit = 6 + (level*64 / 9);
        let chain_check = 1 + ((65 * level) / 9);
        let extra_short_matches = level >= 6;
        Self { compress_early_exit, chain_check, prev_table_capacity: None , hash_win_len: None}
    }
    pub fn with_table_capacity(mut self, table_capacity:usize)->Self{
        self.prev_table_capacity = Some(table_capacity);
        self
    }
    pub fn build<'a>(&mut self,trgt:&'a [u8],trgt_start_pos:usize)->TrgtMatcher<'a>{
        let Self { compress_early_exit, chain_check, prev_table_capacity,hash_win_len } = self;
        let effective_len = trgt.len() - trgt_start_pos;
        let prev_table_capacity = prev_table_capacity.get_or_insert(DEFAULT_PREV_SIZE.min(effective_len));
        let win_size = hash_win_len.get_or_insert(trgt_hash_len(effective_len));
        let hasher = SmallHashCursor::new(trgt, *win_size);
        let table = BasicHashTable::new(DEFAULT_TRGT_WIN_SIZE.min(effective_len), *prev_table_capacity);
        let mut matcher = TrgtMatcher::new(*compress_early_exit, *chain_check, hasher, table );
        if trgt_start_pos > 0 { //prefill with hash start positions.
            let start = trgt_start_pos.saturating_sub(*prev_table_capacity);
            let end = trgt_start_pos;
            matcher.seek(start);
            for _ in start..end{
                matcher.advance_and_store()
            }
        }
        matcher
    }
}

pub fn trgt_hash_len(len:usize)->usize{
    if len <= 127{
        3
    }else{
        4
    }
}