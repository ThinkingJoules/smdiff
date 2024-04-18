





use std::{collections::BTreeMap, ops::Bound};

use crate::MIN_MATCH_BYTES;
#[derive(Debug)]
pub struct SuffixArray {
    suffixes: Vec<usize>,
    prefix_map: BTreeMap<[u8;Self::MIN_MATCH_BYTES], usize>,
}

impl SuffixArray {
    pub const MIN_MATCH_BYTES: usize = MIN_MATCH_BYTES;

    pub fn new(sa_src: &[u8]) -> Self {
        //we only keep the longest match so that all shorter copies come from the same position.

        let mut suffixes = Vec::new();
        let mut i = 0;

        for win in sa_src.windows(u16::MAX as usize){
            suffixes.push((i,win));
            i+=1;
        }
        let end = sa_src.len() - Self::MIN_MATCH_BYTES;
        while i <= end{
            suffixes.push((i,&sa_src[i..]));
            i += 1;
        }
        let mut prefix_map = BTreeMap::new();
        for (index, suffix) in suffixes.iter() {
            let prefix = Self::bytes_to_array(&suffix[0..Self::MIN_MATCH_BYTES]);
            prefix_map.entry(prefix).or_insert(*index);
        }
        SuffixArray {
            suffixes: suffixes.into_iter().map(|(index, _)| index).collect(),
            prefix_map,
        }
    }
    fn bytes_to_array(bytes: &[u8]) -> [u8; Self::MIN_MATCH_BYTES] {
        let mut array = [0u8; Self::MIN_MATCH_BYTES];
        array.copy_from_slice(bytes);
        array
    }


    ///Return Ok(start_pos) if the slice is found Or
    ///Err(Some((prefix_byte_len,start_pos_for_matching_prefix))) if the full slice is not found, but a prefix is
    /// Err(None) if the prefix is not found (match is less than the min_match_bytes)
    pub fn search(&self, sa_src: &[u8], find: &[u8]) -> SearchResult {
        let find_len = find.len();
        if find_len < Self::MIN_MATCH_BYTES {
            return None;
        }

        let start_key = Self::bytes_to_array(&find[..Self::MIN_MATCH_BYTES]);
        //let end_key = Self::increment_slice(&start_key);

        let start = *self.prefix_map.get(&start_key)?;
        if find_len == Self::MIN_MATCH_BYTES {
            //Early return if we are only looking for the prefix.

            return Some(Ok(start));
        }


        let end = self.prefix_map.range((Bound::Excluded(start_key),Bound::Unbounded)).next().map(|a|*a.1).unwrap_or(self.suffixes.len());
        //ideal for Src match addr encoding (relative addr)
        //worst case for trgt match addr encoding (addr will steadily grow.)

        //we start with the largest lexical value to make sure we match the longest possible amount of find.
        //Optimization would be to store more info on the lengths of the prefixes, or allow the btree to store unequal len keys.
        let max_len = sa_src.len() - find_len;
        for &i in self.suffixes[start..end].iter().rev() {
            let suffix = &sa_src[i..std::cmp::min(i + find_len, max_len)];
            if suffix > find {
                continue;
            }
            //We assume most will not be exact matches, so we check equality using this zip method.
            let common_prefix_len = find.iter().zip(suffix).take_while(|(a, b)| a == b).count();
            if common_prefix_len == find_len {
                return Some(Ok(i)); // full match found
            }
            else {
                return Some(Err((i,common_prefix_len)));
            }
            //can we early return?
            // if common_prefix_len > best_match_len {
            //     best_match_len = common_prefix_len;
            //     best_match_pos = i;
            // }
        }
        unreachable!()
    }
}

/// - Ok(`start_pos`) if the slice is found OR
/// - Err(Some( ( `prefix_byte_len`, `start_pos` ) )) if the full slice is not found, but a prefix is
/// - Err(None) if the prefix is not found (match is less than the min_match_bytes)
pub type _SearchResult = Result<usize, Option<(usize, usize)>>;

