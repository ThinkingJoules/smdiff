
use smdiff_common::{diff_addresses_to_i64, u_varint_encode_size, zigzag_encode};

use crate::{suffix::{SearchResult, SuffixArray}, MIN_MATCH_BYTES};



/*
Really the only thing to do here is create a scan function that will allow us to minimize the in between add instructions.

*/
///Returns the number of bytes to advance to get another match or None if no matches found.
pub fn scan_for_next_match(trgt_sa_src:&[u8], src_sa:&SuffixArray, trgt_sa: &SuffixArray,cur_pos:usize,use_src:bool)->Option<(bool,usize,SearchResult)>{
    //We need to find the first min match bytes that from either trie.
    //we just use a rolling window and ask the trie if we have a match (Ok())
    //in theory this will just be calls to the btrees, so it shouldn't be too costly.
    let mut offset = 0;
    for window in trgt_sa_src[cur_pos..].windows(MIN_MATCH_BYTES){
        let result = trgt_sa.search(trgt_sa_src,window);
        let src_result = src_sa.search(trgt_sa_src,window);

        if valid_target(&result, MIN_MATCH_BYTES, cur_pos+offset)
            || (use_src && src_sa.search(trgt_sa_src,window).is_ok())
        {
            return Some((true,cur_pos+offset,result));
        }
        offset += 1;
    }
    None

}

pub fn valid_target(trgt_match: &SearchResult, given_len: usize, cur_pos: usize) -> bool {
    match trgt_match {
        Ok(ok_value) => *ok_value + given_len < cur_pos,
        Err(Some((match_len, start_pos))) => match_len + start_pos < cur_pos,
        Err(None) => false, //short circuit so we don't compare it.
    }
}
pub fn use_trgt_result(
    src: &SearchResult,
    trgt: &SearchResult,
    last_d_addr: u64,
    cur_o_pos: u64,
) -> bool {
    match (src, trgt) {
        (Ok(d_addr), Ok(o_addr)) => {
            //the only difference would be the addr encoded size.
            let d_cost = calc_addr_cost(last_d_addr, *d_addr as u64);
            let o_cost = calc_addr_cost(cur_o_pos as u64, *o_addr as u64);
            o_cost <= d_cost
        },
        (Ok(_), _) => false,
        (_, Ok(_)) => true,
        (Err(Some((src_match_len, _))), Err(Some((trgt_match_len, _)))) => {
            trgt_match_len >= src_match_len
        }
        (Err(Some(_)), _) => false,
        (_, Err(Some(_))) => true,
        (Err(None), Err(None)) => false,
    }
}
pub fn use_copy_op(last_addr: u64, next_addr:u64, len: usize) -> bool {
    if len > 6 {
        return true;
    }
    calc_addr_cost(last_addr, next_addr) + 1 <= len as u8
}


pub fn optimistic_add(last_d_addr: u64, cur_o_pos: usize, max_len: usize) -> bool {
    //6 because a 6 byte addr varint would be > u32::MAX file.
    //Adds can only beat a copy if the bytes to add are very few.
    if max_len > 6{
        return false;
    }
    let worst_case_d_copy = u_varint_encode_size(last_d_addr);
    let worst_case_o_copy = u_varint_encode_size(cur_o_pos as u64);
    let addr_cost = worst_case_d_copy.max(worst_case_o_copy);
    addr_cost + 1 <= max_len
}

pub fn calc_addr_cost(last_addr: u64, next_addr: u64) -> u8 {
    let new_value = diff_addresses_to_i64(last_addr, next_addr);
    let to_u64 = zigzag_encode(new_value);
    u_varint_encode_size(to_u64) as u8
}
