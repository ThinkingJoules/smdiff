

const BUCKET_VALUE_OFFSET: usize = 1;

#[derive(Clone, Debug)]
struct PrevList {
    positions: Vec<usize>, // Storage for previous positions
    mod_mask: usize,       // Mask for modulo operation
}

impl PrevList {
    /// Creates a new PrevList with the given capacity.
    /// Capacity must be a power of 2.
    fn new(capacity: usize) -> Self {
        //assert that capacity is a power of 2
        assert_eq!(capacity & (capacity - 1), 0);
        Self {
            positions: vec![0; capacity],
            mod_mask: capacity - 1,
        }
    }
    /// Gets the previous position for a given position.
    /// last_pos: The current position (BUCKET_VALUE_OFFSET is already subtracted)
    /// Returns the previous position with BUCKET_VALUE_OFFSET subtracted. (or none if bucket value == 0)
    fn get_prev_pos(&self, last_pos: usize) -> Option<usize> {
        let prev_idx_value = self.positions[last_pos & self.mod_mask];
        if prev_idx_value == 0 {// End of chain or invalid position
            return None;
        }
        Some(prev_idx_value - BUCKET_VALUE_OFFSET)
    }
    /// Inserts a new position into the chain.
    /// key: The parent position in the chain (new head) (BUCKET_VALUE_OFFSET is already subtracted)
    /// new_pos: The new position to insert
    fn insert(&mut self, key_position: usize, position: usize) {
        *self.positions.get_mut(key_position & self.mod_mask).unwrap() = position + BUCKET_VALUE_OFFSET;
    }
}
struct PrevPositionIterator<'a> {
    list: &'a PrevList,
    last_pos: usize,     // Base position for comparison
    cur_out_pos: usize,  // Current output position
    mod_mask: usize,     // Mask for modulo operation
}

impl<'a> PrevPositionIterator<'a> {
    fn new(list: &'a PrevList, last_pos: usize,cur_out_pos:usize) -> Self {
        Self {
            list,
            last_pos,
            cur_out_pos,
            mod_mask: list.positions.len() - 1,
        }
    }
}

impl<'a> Iterator for PrevPositionIterator<'a> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        let prev_pos = self.list.get_prev_pos(self.last_pos)?;
        // Check for termination conditions
        if prev_pos > self.last_pos {
            return None; // End of chain or invalid position
        }
        //This is a valid-looking position, but it may not be part of the chain
        self.last_pos = prev_pos;

        let diff_pos = self.cur_out_pos - self.last_pos;
        if diff_pos & !self.mod_mask != 0{ //should be the same as diff_pos >= self.list.positions.len()
            // Exceeded buffer capacity (wrap-around)
            // This position is technically valid looking, but logically cannot be part of this chain
            // This position has logically been evicted, but has not been overwritten in the buffer
            return None;
        }
        Some(self.last_pos)
    }
}
// Trait to allow for different hash table implementations.
pub trait HashTable {
    /// Calculates the bucket index for a given checksum or data slice.
    fn calc_index(&self, checksum: usize) -> usize;
    /// Gets the most recent position found for a given index.
    fn get_last_pos(&self,index:usize)->Option<usize>;
    /// Iterates the chained values for a given index.
    /// Iterator does *not* include the last_pos given as it is needed to generate the iterator.
    /// If you want to include it do something like: `std::iter::once(last_pos).chain(table.iter_chain_idx(last_pos))`
    fn iter_prev_starts<'a>(&'a self, last_pos: usize, cur_out_pos:usize) -> PrevPositionIterator<'a>;

    /// Inserts a position into the chain at the given index.
    fn insert(&mut self, idx: usize, start_pos: usize);
}
pub(crate) struct BasicHashTable {
    table_size: usize,       // Number of slots in the hash table
    shift_amount: usize,    // Bit shift amount for hash reduction
    mod_mask: usize,          // Bitmask for hash table indexing
    buckets: Vec<usize>,
    prev_list: PrevList,
}
impl BasicHashTable{
    pub(crate) fn new(num_slots: usize, prev_start_capacity: usize) -> Self {
        let num_bits = determine_hash_table_size_bits(num_slots);
        let table_size = 1 << num_bits;
        Self {
            mod_mask: table_size - 1,
            table_size,
            shift_amount: (std::mem::size_of::<usize>() * 8) - num_bits,
            buckets: vec![0; table_size],
            prev_list: PrevList::new(prev_start_capacity),
        }
    }
}
impl HashTable for BasicHashTable {
    fn calc_index(&self, checksum: usize) -> usize {
        get_bucket_idx(checksum, self.shift_amount, self.mod_mask, self.table_size)
    }
    fn get_last_pos(&self,idx:usize)->Option<usize>{
        let last_pos = self.buckets[idx];
        if last_pos == 0{
            return None;
        }
        Some(last_pos - BUCKET_VALUE_OFFSET)
    }

    fn insert(&mut self, idx: usize, position: usize) {
        let idx_value = self.buckets[idx];
        if !self.prev_list.positions.is_empty() && idx_value != 0{
            //move the old value into the prev list
            self.prev_list.insert(position, idx_value - BUCKET_VALUE_OFFSET);
        }
        //set the new value in the table
        self.buckets[idx] = position + BUCKET_VALUE_OFFSET;
    }

    fn iter_prev_starts<'a>(&'a self, last_pos: usize, cur_out_pos: usize) -> PrevPositionIterator<'a>{
        PrevPositionIterator::new(&self.prev_list, last_pos, cur_out_pos)
    }
}

fn determine_hash_table_size_bits(slots: usize) -> usize {
    let mut i = 3;
    while i <= std::mem::size_of::<usize>() * 8 && slots >= (1 << i) {
        i += 1;
    }
    i - 1 // Subtract 1 to get the correct number of bits
}
fn get_bucket_idx(checksum: usize, shift_amt:usize, mod_mask:usize,num_buckets:usize) -> usize {
   (checksum >> shift_amt).wrapping_pow((checksum & mod_mask) as u32) % num_buckets
}