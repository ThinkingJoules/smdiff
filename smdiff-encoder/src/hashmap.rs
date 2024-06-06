/// An offset value to distinguish between empty bucket positions, and real offsets.
const BUCKET_VALUE_OFFSET: usize = 1;

#[derive(Clone, Debug, Default)]
pub(crate) struct BucketValue{
    hash:usize,
    value:usize,
}

impl BucketValue {
    fn get_value_unchecked(&self) -> usize {
        self.value - BUCKET_VALUE_OFFSET
    }
    pub(crate) fn get(&self,hash:usize) -> Option<usize> {
        if self.value == 0 {
            return None;
        }
        if self.hash == hash {
            Some(self.value - BUCKET_VALUE_OFFSET)
        } else {
            None
        }
    }
    pub(crate) fn set(&mut self, hash:usize, value: usize) -> Result<Option<usize>,(usize,usize)> {
        if self.value == 0 {
            self.hash = hash;
            self.value = value + BUCKET_VALUE_OFFSET;
            return Ok(None);
        }

        if self.hash == hash {
            let old = self.value;
            self.value = value + BUCKET_VALUE_OFFSET;
            return Ok(Some(old - BUCKET_VALUE_OFFSET));
        }
        let old = (self.hash, self.value - BUCKET_VALUE_OFFSET);
        self.hash = hash;
        self.value = value + BUCKET_VALUE_OFFSET;
        Err(old)
    }
}

/// A basic hash table implementation.
/// This is effectively the innards of a HashMap<K, usize> with a fixed size table.
/// The caller is responsible for hashing K to a usize value.
/// We expose the bucket index to allow bucket calculation to happen exactly once.
/// The caller is responsible for dealing with hash collisions.
/// This way the caller can avoid recalculating the index.
#[derive(Clone, Debug)]
pub(crate) struct BasicHashTable {
    shift_amount: usize,     // Bit shift amount for hash reduction
    mod_mask: usize,         // Bitmask for hash table indexing
    buckets: Vec<BucketValue>,     // Power of 2 sized array of positions
}
impl BasicHashTable{
    /// Creates a new BasicHashTable with the given number of slots.
    /// min_capacity will be rounded up to the next power of 2.
    /// If interpret_hash_as_32_bit is true, the hash will be interpreted as a 32 bit value, regardless of the std::mem::size_of::<usize>().
    /// This allows for using small hashes on a 64 bit system.
    pub(crate) fn new(min_capacity: usize, interpret_hash_as_32_bit:bool) -> Self {
        let num_bits = determine_hash_table_size_bits(min_capacity);
        let table_size = 1 << num_bits;
        let mod_mask = table_size - 1;
        //let shift_amount = (std::mem::size_of::<usize>() * 8) - num_bits;
        let base_shift = if interpret_hash_as_32_bit {4} else {std::mem::size_of::<usize>()};
        let shift_amount = (base_shift * 8) - num_bits;
        let buckets = vec![BucketValue::default(); table_size];
        Self {
            mod_mask,
            shift_amount,
            buckets,
        }
    }
}
impl BasicHashTable {
    #[inline(always)]
    pub(crate) fn get(&self,hash:usize)->Option<usize>{
        let idx = get_bucket_idx(hash, self.shift_amount, self.mod_mask);
        self.buckets[idx].get(hash)
    }
    #[inline(always)]
    pub(crate) fn insert(&mut self, hash: usize, position: usize) -> Result<Option<usize>,(usize,usize)> {
        let idx = get_bucket_idx(hash, self.shift_amount, self.mod_mask);
        self.buckets[idx].set(hash, position)
    }
    /// Inserts a new position into the hash table conditional on if the old position is old enough.
    /// * hash: The hash of the key
    /// * position: The new position to insert
    /// * old_lt: Will only insert if the to-be-evicted position is less than this value. (always inserts if unset)
    #[inline(always)]
    pub(crate) fn insert_cond(&mut self, hash: usize, position: usize,old_lt:usize) -> Result<Option<usize>,(usize,usize)> {
        let idx = get_bucket_idx(hash, self.shift_amount, self.mod_mask);
        let bucket = &mut self.buckets[idx];
        if bucket.value == 0 || bucket.value > 0 && bucket.value - BUCKET_VALUE_OFFSET < old_lt {
            return bucket.set(hash, position)
        }
        Err((hash,position)) //could not set
    }

}

