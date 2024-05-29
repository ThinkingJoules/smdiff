use crate::{hasher::{HasherCusor, SmallHashCursor}, hashmap::{BasicHashTable, HashTable}};



pub(crate) struct TrgtMatcher<'a>{
    compress_early_exit:usize,
    chain_check: usize,
    hasher: SmallHashCursor<'a>,
    table: BasicHashTable,
}

impl<'a> TrgtMatcher<'a> {
    pub(crate) fn advance_and_store(&mut self) {
        if let Some((hash,_)) = self.hasher.next(){
            self.store(hash)
        }
    }
    pub(crate) fn seek(&mut self, pos:usize){
        assert!(self.hasher.pos() <= pos);
        if let Some(hash) = self.hasher.seek(pos) {
            self.store(hash)
        }
    }
    fn store(&mut self, hash:usize){
        let idx = self.table.calc_index(hash);
        self.table.insert(idx, hash);
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
}
impl TrgtMatcherConfig {
    ///Creates a new TrgtMatcherConfig with the given parameters.
    /// compress_early_exit: If the small match (in trgt) is >= than this we stop searching for better matches and emit this one.
    /// chain_check: Max number of entries to check in the chain during matching.
    /// table_capacity: Advanced setting, leave as None for default.
    pub fn new(compress_early_exit: usize, chain_check: usize, prev_table_capacity: Option<usize>) -> Self {
        Self { compress_early_exit, chain_check, prev_table_capacity}
    }

    ///Creates a new TrgtMatcherConfig with the given compression level.
    /// level: The compression level to use. Must be between 0 and 9.
    /// The higher the level the more accurate the matches but slower.
    pub fn new_from_compression_level(level:usize)->Self{
        assert!(level <= 9);
        let compress_early_exit = 6 + (level*64 / 9);
        let chain_check = 1 + ((65 * level) / 9);
        let extra_short_matches = level >= 6;
        Self { compress_early_exit, chain_check, prev_table_capacity: None }
    }
    pub fn with_table_capacity(mut self, table_capacity:usize)->Self{
        self.prev_table_capacity = Some(table_capacity);
        self
    }
    pub fn build(self,trgt:&[u8],trgt_start_pos:usize)->TrgtMatcher{
        let Self { compress_early_exit, chain_check, prev_table_capacity } = self;
        let effective_len = trgt.len() - trgt_start_pos;
        let prev_table_capacity = prev_table_capacity.unwrap_or(DEFAULT_PREV_SIZE.min(effective_len));
        let hasher = SmallHashCursor::new(trgt, trgt_hash_len(effective_len));
        let table = BasicHashTable::new(DEFAULT_TRGT_WIN_SIZE.min(effective_len), prev_table_capacity);
        let mut matcher = TrgtMatcher{ compress_early_exit, chain_check, hasher, table };
        if trgt_start_pos > 0 { //prefill with hash start positions.
            let start = trgt_start_pos.saturating_sub(prev_table_capacity);
            let end = trgt_start_pos;
            matcher.seek(start);
            for _ in start..end{
                matcher.advance_and_store()
            }
        }
        matcher
    }
}

fn trgt_hash_len(len:usize)->usize{
    if len <= 127{
        3
    }else{
        4
    }
}