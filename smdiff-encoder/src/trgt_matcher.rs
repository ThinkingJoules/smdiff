use crate::{hasher::*, hashmap::{BasicHashTable, ChainList}, Ranger};



pub(crate) struct TrgtMatcher{
    compress_early_exit:usize,
    chain_check: usize,
    pub(crate) fwd_hash: u32,
    pub(crate) fwd_pos: usize,
    pub(crate) max_fwd_hash_pos: usize,
    table: BasicHashTable,
    chain: ChainList,
}

impl TrgtMatcher {
    // pub(crate) fn hash_win_len(&self)->usize{
    //     self.hash_win_len
    // }
    // pub(crate) fn peek_next_pos(&self) -> usize{
    //     self.hasher.peek_next_pos()
    // }
    // pub(crate) fn peek_next_hash(&self) -> usize{
    //     self.hasher.peek_next_hash()
    // }
    // pub(crate) fn advance_and_store(&mut self) {
    //     //let start_pos = hasher.peek_next_pos();
    //     //dbg!(start_pos);
    //     self.hasher.next();
    //     self.store(self.hasher.peek_next_hash(),self.hasher.peek_next_pos());
    // }
    // pub(crate) fn seek(&mut self, pos:usize){
    //     debug_assert!(self.hasher.peek_next_pos() <= pos, "self.hasher.peek_next_pos({}) > pos({})",self.hasher.peek_next_pos(),pos);
    //     if let Some(_hash) = self.hasher.seek(pos) {
    //         //don't store the hash here. We seek before we query,
    //         // so we want to store using the advance_and_store method.
    //         // self.store(hash,pos)
    //     }
    // }
    ///Returns (trgt_pos, post_match) post_match *includes* the hash_win_len.
    pub fn find_best_trgt_match(&self,trgt:&[u8],min_match:usize)->Option<(usize,usize)>{
        let cur_hash = self.fwd_hash as usize;
        let table_pos = self.table.get(cur_hash)?;
        let mut iter = std::iter::once(table_pos).chain(self.chain.iter_prev_starts(table_pos, self.fwd_pos,cur_hash)).filter(|start|start + 4 < self.fwd_pos);
        let mut chain = if min_match > 4 {(self.chain_check/4).max(1)} else {self.chain_check};
        let mut best = None;
        let mut best_len = 0;
        let mut _chain_len = 0;
        let mut _collisions = 0;
        loop {
            if chain == 0{
                break;
            }
            if let Some(start_pos) = iter.next() {
                _chain_len += 1;
                //first verify hash matches the data
                let initial_match = trgt[start_pos..start_pos + 4]
                .iter().zip(trgt[self.fwd_pos..self.fwd_pos + 4].iter())
                .all(|(a,b)| a == b);
                if !initial_match{
                    // dbg!(&trgt[start_pos..start_pos + hash_len], &trgt[cur_o_pos..cur_o_pos + hash_len],cur_o_pos,start_pos);
                    // panic!();
                    _collisions += 1;
                    continue;
                }

                // Extend forward
                let match_end = start_pos + 4;
                let trgt_end = self.fwd_pos + 4;
                let src_remain = self.fwd_pos - match_end;
                let trgt_remain = trgt.len() - trgt_end;
                let post_match = (0..src_remain.min(trgt_remain)).take_while(|&i| {
                    trgt[match_end + i] == trgt[trgt_end + i]
                }).count();
                let total_post_match = post_match + 4;
                if total_post_match > best_len{
                    best_len = total_post_match;
                    best = Some((start_pos,total_post_match));
                    if best_len >= self.compress_early_exit{
                        break;
                    }
                }
                chain -= 1;
            }else{break;}

        }
        // if _collisions > 0{
        //     dbg!(_chain_len,_collisions,self.fwd_pos);
        //     if self.fwd_pos > 100{
        //         panic!();
        //     }
        // }
        // println!("MaxCheck: {}, Chain checked: {}, Fwd pos: {}, Collisions:{}, Best:{:?}",if min_match > 4 {(self.chain_check/4).max(1)} else {self.chain_check},_chain_len,self.fwd_pos,_collisions,best);
        // std::thread::sleep(std::time::Duration::from_millis(100));
        best
    }
    pub(crate) fn store(&mut self, hash:usize, pos:usize){
        match self.table.insert(hash, pos){
            Ok(None) => {},
            Ok(Some(prev)) => {
                self.chain.insert(hash, pos, prev);
            },
            Err((old_hash,prev_pos)) => {
                self.chain.insert(old_hash, pos, prev_pos);
            }
        }
    }
}





