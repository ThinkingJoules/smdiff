
use smdiff_common::MAX_INST_SIZE;

use crate::{addr_cost, CopyScore};
const INITIAL_VALUE: u32 = 0; //5381;  // Not sure how to roll non-zero initial value
pub const MULTIPLICATVE: u32 = 257;
/// Returns (start_pos, match_len) of the longest match in `chunks` for `trgt_exact`.
pub fn find_sub_string_in_src(src_slice: &[u8],chunks:&[ChunkHashMap],trgt_slice:&[u8],trgt_hash:u32,hash_len:usize,trgt_start_pos:usize,last_addr:u64)->Option<CopyScore>{
    let mut best = None;
    for chunk_map in chunks {
        let found = find_substring_in_src(src_slice,chunk_map,&trgt_slice, trgt_hash,hash_len,trgt_start_pos,last_addr);
        if found > best {
            best = found;
        }
        // if let Some(score) = find_substring_in_src(src_slice,chunk_map,&trgt_slice, trgt_hash,hash_len,trgt_start_pos,last_addr) {
        //     if let Some((_, len)) = best {
        //         if match_len > len {
        //             best = Some((start_pos, match_len));
        //         }
        //         if match_len > MAX_INST_SIZE / 2{
        //             return Some((start_pos, match_len));
        //         }
        //     } else {
        //         best = Some((start_pos, match_len));
        //     }
        // }
    }
    best
}

pub fn find_substring_in_src(slice: &[u8],map:&ChunkHashMap,trgt_slice:&[u8],trgt_hash:u32,hash_len:usize,trgt_start_pos:usize,last_addr:u64)->Option<CopyScore>{
    if let Some(start_positions) = map.get(trgt_hash) {
        return find_longest_match(slice,start_positions, trgt_slice, hash_len,trgt_start_pos,last_addr);
    }
    None
}