/// Determines the number of bits to use for the hash table size.
fn determine_hash_table_size_bits(slots: usize) -> usize {
    ((slots+1).next_power_of_two().trailing_zeros() as usize)
        .clamp(3, std::mem::size_of::<usize>() * 8) - 1
}
#[inline(always)]
fn get_bucket_idx(checksum: usize, shift_amt:usize, mod_mask:usize) -> usize {
   (checksum >> shift_amt) ^ (checksum & mod_mask)
}




/// This is a chain list to store overflow values from the hash table.
/// It is an in-memory linked list structure.
/// This is sort of like an interleaved Vec of Vecs.
#[derive(Clone, Debug)]
pub(crate) struct ChainList {
    positions: Vec<BucketValue>, // Storage for previous positions
    mod_mask: usize,       // Mask for modulo operation. Buckets must be a power of 2
}

impl ChainList {
    /// Creates a new PrevList with the given capacity.
    /// Capacity must be a power of 2.
    /// Use 0 to short-circuit all functions and not allocate any memory.
    pub(crate) fn new(capacity: usize) -> Self {
        //assert that capacity is a power of 2
        if capacity == 0 {
            return Self {
                positions: Vec::new(),
                mod_mask: 0,
            };
        }
        assert_eq!(capacity & (capacity - 1), 0, "Prev Table Capacity ({}) is not a power of 2", capacity);
        Self {
            positions: vec![BucketValue::default(); capacity],
            mod_mask: capacity - 1,
        }
    }
    /// Gets the previous position for a given position.
    /// last_pos: The current position (BUCKET_VALUE_OFFSET is already subtracted)
    /// Returns the previous position with BUCKET_VALUE_OFFSET subtracted. (or none if bucket value == 0)
    #[inline(always)]
    fn get_prev_pos(&self, last_pos: usize) -> Option<&BucketValue> {
        if self.positions.is_empty() {
            return None;
        }
        let prev_idx_value = &self.positions[last_pos & self.mod_mask];
        if prev_idx_value.value == 0 {// End of chain or invalid position
            return None;
        }
        Some(prev_idx_value)
    }
    /// Inserts a new position into the chain.
    /// key: The parent position in the chain (new head) (BUCKET_VALUE_OFFSET is already subtracted)
    /// new_pos: The new position to insert
    #[inline(always)]
    pub(crate) fn insert(&mut self, hash:usize, key_position: usize, position: usize) {
        if self.positions.is_empty() {return;}
        let _ = self.positions.get_mut(key_position & self.mod_mask).unwrap().set(hash, position);
    }

    pub(crate) fn iter_prev_starts<'a>(&'a self, last_pos: usize, cur_out_pos: usize, hash_value:usize) -> PrevPositionIterator<'a>{
        PrevPositionIterator::new(&self, last_pos, cur_out_pos, hash_value)
    }
}
pub(crate) struct PrevPositionIterator<'a> {
    list: &'a ChainList,
    last_pos: usize,     // Base position for comparison
    cur_out_pos: usize,  // Current output position
    hash_value: usize,     // Mask for modulo operation
}

impl<'a> PrevPositionIterator<'a> {
    fn new(list: &'a ChainList, last_pos: usize,cur_out_pos:usize, hash_value:usize) -> Self {
        if list.positions.is_empty() {
            return Self {
                list,
                last_pos,
                cur_out_pos,
                hash_value: 0,
            };
        }
        Self {
            list,
            last_pos,
            cur_out_pos,
            hash_value,
        }
    }
}

impl<'a> Iterator for PrevPositionIterator<'a> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        loop{
            let populated_bucket = self.list.get_prev_pos(self.last_pos)?;
            // Check for termination conditions
            let prev_pos = populated_bucket.get_value_unchecked(); //safe since get_prev_pos checks
            if prev_pos > self.last_pos {
                return None; // End of chain or invalid position
            }
            //This is a valid-looking position, but it may not be part of the chain
            self.last_pos = prev_pos;

            let diff_pos = self.cur_out_pos - self.last_pos;
            if diff_pos  >= self.list.positions.len(){
                // Exceeded buffer capacity (wrap-around)
                // This position is technically valid looking, but logically cannot be part of this chain
                // This position has logically been evicted, but has not been overwritten in the buffer
                return None;
            }
            if populated_bucket.hash == self.hash_value {
                return Some(prev_pos);
            }

        }
    }
}