
use smdiff_common::{diff_addresses_to_i64, u_varint_encode_size, zigzag_encode, Copy, CopySrc, MAX_INST_SIZE};

use crate::{suffix::{SearchResult, SuffixArray}, MIN_MATCH_BYTES};



/*
Really the only thing to do here is create a scan function that will allow us to minimize the in between add instructions.

*/
///Returns (len,pos) of the match.
pub fn find_certain_match(
    target: &[u8],
    sa_src: &[u8],
    sa: &SuffixArray,
    cur_o_pos: usize,
    next_run_start:usize,
    max_end_pos:usize
) -> (u64,u16) {
    let check_len = *[MAX_INST_SIZE as usize, next_run_start - cur_o_pos,max_end_pos-cur_o_pos].iter().min().unwrap();
    let end_pos = cur_o_pos + check_len;
    let next_slice = &target[cur_o_pos..end_pos];
    unwrap_search_result(&sa.search(sa_src,next_slice), check_len)
}
pub struct NextMinMatch{
    pub next_o_pos: usize,
    pub src_found: bool,
    pub trgt_found: bool,
}
///Returns the number of bytes to advance to get another match or None if no matches found.
///This only tries to find the min_mactch_bytes, it doesn't try to find the best match.
pub fn scan_for_next_match(trgt_sa_src:&[u8], src_sa:&SuffixArray, trgt_sa: &SuffixArray,cur_pos:usize)->Option<NextMinMatch>{
    //We need to find the first min match bytes that from either trie.
    //we just use a rolling window and ask the trie if we have a match (Ok())
    //in theory this will just be calls to the btrees, so it shouldn't be too costly.
    let mut offset = 0;
    for window in trgt_sa_src[cur_pos..].windows(MIN_MATCH_BYTES){
        let cur_start = cur_pos + offset;
        let result = trgt_sa.search(trgt_sa_src,window);
        let trgt_found = result.is_some() && valid_target(unwrap_search_result(&result, MIN_MATCH_BYTES), cur_start as u64);
        let src_result = src_sa.search(trgt_sa_src,window);

        if trgt_found || src_result.is_some(){
            return Some(NextMinMatch{
                next_o_pos: cur_start,
                src_found: src_result.is_some(),
                trgt_found,
            });
        }
        offset += 1;
    }
    None

}
pub fn unwrap_search_result(result:&SearchResult,len:usize)->(u64,u16){
    result.unwrap()
    .map(|pos|(pos as u64,len as u16))
    .map_err(|(pos,len)| (pos as u64,len as u16))
    .unwrap()
}
pub fn valid_target(trgt_match: (u64,u16),cur_o_pos: u64) -> bool {
    (trgt_match.0 + trgt_match.1 as u64) < cur_o_pos
}
pub fn use_trgt_result(
    src: Option<(u64,u16)>,
    trgt: (u64,u16),
    last_d_addr: u64,
    last_o_addr: u64,
) -> bool {
    if let Some((d_addr,d_len)) = src {
        let (o_addr,o_len) = trgt;
        //the only difference would be the addr encoded size.
        let d_cost = calc_addr_cost(last_d_addr, d_addr) as isize * -1;
        let o_cost = calc_addr_cost(last_o_addr, o_addr) as isize * -1;
        (d_cost + d_len as isize) < (o_cost + o_len as isize)
    }else{true}
}
pub fn use_copy_op(last_addr: u64, next_addr:u64, len: u16) -> bool {
    if len > 6 {
        return true;
    }
    calc_addr_cost(last_addr, next_addr) + 1 <= len as u8
}

///Returns None if an Add instruction is better.
pub fn make_copy(start_len:(u64,u16),src:CopySrc,last_addr:&mut u64)->Option<Copy>{
    let (pos,len) = start_len;
    if !use_copy_op(*last_addr, pos, len){return None;}
    *last_addr = pos;
    Some(Copy{
        src,
        addr: pos,
        len,
    })
}

pub fn calc_addr_cost(last_addr: u64, next_addr: u64) -> u8 {
    let new_value = diff_addresses_to_i64(last_addr, next_addr);
    let to_u64 = zigzag_encode(new_value);
    u_varint_encode_size(to_u64) as u8
}