/// - Some(Ok(`start_pos`)) if the slice is found OR
/// - Some(Err( ( `prefix_byte_len`, `start_pos` ) )) if the full slice is not found, but a prefix >= MIN_MATCH_BYTES is
/// - None if the prefix is not found (match is less than the min_match_bytes)
pub type SearchResult = Option<SearchFound>;
/// - Ok(`start_pos`) if the slice is found OR
/// - Err( ( `start_pos`, `prefix_byte_len` ) ) if the full slice is not found, but a prefix >= MIN_MATCH_BYTES is
pub type SearchFound = Result<usize,(usize,usize)>;
// fn increment_slice(bytes: &[u8]) -> Option<[u8; Self::MIN_MATCH_BYTES]> {
//     let mut result = Self::bytes_to_array(bytes);
//     let mut i = Self::MIN_MATCH_BYTES;
//     while i > 0 {
//         i -= 1;
//         if result[i] == 0xFF {
//             result[i] = 0;
//         } else {
//             result[i] += 1;
//             return Some(result);
//         }
//     }
//     None
// }

#[cfg(test)]
mod test_super {
    use super::*;
    #[test]
    fn test_insert_min() {
        let _trie = SuffixArray::new(&[0b1011_0100, 0b0011_1011]);
        // dbg!(_trie);
    }
    #[test]
    fn test_insert_and_search_min() {
        let src = &[0b1011_0100, 0b0011_1011];
        let trie = SuffixArray::new(src);
        assert_eq!(trie.search(src,src).unwrap().unwrap(),0);

        assert!(trie.search(src,"01".as_bytes()).is_none()); // Shouldn't exist
    }

    #[test]
    fn test_multiple_prefixes() {
        let src = "01234012".as_bytes();
        let trie = SuffixArray::new(src);
        //dbg!(&trie);
        assert_eq!(trie.search(src, "01234".as_bytes()).unwrap().unwrap(), 0);
        assert_eq!(trie.search(src, "0123".as_bytes()).unwrap().unwrap(), 0);
        assert_eq!(trie.search(src, "1234".as_bytes()).unwrap().unwrap(), 1);
        assert_eq!(trie.search(src, "012".as_bytes()).unwrap().unwrap(), 0);
        assert_eq!(trie.search(src, "234".as_bytes()).unwrap().unwrap(), 2);
        assert_eq!(trie.search(src, "23".as_bytes()).unwrap().unwrap(), 2);
        assert_eq!(trie.search(src, "1233".as_bytes()).unwrap().unwrap_err(), (3, 1));
        assert_eq!(trie.search(src, "1235".as_bytes()).unwrap().unwrap_err(),(3, 1));
        assert_eq!(trie.search(src, "6789".as_bytes()), None);
    }
    #[test]
    fn test_minimal_suffixes() {
        let src = "01010101010".as_bytes();
        let trie = SuffixArray::new(src);
        //dbg!(&trie);
        assert_eq!(trie.search(src, "010".as_bytes()).unwrap().unwrap(), 0);
    }
    // #[test]
    // fn test_skip_ranges() {
    //     let src = "01234012".as_bytes();
    //     let trie = SuffixArray::new(src,Some(vec![1..3]));
    //     //dbg!(&trie);

    //     //This is the only non-logical result
    //     //We must capture at least MIN_MATCH_BYTES or we miss a *prefix*.
    //     //So we end up capturing the skip section, but *only in a suffix*.
    //     assert_eq!(trie.search(src, "01234".as_bytes()).unwrap(), 0);

    //     assert_eq!(trie.search(src, "0123".as_bytes()).unwrap_err(), Some((3, 5)));
    //     assert_eq!(trie.search(src, "1234".as_bytes()).unwrap_err(), Some((2, 6)));
    //     assert_eq!(trie.search(src, "012".as_bytes()).unwrap(), 5);
    //     assert_eq!(trie.search(src, "234".as_bytes()).unwrap_err(), None);
    //     assert_eq!(trie.search(src, "23".as_bytes()).unwrap_err(), None);
    //     assert_eq!(trie.search(src, "340".as_bytes()).unwrap(), 3);
    //     assert_eq!(trie.search(src, "6789".as_bytes()).unwrap_err(), None);
    // }

}