use std::ops::Range;

use crate::{src_matcher::SrcMatcherConfig, trgt_matcher::TrgtMatcherConfig};


#[derive(Copy,Clone,Debug)]
pub enum LargerTrgtNaiveTests{
    Prepend,
    Append,
    PrependAppend,
    AppendPrepend,
}
impl Default for LargerTrgtNaiveTests {
    fn default() -> Self {
        LargerTrgtNaiveTests::AppendPrepend
    }
}
const MAX_SRC_WIN_SIZE:usize = 1<<26;
#[derive(Debug)]
pub(crate) enum InnerSegment{
    NoMatch{length:usize},
    MatchSrc{start:usize,length:usize},
    MatchTrgt{start:usize,length:usize}
}
//we do not do equality check on src and trgt, that is the job of the caller.
pub(crate) fn encode_inner(config:&EncoderConfig,src:&[u8],trgt:&[u8])->Vec<InnerSegment>{
    let EncoderConfig { match_trgt, match_src, naive_tests } = config;
    let trgt_len = trgt.len();
    if (match_src.is_none() && match_trgt.is_none())
        || (src.len() == 0 && match_trgt.is_none())
        || trgt_len == 0 {
        return vec![InnerSegment::NoMatch{length:trgt_len}];
    }
    //first try the naive tests.
    //these are here to avoid the overhead of the matcher.
    //they will likely fail but if they don't we can save a lot of time.
    //ultimately only the user can decide if they are worth it.
    let src_len = src.len();
    let mut matches = Vec::new();
    let mut max_trgt_matcher_len = trgt_len;
    let mut cur_o_pos = 0;
    if trgt_len > src_len && naive_tests.is_some(){
        //try tests
        let naive_tests = naive_tests.as_ref().unwrap();
        let handle_start_match = |segs: &mut Vec<InnerSegment>, cur_o: &mut usize| {
            if trgt.starts_with(src) {
                segs.push(InnerSegment::MatchSrc {start: 0,length: src.len(),});
                *cur_o = src.len();
                true
            } else {false}
        };

        let handle_end_match = |max_t_len: &mut usize| {
            if trgt.ends_with(src) {*max_t_len = src.len();true} else {false}
        };

        match naive_tests {
            LargerTrgtNaiveTests::Append => {handle_start_match(&mut matches, &mut cur_o_pos);},
            LargerTrgtNaiveTests::Prepend => {handle_end_match(&mut max_trgt_matcher_len);},
            LargerTrgtNaiveTests::AppendPrepend => {
                if !handle_start_match(&mut matches, &mut cur_o_pos){handle_end_match(&mut max_trgt_matcher_len);}
            }
            LargerTrgtNaiveTests::PrependAppend => {
                if !handle_end_match(&mut max_trgt_matcher_len){handle_start_match(&mut matches, &mut cur_o_pos);}
            }
        }
    }

    //now we decide our matcher configs.
    //Since we may have matched part of the trgt from naive we need to consider that in our parameters.
    let trgt_matchable_len = max_trgt_matcher_len - cur_o_pos;



    let src_win_size = src_len.min(MAX_SRC_WIN_SIZE);
    let roll_src_hash_at_trgt_pos = if trgt_matchable_len > src_len {cur_o_pos.max(src_win_size / 2)}else{trgt_len};
    let start = cur_o_pos.saturating_sub(src_win_size / 2);
    let end = (start + src_win_size).min(src_len);


    todo!()

}

struct EncoderState{
    out_pos:usize,
    min_match:usize,
}

fn src_hash_len(len:usize)->usize{
    if len <= 127{
        3
    }else if len <= 16_383{
        4
    }else if len <= 2_097_151{
        5
    }else if len <= 6_998_841{
        6
    }else if len <= 23_541_202{
        7
    }else if len <= 79_182_851{
        8
    }else{
        9
    }
}



#[derive(Debug, Clone)]
pub struct EncoderConfig{
    pub match_trgt: Option<TrgtMatcherConfig>,
    pub match_src: Option<SrcMatcherConfig>,
    pub naive_tests: Option<LargerTrgtNaiveTests>,

}

impl EncoderConfig {
    pub fn new(match_trgt: Option<TrgtMatcherConfig>, match_src: Option<SrcMatcherConfig>, naive_tests:Option<LargerTrgtNaiveTests>) -> Self {
        Self { match_trgt, match_src,naive_tests }
    }
}

impl Default for EncoderConfig {
    fn default() -> Self {
        Self { match_trgt: None, match_src: Some(SrcMatcherConfig::new_from_compression_level(3)),naive_tests: Some(LargerTrgtNaiveTests::Append)}
    }
}