pub fn find_longest_match(src_slice: &[u8], positions: &[u32], trgt_slice:&[u8],hash_len:usize,trgt_start_pos:usize,last_addr:u64) -> Option<CopyScore> {
    // Iterate over each position to use as the starting point for comparison
    // if positions.len() > 500{
    //     dbg!("START",positions.len());
    // }
    //let start = std::time::Instant::now();
    let mut best_score = CopyScore::new(isize::MIN, 0, 0);
    for &start_pos in positions {
        if start_pos as usize + best_score.size > src_slice.len() {
            break;
        }
        let end_pos = start_pos as usize + hash_len;
        let base_subslice = &src_slice[start_pos as usize..end_pos];
        let trgt_exact = &trgt_slice[trgt_start_pos..trgt_start_pos+hash_len];

        // Only consider this position if it can match the expected_match fully
        if base_subslice == trgt_exact {
            //zipper try to extend the match
            let a_cost = addr_cost(last_addr, start_pos as u64);
            let max_len = MAX_INST_SIZE.min(trgt_slice.len() - trgt_start_pos);
            if CopyScore::new(a_cost,max_len,start_pos as usize) < best_score {
                //this suffix is too short to be a better match
                //this is at the tail of the file.
                //so we can break out of the loop.
                //all future suffixes will be shorter yet
                //and the a_cost larger
                break;
            }
            let j = start_pos as usize + hash_len;
            let max_end = trgt_start_pos + max_len;
            let count = src_slice[j..].iter().zip(trgt_slice[trgt_start_pos+hash_len..max_end].iter()).take_while(|(&a, &b)| a == b).count();
            let match_len = count + hash_len;
            let score = CopyScore::new(a_cost, match_len, start_pos as usize);

            if score > best_score {
                best_score = score;
                // if match_len > early_return{
                //     return Some((max_match_pos, max_match_length));
                // }
            }
        }
    }
    // if positions.len() > 500{
    //     dbg!(start.elapsed());
    // }

    if best_score.score > 1{
        return Some(best_score);
    } else {
        None
    }
}
pub fn find_sub_string_in_trgt(chunks:&[ChunkHashMap],trgt_slice:&[u8],trgt_hash:u32,hash_len:usize,trgt_start_pos:usize,last_addr:u64)->Option<CopyScore>{
    let mut best = None;
    for chunk in chunks {
        let found = find_substring_in_trgt(chunk, &trgt_slice, trgt_hash,hash_len,trgt_start_pos,last_addr);
        if found > best {
            best = found;
        }
        // if let Some((start_pos, match_len)) = find_substring_in_trgt(chunk, &trgt_slice, trgt_hash,hash_len,trgt_start_pos,last_addr) {
        //     if let Some((_, len)) = best {
        //         if match_len > len {
        //             best = Some((start_pos, match_len));
        //         }
        //         if match_len > MAX_INST_SIZE / 2{
        //             return Some((start_pos, match_len));
        //         }
        //     } else {
        //         best = Some((start_pos, match_len));
        //     }
        // }
    }
    best
}
pub fn find_substring_in_trgt(map:&ChunkHashMap,trgt_slice:&[u8],trgt_hash:u32,hash_len:usize,trgt_start_pos:usize,last_addr:u64)->Option<CopyScore>{
    if let Some(start_positions) = map.get(trgt_hash) {
        return find_longest_match_up_to(start_positions, &trgt_slice, hash_len,trgt_start_pos,last_addr);
    }
    None
}
pub fn find_longest_match_up_to(positions: &[u32], trgt_slice:&[u8],hash_len:usize,trgt_start_pos:usize,last_addr:u64) -> Option<CopyScore> {
    //let start = std::time::Instant::now();
    let mut best_score = CopyScore::new(isize::MIN, 0, 0);
    for &start_pos in positions {
        if start_pos as usize + best_score.size >= trgt_start_pos{
            break;
        }
        let end_pos = start_pos as usize + hash_len;
        let base_subslice = &trgt_slice[start_pos as usize..end_pos];
        let trgt_exact = &trgt_slice[trgt_start_pos..trgt_start_pos+hash_len];

        // Only consider this position if it can match the expected_match fully
        if base_subslice == trgt_exact {
            let a_cost = addr_cost(last_addr, start_pos as u64);
            let max_len = MAX_INST_SIZE.min(trgt_start_pos - start_pos as usize).min(trgt_slice.len() - trgt_start_pos);
            if CopyScore::new(a_cost,max_len,start_pos as usize) < best_score {
                //this suffix is too short to be a better match
                //this is at the tail of the file.
                //so we can break out of the loop.
                //all future suffixes will be shorter yet
                //and the a_cost larger
                break;
            }
            //zipper try to extend the match
            let j = start_pos as usize + hash_len;
            let max_end = trgt_start_pos + max_len;
            let count = trgt_slice[j..].iter().zip(trgt_slice[trgt_start_pos+hash_len..max_end].iter()).take_while(|(&a, &b)| a == b).count();
            let match_len = count + hash_len;
            let score = CopyScore::new(a_cost, match_len, start_pos as usize);

            if score > best_score {
                best_score = score;
                // if match_len > early_return{
                //     return Some((max_match_pos, max_match_length));
                // }
            }
        }
    }
    if best_score.score > 0{
        return Some(best_score);
    } else {
        None
    }
}
pub fn hash_chunk(slice: &[u8],abs_start_pos:u32, win_size:u32, multiplicative:u32, table_size:u32) -> ChunkHashMap {
    if slice.len() < win_size as usize {
        return ChunkHashMap::new(table_size);
    }
    let mut rolling_hash = RollingHash::new(&slice[..win_size as usize],multiplicative);
    let mut chunk_map = ChunkHashMap::new(table_size);
    chunk_map.insert(rolling_hash.hash(), abs_start_pos);
    for (i, &byte) in slice.iter().enumerate().skip(win_size as usize) {
        rolling_hash.update(byte);
        let rel_pos = i as u32 - win_size + 1;
        let abs_pos = rel_pos +abs_start_pos;
        chunk_map.insert(rolling_hash.hash(), abs_pos);
    }
    chunk_map
}
#[derive(Clone, Debug)]
pub struct HashCursor<'a> {
    rolling_hash: RollingHash,
    slice: &'a [u8],
    start_pos: usize,
}
impl<'a> HashCursor<'a>{
    pub fn new(slice: &'a [u8], win_size:u32, multiplicative:u32) -> Self {
        let rolling_hash = RollingHash::new(&slice[..win_size as usize],multiplicative);
        HashCursor {
            rolling_hash,
            slice,
            start_pos: 0,
        }
    }
    pub fn next(&mut self) -> Option<(u32, usize)> {
        let end_pos = self.start_pos + self.rolling_hash.win.len();
        if end_pos >= self.slice.len() {
            return None;
        }
        let hash = self.rolling_hash.hash();
        let start_pos = self.start_pos;
        self.rolling_hash.update(self.slice[end_pos]);
        self.start_pos += 1;
        Some((hash, start_pos))
    }
    pub fn seek(&mut self, pos: usize)-> Option<u32> {
        if pos == self.start_pos{
            return self.next().map(|(hash,start)|{
                debug_assert!(pos == start);
                hash
            });
        }
        let end_pos = pos + self.rolling_hash.win.len();
        if end_pos > self.slice.len() {
            return None;
        }
        let prev_end = self.start_pos + self.rolling_hash.win.len();
        //if self.start_pos..self.start_pos + self.rolling_hash.win.len().contains(&pos) we should just call next the right amount of times
        if (self.start_pos..prev_end).contains(&pos) && pos > self.start_pos{
            let diff = pos - self.start_pos;
            for _ in 0..diff{
                self.next();
            }
            return self.next().map(|(hash,start)|{
                debug_assert!(pos == start);
                hash
            });
        }
        //else make an new hash
        self.start_pos = pos;
        self.rolling_hash = RollingHash::new(&self.slice[pos..end_pos],self.rolling_hash.multiplicative);
        return self.next().map(|(hash,start)|{
            debug_assert!(pos == start);
            hash
        });
    }

}

