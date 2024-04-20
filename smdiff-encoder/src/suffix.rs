


/*
I think we need to make variants of this for both Dict and Output.
Output has more requirements, as we need a second index to efficiently search for *valid* matches.
This is because we cannot emit a Copy for bytes that haven't been emitted yet.

Src is very simple in that manner.

Also: Store the indicies as u32 to reduce memory cost. (need to cast to usize).
We could technically use 3 bytes for trgt since max output size is 16MB.
Not sure if it is worth it?

I think I also want to make each of these a separate struct and eventually a macro for each.

I also need to do some benchmarking to see memory usage and cost per input byte.
*/


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
        if sa_src.len() < Self::MIN_MATCH_BYTES {
            return SuffixArray {
                suffixes: Vec::new(),
                prefix_map: BTreeMap::new(),
            };
        }

        let mut long_suffixes = Vec::new();
        let mut i = 0;

        for win in sa_src.windows(u16::MAX as usize){
            long_suffixes.push((i,win));
            i+=1;
        }
        //this is likely empty for small inputs so it isn't expensive.
        long_suffixes.sort_unstable_by(|a,b|b.1.cmp(a.1));

        let end = sa_src.len() - Self::MIN_MATCH_BYTES;
        let mut short_suffixes = Vec::new();
        while i <= end{
            //all of these might be prefixes of existing suffixes.
            //we need to only add them to the list *if these are not prefixes of existing suffixes*
            short_suffixes.push((i,&sa_src[i..]));
            i += 1;
        }
        //should test if sorting is better to have the shortest slices first or last
        //we could push the suffixes in reverse order
        short_suffixes.sort_unstable_by(|a,b|b.1.cmp(a.1));

        let mut prefix_map = BTreeMap::new();
        let mut suffixes  = Vec::new();
        //We want to work from largest lexical value to smallest, so we just pop them off and push them to suffixes.
        loop{
            let (start_pos,suffix) = match (long_suffixes.last(),short_suffixes.last()){
                (Some((_,long_suffix)),Some((_,short_suffix))) => {
                    let short_len = short_suffix.len();
                    let long_cmp = long_suffix[..short_len].cmp(short_suffix);
                    dbg!(&long_cmp,&short_suffix,&long_suffix);
                    if long_cmp == std::cmp::Ordering::Greater{
                        long_suffixes.pop().unwrap()
                    }else if long_cmp == std::cmp::Ordering::Less{
                        short_suffixes.pop().unwrap()
                    }else{
                        short_suffixes.pop(); //short suffix is a prefix of the longer suffix
                        long_suffixes.pop().unwrap()
                    }
                },
                (Some(_),None) => {
                    long_suffixes.pop().unwrap()
                },
                (None,Some(_)) => {
                    //simiar to how we compare the long suffixes, we compare the short suffixes.
                    //The popped suffix might be a superstring of last().
                    //if it is, we need to pop the last() and push the popped.
                    //then we continue the loop.
                    //alternatively we could put the loop below.
                    let (lesser_start,lesser_suffix) = short_suffixes.pop().unwrap();
                    match short_suffixes.last(){
                        Some((_,greater_suffix)) => {
                            //if lesser is >= greater.len(), we don't compare (must be a different prefix)
                            //if lesser is < greater.len(), we compare the prefix.
                            if lesser_suffix.len() < greater_suffix.len() && greater_suffix.starts_with(lesser_suffix){
                                continue; //leave the greater so it becomes the lesser next iteration.
                            }else{//leave greater, return lesser
                                (lesser_start,lesser_suffix)
                            }
                        },
                        None => (lesser_start,lesser_suffix),
                    }
                },
                (None,None) => {
                    break;
                }
            };
            let prefix = Self::bytes_to_array(&suffix[0..Self::MIN_MATCH_BYTES]);
            let index = suffixes.len();
            prefix_map.entry(prefix).or_insert(index);
            suffixes.push(start_pos);
        }
        SuffixArray {
            suffixes,
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
        if find_len < Self::MIN_MATCH_BYTES{
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
        let mut best_len = 0;
        let mut best_pos = 0;
        let max_len = sa_src.len();
        let tries = end - start;
        let mut attempt = 0;
        let mut left = start;
        let mut right = end;
        while attempt < tries{
            let index = left + (right - left) / 2;
            let i = self.suffixes[index];
            let test_len = std::cmp::min(find_len, max_len - i);
            let suffix_end_pos = i + test_len;
            let suffix = &sa_src[i..suffix_end_pos];
            //We assume most will not be exact matches, so we check equality using this zip method.
            let common_prefix_len = find.iter().zip(suffix).take_while(|(a, b)| a == b).count();
            //We want the suffix closes to the start of the file, but also the longest.
            //this is really for trgt file matches, since we cannot use a match that hasn't occured yet.
            if common_prefix_len >= best_len {
                best_len = common_prefix_len;
                best_pos = i;
            }
            if best_len == find_len {
                return Some(Ok(i)); // full match found
            }

            // Binary search adjustment based on prefix comparison
            if suffix < find && index < right {
                left = index + 1;
            } else {
                right = index;
            }
            attempt += 1;
        }
        if best_len >= Self::MIN_MATCH_BYTES {
            return Some(Err((best_pos,best_len)));
        }else{
            return None;
        }
    }

    ///Return Ok(start_pos) if the slice is found Or
    ///Err(Some((prefix_byte_len,start_pos_for_matching_prefix))) if the full slice is not found, but a prefix is
    /// Err(None) if the prefix is not found (match is less than the min_match_bytes)
    pub fn search_restricted(&self, sa_src: &[u8], find: &[u8],max_end_len:usize) -> SearchResult {
        let find_len = find.len();
        if find_len < Self::MIN_MATCH_BYTES || max_end_len < Self::MIN_MATCH_BYTES{
            return None;
        }
        let start_key = Self::bytes_to_array(&find[..Self::MIN_MATCH_BYTES]);
        let start = *self.prefix_map.get(&start_key)?;
        let end = self.prefix_map.range((Bound::Excluded(start_key),Bound::Unbounded)).next().map(|a|*a.1).unwrap_or(self.suffixes.len());

        let mut best_len = 0;
        let mut best_pos = 0;
        let max_len = sa_src.len();
        //because of the restriction we must try all the suffixes in the range.
        //the start positions are not sorted, so we must iterate through all of them.
        //long keys are at the end of ranges, but we have lots of suffixes within a range
        //not an easy solution, so we just iterate through all of them.
        //maybe we can improve on this at some point.
        //probably need a special suffix array for each file.
        //this restriction is for the trgt file, src doesn't have this.
        //something to work on.
        for &i in self.suffixes[start..end].iter().rev() {
            //dbg!(i,max_end_len,&sa_src[i..]);
            if i >= max_end_len {
                continue;
            }
            let test_len = *[find_len, max_len - i, max_end_len - i].iter().min().unwrap();
            let suffix_end_pos = i + test_len;
            let suffix = &sa_src[i..suffix_end_pos];
            //We assume most will not be exact matches, so we check equality using this zip method.
            let common_prefix_len = find.iter().zip(suffix).take_while(|(a, b)| a == b).count();
            //We want the suffix closes to the start of the file, but also the longest.
            //this is really for trgt file matches, since we cannot use a match that hasn't occured yet.

            // let valid_prefix_len = if i + common_prefix_len < max_end_len {
            //     common_prefix_len
            // } else{
            //     common_prefix_len - (suffix_end_pos - max_end_len)
            // };
            //dbg!(common_prefix_len,best_len,test_len,suffix,find,suffix_end_pos,max_end_len);
            if common_prefix_len >= best_len {
                best_len = common_prefix_len;
                best_pos = i;
            }
            if best_len == find_len {
                return Some(Ok(i)); // full match found
            }
        }
        dbg!(best_pos,best_len);
        if best_len >= Self::MIN_MATCH_BYTES {
            return Some(Err((best_pos,best_len)));
        }else{
            return None;
        }
    }
}