const DEFAULT_TRGT_WIN_SIZE: usize = 1 << 23;
const DEFAULT_PREV_SIZE: usize = 1 << 18;
///Configuration for the TrgtMatcher.
#[derive(Debug, Clone)]
pub struct TrgtMatcherConfig{
    /// If the small match (in trgt) is >= than this we stop searching the chain for better matches and use this one.
    pub compress_early_exit:usize,
    /// Max number of entries to check in the chain during matching.
    /// Larger value means more accurate matches but slower.
    /// `compress_early_exit` stops checking the chain,
    /// this value is the fallback in case the chain is long, and has no good matches.
    pub chain_check: usize,
    /// How many historical hashes to store if we find multiple start points for a given hash.
    /// This memory is shared across all hashes. Leave blank for dynamic calculation.
    pub prev_table_capacity: Option<usize>,
}

impl Default for TrgtMatcherConfig {
    fn default() -> Self {
        Self::comp_level(3)
    }
}
impl TrgtMatcherConfig {
    ///Creates a new TrgtMatcherConfig with the given compression level.
    /// level: The compression level to use. Must be between 0 and 9.
    /// The higher the level the more accurate the matches but slower.
    pub fn comp_level(level:usize)->Self{
        assert!(level <= 9);
        let compress_early_exit = Ranger::new(0..10, 6..=70).map(level);
        let chain_check = Ranger::new(0..10, 1..=33).map(level);
        Self { compress_early_exit, chain_check, prev_table_capacity: None}
    }
    pub fn with_table_capacity(mut self, table_capacity:usize)->Self{
        self.prev_table_capacity = Some(table_capacity);
        self
    }
    pub(crate) fn build(&mut self,trgt:&[u8],trgt_start_pos:usize)->TrgtMatcher{
        let Self { compress_early_exit, chain_check, prev_table_capacity } = self;
        let effective_len = trgt.len() - trgt_start_pos;
        // self.prev_table_capacity =  Some(self.prev_table_capacity
        //     .unwrap_or_else(||{
        //         let exact = max_unique_substrings_gt_hash_len(*win_size, effective_len, 1);
        //         exact.next_power_of_two() >> 1
        //     }));
        prev_table_capacity.get_or_insert(DEFAULT_PREV_SIZE.min(effective_len.next_power_of_two()>>1));
        //let table = BasicHashTable::new(DEFAULT_TRGT_WIN_SIZE.min((effective_len + (effective_len/2)).next_power_of_two() >> 1), self.prev_table_capacity.unwrap(),if *win_size>4{8}else{4});
        let table = BasicHashTable::new(DEFAULT_TRGT_WIN_SIZE.min((effective_len + (effective_len/2)).next_power_of_two() >> 1), true);
        let mut matcher = TrgtMatcher{
            compress_early_exit: *compress_early_exit,
            chain_check: *chain_check,
            fwd_hash: 0,
            fwd_pos: trgt_start_pos,
            table,
            chain: ChainList::new(self.prev_table_capacity.unwrap()),
            max_fwd_hash_pos: trgt.len().saturating_sub(4),
        };
        if trgt_start_pos > 0 { //prefill with hash start positions.
            let start = trgt_start_pos.saturating_sub(self.prev_table_capacity.unwrap());
            let end = trgt_start_pos;
            let mut hash = calculate_small_checksum(&trgt[start..]);
            matcher.store(hash as usize, start);
            for old_pos in start..end{
                hash = update_small_checksum_fwd(hash, trgt[old_pos], trgt[old_pos + 4]);
                matcher.store(hash as usize, old_pos + 1);
            }
        }
        matcher
    }
}