#[derive(Debug,Clone,PartialEq,Eq)]
pub enum Value {
    SeenFew([u32;3]),
    SeenMany{vec_idx:usize},
}

impl Default for Value {
    fn default() -> Self {
        Value::SeenFew([u32::MAX,u32::MAX,u32::MAX])
    }
}

impl Value {
    pub fn is_many(&self) -> bool {
        matches!(self, Value::SeenMany{..})
    }
    pub fn can_push(&self) -> bool {
        match self {
            Value::SeenFew([_,_,c]) => c == &u32::MAX,
            _ => false,
        }
    }
    pub fn many(&self) -> Option<usize>{
        match self {
            Value::SeenMany{vec_idx} => Some(*vec_idx),
            _ => None,
        }
    }
    pub fn few(&self) -> Option<(usize,&[u32;3])>{
        match self {
            Value::SeenFew(arr) => {
                let count = (arr[0] != u32::MAX) as usize + (arr[1] != u32::MAX) as usize + (arr[2] != u32::MAX) as usize;
                Some((count, arr))
            },
            Value::SeenMany{..} => None,
        }
    }
    ///Panics if this is not pushable
    pub fn push(&mut self, pos:u32){
        match self {
            Value::SeenFew([a,b,c]) => {
                if c != &u32::MAX {
                    panic!("Cannot push to SeenFew with full buffer");
                }
                if a == &u32::MAX {
                    *a = pos;
                }else if b == &u32::MAX {
                    *b = pos;
                }else{
                    *c = pos;
                }
            },
            Value::SeenMany{..} => panic!("Cannot push to SeenMany"),
        }
    }
    pub fn upgrade(&mut self,vec_idx:usize)->[u32;3]{
        let (count,values) = self.few().unwrap();
        assert_eq!(count,3);
        let v = values.clone();
        *self = Value::SeenMany{vec_idx};
        v
    }
}
#[derive(Clone, Debug)]
pub struct ChunkHashMap {
    buckets: Vec<Value>,
    overflow: Vec<Vec<u32>>
}
impl ChunkHashMap {
    pub fn new(table_size:u32) -> Self {

        let buckets = vec![Value::default(); table_size.max(1) as usize];
        ChunkHashMap { buckets, overflow: Vec::new() }
    }
    pub fn insert(&mut self, rolling_hash_value: u32, start_pos: u32) {
        let index = rolling_hash_value as usize % self.buckets.len();
        let value = &mut self.buckets[index];
        if value.can_push() {
            value.push(start_pos);
        } else if !value.is_many() {
            let vec_idx = self.overflow.len();
            let values = value.upgrade(vec_idx);
            let mut vec = values.to_vec();
            vec.push(start_pos);
            self.overflow.push(vec);
        }else{
            let vec_idx = value.many().unwrap();
            self.overflow[vec_idx].push(start_pos);
        }
    }
    fn get_bucket(&self, rolling_hash_value: u32) -> &Value {
        let index = rolling_hash_value as usize % self.buckets.len();
        &self.buckets[index]
    }
    // pub fn contains_key(&self, rolling_hash_value: u32) -> bool {
    //     if let Some((count,_)) = self.get_bucket(rolling_hash_value).few(){
    //         return count > 0;
    //     }
    //     true //many means it exists
    // }
    pub fn get(&self, rolling_hash_value: u32) -> Option<&[u32]> {
        let bucket = self.get_bucket(rolling_hash_value);
        match bucket {
            Value::SeenFew(arr) => {
                let count = (arr[0] != u32::MAX) as usize + (arr[1] != u32::MAX) as usize + (arr[2] != u32::MAX) as usize;
                if count == 0 {
                    return None;
                }
                return Some(&arr[..count]);
            }
            Value::SeenMany{vec_idx} => {
                return Some(&self.overflow[*vec_idx]);
            }
        }
    }
    pub fn num_hashes(&self) -> usize {
        self.buckets.iter().filter(|x| matches!(x,Value::SeenFew(arr) if arr[0] != u32::MAX) || matches!(x,Value::SeenMany { .. })).count()
    }

}

