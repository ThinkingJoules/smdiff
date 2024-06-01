
use core::panic;
pub enum LargeHashCursor<'a>{
    Direct(SmallHashCursor<'a>),
    Rolling(RollingHashCursor<'a>),
}

impl<'a> LargeHashCursor<'a> {
    pub fn new(slice:&'a [u8], hash_window_len: usize) -> Self {
        if (3..=4).contains(&hash_window_len) {
            let cursor = SmallHashCursor::new(slice, hash_window_len);
            LargeHashCursor::Direct(cursor)
        } else if (5..=9).contains(&hash_window_len){
            let config = RollingHashConfig::new(hash_window_len);
            let cursor = RollingHashCursor::new(slice,hash_window_len, &config);
            LargeHashCursor::Rolling(cursor)
        }else{
            panic!("Valid hash window sizes are 3-9")
        }
    }
    pub fn data_len(&self) -> usize {
        match self {
            LargeHashCursor::Direct(cursor) => {
                cursor.slice.len()
            }
            LargeHashCursor::Rolling(cursor) => {
                cursor.slice.len()
            }
        }
    }
    pub(crate) fn win_size(&self) -> usize {
        match self {
            LargeHashCursor::Direct(cursor) => {
                cursor.win_size()
            }
            LargeHashCursor::Rolling(cursor) => {
                cursor.rolling_hash.win_len
            }
        }

    }
}

impl<'a> HasherCusor for LargeHashCursor<'a> {
    fn next(&mut self) -> Option<(usize, usize)> {
        match self {
            LargeHashCursor::Direct(cursor) => {
                cursor.next()
            }
            LargeHashCursor::Rolling(cursor) => {
                cursor.next()
            }
        }
    }

    fn seek(&mut self, pos: usize) -> Option<usize> {
        match self {
            LargeHashCursor::Direct(cursor) => {
                cursor.seek(pos)
            }
            LargeHashCursor::Rolling(cursor) => {
                cursor.seek(pos)
            }
        }
    }
    fn peek_next_pos(&self) -> usize {
        match self {
            LargeHashCursor::Direct(cursor) => {
                cursor.peek_next_pos()
            }
            LargeHashCursor::Rolling(cursor) => {
                cursor.peek_next_pos()
            }
        }
    }
    fn peek_next_hash(&self) -> usize {
        match self {
            LargeHashCursor::Direct(cursor) => {
                cursor.peek_next_hash()
            }
            LargeHashCursor::Rolling(cursor) => {
                cursor.peek_next_hash()
            }
        }
    }
}

pub trait HasherCusor{
    /// Returns the (next hash, position in the slice).
    fn next(&mut self) -> Option<(usize, usize)>;
    /// Seeks to a specific position in the slice and returns the hash (or None if the position is invalid)
    fn seek(&mut self, pos: usize) -> Option<usize>;
    /// Current position in the slice
    fn peek_next_pos(&self) -> usize;
    /// Hash of the current position
    fn peek_next_hash(&self) -> usize;
}

const HASH_MULTIPLIER_32_BIT: u32 = 1597334677;
const HASH_MULTIPLIER_64_BIT: u64 = 1181783497276652981;
#[derive(Clone, Debug)]
pub(crate) struct RollingHashConfig {
    hash_window_len: usize,     // Size of the data window for hash calculations
    multiplicative_factor: usize, // Precomputed factor for hash updates
    precomputed_powers: Vec<usize>, // Precomputed powers of the multiplier
}

impl RollingHashConfig {
    fn new(hash_window_len: usize) -> Self {
        // Determine the appropriate multiplier based on target pointer width
        let hash_multiplier = if cfg!(target_pointer_width = "64") {
            HASH_MULTIPLIER_64_BIT as usize
        } else {
            HASH_MULTIPLIER_32_BIT as usize
        };

        let mut precomputed_powers = vec![1usize; hash_window_len];
        for i in (0..hash_window_len - 1).rev() {
            precomputed_powers[i] = precomputed_powers[i + 1].wrapping_mul(hash_multiplier);
        }

        Self {
            hash_window_len,
            multiplicative_factor: precomputed_powers[0].wrapping_mul(hash_multiplier),
            precomputed_powers,
        }
    }
    pub(crate) fn calculate_large_checksum(&self, data: &[u8]) -> usize {
        assert_eq!(data.len(), self.hash_window_len);
        data.iter()
            .zip(self.precomputed_powers.iter())
            .fold(0, |acc, (byte, power)|{
                acc.wrapping_add(power.wrapping_mul(*byte as usize))
                //acc.wrapping_mul(*power).wrapping_add(*byte as usize)
            })
    }


    #[cfg(target_pointer_width = "64")]
    pub(crate) fn update_large_checksum(&self, checksum: usize, old:u8, new:u8) -> usize {
        (HASH_MULTIPLIER_64_BIT as usize).wrapping_mul(checksum)
        .wrapping_sub(self.multiplicative_factor.wrapping_mul(old as usize))
        .wrapping_add(new as usize)
    }

    #[cfg(target_pointer_width = "32")]
    pub(crate) fn update_large_checksum(&self, checksum: usize, old:u8, new:u8) -> usize{
        (HASH_MULTIPLIER_32_BIT as usize).wrapping_mul(checksum)
        .wrapping_sub(self.multiplicative_factor.wrapping_mul(old as usize))
        .wrapping_add(new as usize)
    }

}

#[derive(Clone, Debug)]
pub(crate) struct RollingHasher {
    config: RollingHashConfig,
    hash: usize,
    win: [u8; 9],
    mod_pos: usize,
    win_len: usize,
}

impl RollingHasher {
    pub(crate) fn new(initial_win: &[u8],config:&RollingHashConfig) -> Self {
        assert_eq!(initial_win.len(), config.hash_window_len);
        assert!(config.hash_window_len <= 9);
        let win_len = config.hash_window_len;
        let mut win = [0; 9];
        win[..win_len].copy_from_slice(&initial_win);
        let hash = config.calculate_large_checksum(&initial_win);
        Self {
            config: config.clone(),
            hash,
            win,
            mod_pos: 0,
            win_len,
        }
    }

    fn jump(&mut self, new_win: &[u8]) {
        assert_eq!(new_win.len(), self.win_len);
        self.hash = self.config.calculate_large_checksum(new_win);
        (&mut self.win[..self.win_len]).copy_from_slice(&new_win[..self.win_len]);
        self.mod_pos = 0;
    }

    pub(crate) fn update(&mut self, new_char: u8) {
        let old_char = self.win[self.mod_pos];
        self.hash = self.config.update_large_checksum(self.hash, old_char, new_char);
        self.win[self.mod_pos] = new_char;
        self.mod_pos = (self.mod_pos + 1) % self.win_len;
    }

    pub(crate) fn hash(&self) -> usize {
        self.hash
    }
}



#[derive(Clone, Debug)]
pub(crate) struct RollingHashCursor<'a> {
    rolling_hash: RollingHasher,
    slice: &'a [u8],
    rolling_hash_start_pos: usize,
}
impl<'a> RollingHashCursor<'a>{
    pub(crate) fn new(slice: &'a [u8], win_size:usize, config:&RollingHashConfig) -> Self {
        let rolling_hash = RollingHasher::new(&slice[..win_size],config);
        RollingHashCursor {
            rolling_hash,
            slice,
            rolling_hash_start_pos: 0,
        }
    }
}
impl<'a> HasherCusor for RollingHashCursor<'a> {
    fn next(&mut self) -> Option<(usize, usize)> {
        let end_pos = self.rolling_hash_start_pos + self.rolling_hash.win_len;
        if end_pos > self.slice.len() {
            return None;
        }
        let hash = self.rolling_hash.hash();
        let start_pos = self.rolling_hash_start_pos;
        if end_pos < self.slice.len() {
            self.rolling_hash.update(self.slice[end_pos]);
        }
        self.rolling_hash_start_pos += 1;
        Some((hash, start_pos))
    }
    fn seek(&mut self, pos: usize)-> Option<usize> {
        if pos == self.rolling_hash_start_pos{
            return Some(self.rolling_hash.hash());
        }
        let end_pos = pos + self.rolling_hash.win_len;
        if end_pos > self.slice.len() {
            return None;
        }
        let prev_end = self.rolling_hash_start_pos + self.rolling_hash.win_len;
        //if self.start_pos..self.start_pos + self.rolling_hash.win_len.contains(&pos) we should just call next the right amount of times
        if (self.rolling_hash_start_pos..prev_end).contains(&pos) && pos > self.rolling_hash_start_pos{
            let diff = pos - self.rolling_hash_start_pos;
            for _ in 0..diff{
                self.next();
            }
            debug_assert_eq!(self.rolling_hash_start_pos,pos);
            return Some(self.rolling_hash.hash());
        }
        //else make a new hash
        self.rolling_hash_start_pos = pos;
        self.rolling_hash.jump(&self.slice[pos..end_pos]);
        return Some(self.rolling_hash.hash());
    }
    fn peek_next_pos(&self) -> usize {
        self.rolling_hash_start_pos
    }
    fn peek_next_hash(&self) -> usize {
        self.rolling_hash.hash()
    }
}
pub struct SmallHashCursor<'a> {
    slice: &'a [u8],
    rolling_hash_start_pos: usize,
    cur_hash: usize,
    win_size: usize,
}
impl<'a> SmallHashCursor<'a> {
    pub(crate) fn win_size(&self) -> usize {
        self.win_size
    }
    pub(crate) fn new(slice: &'a [u8], win_size: usize) -> Self {
        assert!((3..=4).contains(&win_size));
        let mut crsr = SmallHashCursor {
            slice,
            rolling_hash_start_pos: 0,
            cur_hash: 0,
            win_size,
        };
        crsr.cur_hash = crsr.calc_checksum(0);
        crsr
    }
    fn calc_checksum(&self,position:usize) -> usize {
        let state = if self.win_size == 4 {
            u32::from_ne_bytes(self.slice[position..position + 4].try_into().unwrap())
        } else {
            u32::from_be_bytes([self.slice[position], self.slice[position+1], self.slice[position+2], 0])
        };
        state.wrapping_mul(HASH_MULTIPLIER_32_BIT) as usize
    }
}
impl HasherCusor for SmallHashCursor<'_> {
    fn next(&mut self) -> Option<(usize, usize)> {
        let end_pos = self.rolling_hash_start_pos + self.win_size;
        if end_pos > self.slice.len() {
            return None;
        }
        let hash = self.cur_hash;
        let start_pos = self.rolling_hash_start_pos;
        self.rolling_hash_start_pos += 1;
        self.cur_hash = self.calc_checksum(self.rolling_hash_start_pos);
        Some((hash, start_pos))
    }
    fn seek(&mut self, pos: usize) -> Option<usize> {
        if pos == self.rolling_hash_start_pos {
            return Some(self.cur_hash);
        }
        let end_pos = pos + self.win_size;
        if end_pos > self.slice.len() {
            return None;
        }
        self.rolling_hash_start_pos = pos;
        Some(self.calc_checksum(pos))
    }
    fn peek_next_pos(&self) -> usize {
        self.rolling_hash_start_pos
    }
    fn peek_next_hash(&self) -> usize {
        self.cur_hash
    }
}




