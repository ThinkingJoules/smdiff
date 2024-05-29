

#[derive(Debug, Clone)]
pub struct SrcMatcherConfig{
    ///How much to advance the Large Hash between storing a src hash.
    pub l_step: usize,
    /// If the current match is less than lazy_escape_len it steps byte by byte looking for more matches.
    pub lazy_escape_len: usize,
    /// Max number of entries to check in the chain during matching.
    /// Larger value means more accurate matches but slower.
    pub chain_check: usize,
    ///Advanced setting, leave as None for default.
    pub table_capacity: Option<usize>,
}
impl SrcMatcherConfig {
    ///Creates a new SrcMatcherConfig with the given parameters.
    /// l_step: How much to advance the Large Hash between storing a src hash.
    /// lazy_escape_len: If the current match is less than lazy_escape_len it steps byte by byte looking for more matches.
    /// l_table: Advanced settings, leave as None for default. See TableConfig for more information.
    pub fn new(l_step: usize, lazy_escape_len: usize, chain_check:usize, table_capacity:Option<usize>) -> Self {
        Self { l_step, lazy_escape_len, chain_check, table_capacity}
    }
    ///Creates a new SrcMatcherConfig with the given compression level.
    /// level: The compression level to use. Must be between 0 and 9.
    /// The higher the level the more accurate the matches but slower.
    pub fn new_from_compression_level(level:usize)->Self{
        assert!(level <= 9);
        let l_step = 3 * level % 25;
        let lazy_escape_len = 6 + (level*81 / 9);
        let chain_check = 1 + level;
        Self { l_step, lazy_escape_len, chain_check, table_capacity: None }
    }
    pub fn with_table_capacity(mut self, table_capacity:usize)->Self{
        self.table_capacity = Some(table_capacity);
        self
    }
}