#[derive(Clone, Debug)]
pub struct RollingHash {
    hash: u32,
    win: Vec<u8>,
    pos: usize,
    multiplicative: u32,
    sub_pow: u32,
}

impl RollingHash {
    pub fn new(initial_win: &[u8],multiplicative:u32) -> Self {
        //multiplicative must be odd
        assert!(multiplicative % 2 == 1, "Multiplicative must be odd");
        let hash = hash_multiplicative(initial_win,multiplicative);
        Self {
            hash,
            win: initial_win.to_vec(),
            pos: 0,
            multiplicative,
            sub_pow: multiplicative.wrapping_pow(initial_win.len() as u32 - 1),}
    }

    pub fn update(&mut self, new_char: u8) {
        self.hash = roll_multiplicative_hash(self.hash, self.win[self.pos], new_char, self.multiplicative,self.sub_pow);
        self.win[self.pos] = new_char;
        self.pos = (self.pos + 1) % self.win.len(); // Use `HASH_SIZE` for rolling
    }

    pub fn hash(&self) -> u32 {
        self.hash
    }
}
fn hash_multiplicative(key: &[u8],multiplicative:u32) -> u32 {
    let mut hash = INITIAL_VALUE;
    for (_i,&byte )in key.iter().enumerate() {
        hash = multiplicative.wrapping_mul(hash).wrapping_add(u32::from(byte));
    }
    hash
}
//Currently only works with an INITIAL_VALUE of 0
fn roll_multiplicative_hash(current_hash: u32, subtract: u8, add: u8, multiplicative: u32, sub_pow: u32) -> u32 {
    let subtract_value = sub_pow.wrapping_mul(subtract as u32);
    //let subtract_value = sub_pow.wrapping_add(subtract as u32);
    //println!("subtract_value: {}", subtract_value);  // Log intermediate value

    let minus = current_hash.wrapping_sub(subtract_value);
    //println!("after subtraction: {}", minus);  // Log after subtraction

    let new_hash = multiplicative.wrapping_mul(minus).wrapping_add(add as u32);

    //println!("final hash: {}", new_hash);  // Log final hash

    new_hash
}


#[cfg(test)]
mod test_super {

    use super::*;
    #[test]
    fn test_initial_hash() {
        let initial_data = b"hello world, this is a test of some hashing"; // Example data
        let mh32 = RollingHash::new(&initial_data[..32],33);
        assert_eq!(mh32.hash, hash_multiplicative(&initial_data[..32],33));
    }

    #[test]
    fn test_rolling_hash() {
        let initial_data = b"hello world, this is a test of the rolling hash"; // Longer example
        let mut mh = RollingHash::new(&initial_data[..20], 33);
        // Simulate a rolling window
        let mut hash = hash_multiplicative(&initial_data[..20], 33);
        for i in 1..(initial_data.len() - 20) {
            let old_char = initial_data[i - 1];
            let new_char = initial_data[i + 19];
            let expected_hash = hash_multiplicative(&initial_data[i..i+20], 33);
            hash = roll_multiplicative_hash(hash, old_char, new_char, 33, 33u32.wrapping_pow(19));
            assert_eq!(hash,expected_hash, "Failed at window starting at index {}", i);
            mh.update(new_char);
            assert_eq!(mh.hash(), expected_hash, "Failed at window starting at index {}", i);
        }
    }

    #[test]
    fn test_hash_cursor() {
        let data = b"hello world, this is a test of the rolling hash";
        let mut cursor = HashCursor::new(data,20,33);
        let mut answers = Vec::new();
        for _ in 0..5{
            let (hash,_pos) = cursor.next().unwrap();
            answers.push((hash,_pos));
        }
        answers.sort_by(|a,b|a.0.cmp(&b.0));
        for (hash,pos) in answers{
            let new_hash = cursor.seek(pos).unwrap();
            assert_eq!(hash,new_hash);
        }
    }
    #[test]
    fn test_chunk(){
        let data = b"hello world, this is a test of the rolling hash";
        let chunk = hash_chunk(data,100,20,33,50);
        let mut cursor = HashCursor::new(data,20,33);
        for pos in 0..5{
            let hash = cursor.next().unwrap().0;
            let positions = chunk.get(hash).unwrap();
            assert_eq!(positions.contains(&(pos+100)),true);
        }
    }
}