#[cfg(test)]
mod test_super {
    use super::*;
    #[test]
    fn test_initial_hash() {
        let initial_data = b"hello world, this is a test of some hashing"; // Example data
        let config = RollingHashConfig::new(8);
        let mh = RollingHasher::new(&initial_data[..8],&config);
        dbg!(mh.hash());
        assert_eq!(mh.hash, config.calculate_large_checksum(&initial_data[..8]));
    }

    #[test]
    fn test_rolling_hash() {
        let initial_data = b"hello world, this is a test of the rolling hash"; // Longer example
        let config = RollingHashConfig::new(8);
        let mut mh = RollingHasher::new(&initial_data[..8],&config);
        let mut crsr = RollingHashCursor::new(initial_data,8,&config);
        // Simulate a rolling window
        let mut hash = config.calculate_large_checksum(&initial_data[..8]);
        let (crsr_hash,_) = crsr.next().unwrap();
        assert_eq!(crsr_hash, hash);
        for i in 1..(initial_data.len() - 20) {
            let old_char = initial_data[i - 1];
            let new_char = initial_data[i + 7];
            let expected_hash = config.calculate_large_checksum(&initial_data[i..i+8]);
            hash = config.update_large_checksum(hash, old_char, new_char);
            assert_eq!(hash,expected_hash, "config.update Failed at starting index {}", i);
            mh.update(new_char);
            assert_eq!(mh.hash(), expected_hash, "RollingHash.update Failed at starting index {}", i);
            let (hash2,pos) = crsr.next().unwrap();
            assert_eq!(pos,i, "HashCrsr wrong output position");
            assert_eq!(hash2, expected_hash, "HashCrsr.next Failed at starting index {}", i);
        }
    }

    #[test]
    fn test_hash_cursor() {
        let data = b"hello world, this is a test of the rolling hash";
        let config = RollingHashConfig::new(8);
        let mut cursor = RollingHashCursor::new(data,8,&config);
        let mut answers = Vec::new();
        for _ in 0..5{
            let (hash,pos) = cursor.next().unwrap();
            answers.push((hash,pos));
        }
        answers.sort_by(|a,b|a.0.cmp(&b.0));
        for (hash,pos) in answers{
            let new_hash = cursor.seek(pos).unwrap();
            assert_eq!(hash,new_hash);
        }
    }
    // #[test]
    // fn test_size() {

    //     dbg!(MediumHashConfig::new(55739802/2, 9));
    //     dbg!(HASH_MULTIPLIER_64_BIT.wrapping_pow(8));
    // }
}