/// - Some(Ok(`start_pos`)) if the slice is found OR
/// - Some(Err( ( `prefix_byte_len`, `start_pos` ) )) if the full slice is not found, but a prefix >= MIN_MATCH_BYTES is
/// - None if the prefix is not found (match is less than the min_match_bytes)
pub type SearchResult = Option<Result<usize,(usize,usize)>>;


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
        assert_eq!(trie.search(src, "1233".as_bytes()).unwrap().unwrap_err(), (1, 3));
        assert_eq!(trie.search(src, "1235".as_bytes()).unwrap().unwrap_err(),(1, 3));
        assert_eq!(trie.search(src, "6789".as_bytes()), None);
    }
    #[test]
    fn test_minimal_suffixes() {
        let src = "01010101010".as_bytes();
        let trie = SuffixArray::new(src);
        //dbg!(&trie);
        assert_eq!(trie.search(src, "010".as_bytes()).unwrap().unwrap(), 0);
    }
    #[test]
    fn test_prefix_suffix_search() {
        let src = [1,2,3,4,5,6,1,2,3,4,5,6,6,2,3,4,5,7];
        let trie = SuffixArray::new(&src);
        //dbg!(&trie);

        assert_eq!(trie.search(&src, &[4,5,6,8]).unwrap().unwrap_err(), (9, 3));
        assert_eq!(trie.search_restricted(&src, &[4,5,6,8],9).unwrap().unwrap_err(), (3, 3));
        assert_eq!(trie.search(&src, &[4,5,7]).unwrap().unwrap(), 15);
        assert_eq!(trie.search(&src, &[4,5,4]).unwrap().unwrap_err(), (3, 2));

    }

}