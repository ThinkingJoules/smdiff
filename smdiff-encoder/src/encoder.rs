/*
This aims to be a very general purpose encoder.
This just find a bunch of runs and copies.
They may overlap or have gaps.
The caller is responsible for deciding how to handle that for their situation.
For example, favoring one copy over another due to address cost optimizations.
Thus, this encoder gives all the options.
It attempts to return the longest possible instructions for any given range of bytes.

The only Ops not returned, are those that are completely contained within another op.

There are several ways to configure this encoder, so it should allow for tuning for any particular situation.
(or just for trying to find good defaults).



*/
use crate::{hasher::*, src_matcher::{add_start_positions_to_matcher, SrcMatcherConfig}, trgt_matcher::TrgtMatcherConfig};

#[allow(unused)]
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
#[derive(Debug,Clone)]
pub(crate) enum InnerOp{
    MatchSrc{start:usize,length:usize,o_pos:usize},
    MatchTrgt{start:usize,length:usize,o_pos:usize},
    Run{byte:u8,length:usize,o_pos:usize},
}

impl InnerOp {
    pub(crate) fn o_pos(&self)->&usize{
        match self {
            InnerOp::MatchSrc{o_pos,..} => o_pos,
            InnerOp::MatchTrgt{o_pos,..} => o_pos,
            InnerOp::Run{o_pos,..} => o_pos,
        }
    }
    pub(crate) fn len(&self)->&usize{
        match self {
            InnerOp::MatchSrc{length, ..} => length,
            InnerOp::MatchTrgt{length,..} => length,
            InnerOp::Run{length,..} => length,
        }
    }
    pub(crate) fn set_len(&mut self,new_len:usize){
        assert!(new_len <= *self.len(), "Cannot increase length of InnerOp");
        match self {
            InnerOp::MatchSrc{length,..} => *length = new_len,
            InnerOp::MatchTrgt{length,..} => *length = new_len,
            InnerOp::Run{length,..} => *length = new_len,
        }
    }
    /// This fn also adjusts length to maintain end position.
    pub(crate) fn set_o_pos(&mut self,new_pos:usize){
        let cur_pos = *self.o_pos();
        let cur_end = cur_pos + *self.len();
        assert!(new_pos >= cur_pos, "Cannot decrease position of InnerOp");
        let diff = new_pos - cur_pos;
        match self {
            InnerOp::MatchSrc { start, length, o_pos } => {
                *start += diff;
                *o_pos = new_pos;
                *length -= diff;
            },
            InnerOp::MatchTrgt { start, length, o_pos } => {
                *start += diff;
                *o_pos = new_pos;
                *length -= diff;
            },
            InnerOp::Run { length, o_pos, .. } => {
                *o_pos = new_pos;
                *length -= diff;
            },
        }
        assert_eq!(self.o_pos() + self.len(), cur_end, "End position of InnerOp is not maintained");
    }
    // pub(crate) fn is_src(&self)->bool{
    //     match self {
    //         InnerOp::MatchSrc{..} => true,
    //         InnerOp::MatchTrgt{..} => false,
    //         InnerOp::Run{..} => false,
    //     }
    // }
    // pub(crate) fn is_trgt(&self)->bool{
    //     match self {
    //         InnerOp::MatchSrc{..} => false,
    //         InnerOp::MatchTrgt{..} => true,
    //         InnerOp::Run{..} => false,
    //     }
    // }
    // pub(crate) fn is_run(&self)->bool{
    //     match self {
    //         InnerOp::MatchSrc{..} => false,
    //         InnerOp::MatchTrgt{..} => false,
    //         InnerOp::Run{..} => true,
    //     }
    // }
}

//we do not do equality check on src and trgt, that is the job of the caller.
/// Returns all possible operations to encode src into trgt.
/// This means ops may overlap or not.
/// It is up to the caller to decide how to handle the gaps and overlaps
pub(crate) fn encode_inner(config:&mut GenericEncoderConfig,src:&[u8],trgt:&[u8])->Vec<InnerOp>{
    let naive_tests = config.naive_tests;

    let trgt_len = trgt.len();
    if (config.match_src.is_none() && config.match_trgt.is_none())
        || (src.len() == 0 && config.match_trgt.is_none())
        || trgt_len == 0 {
        return vec![];
    }
    //first try the naive tests.
    //these are here to avoid the overhead of the matcher.
    //they will likely fail but if they don't we can save a lot of time.
    //ultimately only the user can decide if they are worth it.
    let src_len = src.len();
    let mut ops = Vec::new();
    let mut max_trgt_match_len = trgt_len;
    let mut cur_o_pos = 0;
    if trgt_len > src_len && naive_tests.is_some(){ //do naive tests if indicated and valid
        //try tests
        let naive_tests = naive_tests.unwrap();
        let handle_start_match = |segs: &mut Vec<InnerOp>, cur_o: &mut usize| {
            if trgt.starts_with(src) {
                segs.push(InnerOp::MatchSrc {start: 0,length: src.len(),o_pos: 0});
                *cur_o = src.len();
                true
            } else {false}
        };

        let handle_end_match = |max_t_len: &mut usize| {
            if trgt.ends_with(src) {*max_t_len = trgt_len - src_len;true} else {false}
        };

        match naive_tests {
            LargerTrgtNaiveTests::Append => {handle_start_match(&mut ops, &mut cur_o_pos);},
            LargerTrgtNaiveTests::Prepend => {handle_end_match(&mut max_trgt_match_len);},
            LargerTrgtNaiveTests::AppendPrepend => {
                if !handle_start_match(&mut ops, &mut cur_o_pos){handle_end_match(&mut max_trgt_match_len);}
            }
            LargerTrgtNaiveTests::PrependAppend => {
                if !handle_end_match(&mut max_trgt_match_len){handle_start_match(&mut ops, &mut cur_o_pos);}
            }
        }
    }


    //now we decide our matcher configs, at least one of these will be Some.
    let _start = std::time::Instant::now();
    let mut trgt_matcher = config.match_trgt.as_mut().map(|c| c.build(trgt, cur_o_pos));
    let mut src_matcher = config.match_src.as_mut().map(|c| c.build(src, cur_o_pos, trgt));
    // let _elapsed = _start.elapsed();
    // dbg!(_elapsed);
    let lazy_escape_len = config.lazy_escape_len.unwrap_or(90);

    let min_match_value = 4;
    let mut min_match= min_match_value;
    let mut run_len = 0;
    let mut run_byte = 0;
    let mut state = EncoderState::StartNewMatch;
    //now we start the main loop.
    let _start = std::time::Instant::now();
    loop {
        //first we see if we are out of data.
        if cur_o_pos + min_match_value >= max_trgt_match_len {
            break;
        }
        match state{
            EncoderState::StartNewMatch => {
                //we setup for trying to start a new match
                run_len = find_initial_run_len(&trgt[cur_o_pos..], min_match_value, &mut run_byte);

                if let Some(matcher) = src_matcher.as_mut(){
                    if matcher.next_hash_pos <= cur_o_pos{
                        add_start_positions_to_matcher(matcher, cur_o_pos, src)
                    }
                    if matcher.fwd_pos < matcher.max_fwd_hash_pos{
                        if matcher.fwd_pos + 9 > cur_o_pos{
                            for old_pos in matcher.fwd_pos..cur_o_pos{
                                matcher.fwd_hash = update_large_checksum_fwd(matcher.fwd_hash, trgt[old_pos], trgt[old_pos+9]);
                            }
                        }else{
                            matcher.fwd_hash = calculate_large_checksum(&trgt[cur_o_pos..cur_o_pos+9]);
                        }
                        matcher.fwd_pos = cur_o_pos;
                    }
                };
                if let Some(matcher) = trgt_matcher.as_mut(){
                    if matcher.fwd_pos < matcher.max_fwd_hash_pos{
                        if matcher.fwd_pos + 4 > cur_o_pos{
                            for old_pos in matcher.fwd_pos..cur_o_pos{
                                matcher.fwd_hash = update_small_checksum_fwd(matcher.fwd_hash, trgt[old_pos], trgt[old_pos+4]);
                            }
                        }else{
                            matcher.fwd_hash = calculate_small_checksum(&trgt[cur_o_pos..cur_o_pos+4]);
                        }
                        matcher.fwd_pos = cur_o_pos;
                    }
                }
                //adjust our min_match so lazy matching works.
                let last_match_end_pos = ops.last().map(|x|x.o_pos()+x.len()).unwrap_or(0);
                min_match = if last_match_end_pos > cur_o_pos {
                    min_match_value.max(1 + last_match_end_pos - cur_o_pos)
                } else {min_match_value};
                state = EncoderState::TryMatch;
            },
            EncoderState::TryMatch => {
                // if cur_o_pos + 3 >= max_trgt_match_len {
                //     break;
                // }
                // debug_assert!(small_hasher.as_ref().map(|x|x.peek_next_pos()==cur_o_pos).unwrap_or(true),"sh.pos {} != cur o {}",small_hasher.as_ref().map(|x|x.peek_next_pos()).unwrap(),cur_o_pos);
                // debug_assert!(large_hasher.as_ref().map(|x|x.peek_next_pos()==cur_o_pos).unwrap_or(true),"lh.pos {} != cur_o {}",large_hasher.as_ref().map(|x|x.peek_next_pos()).unwrap(),cur_o_pos);
                let remaining_trgt = &trgt[cur_o_pos..];
                if run_len == min_match_value{
                    remaining_trgt[min_match_value..].iter().take_while(|&x| x == &run_byte).for_each(|_|run_len += 1);
                    if run_len >= min_match{
                        let run_start = cur_o_pos;// - min_match_value;
                        debug_assert!(trgt[run_start..].iter().take_while(|x|*x == &run_byte).count() == run_len,"{:?}",&trgt[run_start-min_match_value..run_start+run_len]);
                        ops.push(InnerOp::Run{byte:run_byte as u8,length:run_len,o_pos:run_start});
                        state = EncoderState::FoundMatch { match_len: run_len };
                        continue;
                    }
                }

                if let Some(matcher) = src_matcher.as_mut(){
                    if cur_o_pos + 9 <= max_trgt_match_len{
                        debug_assert!(matcher.fwd_pos == cur_o_pos, "lh.pos {} != cur o {}",matcher.fwd_pos,cur_o_pos);
                        if let Some((src_start,pre_match,post_match)) = matcher.find_best_src_match(src, trgt) {
                            let length = pre_match + post_match;
                            if post_match >= min_match{
                                let trgt_match_start = cur_o_pos - pre_match;
                                if pre_match > 0{
                                    //remove all ops that are fully before this start position
                                    clear_existing_ops(&mut ops, trgt_match_start)
                                }
                                let src_match_start = src_start - pre_match;
                                //assert!(cur_o_pos + length > ops.last().map(|x|x.o_pos()+x.len()).unwrap_or(0));
                                debug_assert!(src_match_start + length <= src_len);
                                debug_assert!(src[src_match_start..src_match_start+length] == trgt[trgt_match_start..trgt_match_start+length]);
                                ops.push(InnerOp::MatchSrc{start:src_match_start, length, o_pos:trgt_match_start});
                                state = EncoderState::FoundMatch { match_len: post_match };
                                continue;
                            }
                        }
                    }
                }

                if let Some(matcher) = trgt_matcher.as_mut(){
                    if cur_o_pos + 4 <= max_trgt_match_len{
                        debug_assert!(matcher.fwd_pos == cur_o_pos, "sh.pos {} != cur o {}",matcher.fwd_pos,cur_o_pos);
                        if let Some((match_start,length)) = matcher.find_best_trgt_match( trgt) {
                            if length >= min_match{
                                debug_assert!(match_start + length <= cur_o_pos);
                                debug_assert!(trgt[match_start..match_start+length] == trgt[cur_o_pos..cur_o_pos+length]);
                                ops.push(InnerOp::MatchTrgt{start:match_start, length, o_pos:cur_o_pos});
                                state = EncoderState::FoundMatch { match_len: length };
                                continue;
                            }
                        }
                    }
                }

                //no matches >= min_match, so we move forward one byte.
                if min_match > min_match_value{
                    //we only need to exceed our min match by one less to match new bytes
                    //since we are moving forward.
                    min_match -= 1;
                }
                state = EncoderState::MoveForwardOneByte;
            },
            EncoderState::FoundMatch{ match_len } => {
                if lazy_escape_len > 0
                    && match_len < lazy_escape_len
                    && cur_o_pos + match_len < max_trgt_match_len - 1
                {
                    min_match = match_len;
                    state = EncoderState::MoveForwardOneByte;
                }else{
                    cur_o_pos += match_len;
                    state = EncoderState::StartNewMatch;
                }
            },
            EncoderState::MoveForwardOneByte => {
                //increment our encoder forward one byte
                let next_char = trgt[cur_o_pos + min_match_value];
                if run_byte == next_char{
                    run_len += 1;
                }else{
                    run_len = 1;
                    run_byte = next_char;
                }
                if let Some(matcher) = trgt_matcher.as_mut() {
                    matcher.store(matcher.fwd_hash as usize, cur_o_pos);
                    if matcher.fwd_pos < matcher.max_fwd_hash_pos{
                        matcher.fwd_hash = update_small_checksum_fwd(matcher.fwd_hash, trgt[cur_o_pos], trgt[cur_o_pos+4]);
                        matcher.fwd_pos = cur_o_pos+1;
                    }
                }
                if let Some(m) = src_matcher.as_mut(){
                    if m.fwd_pos < m.max_fwd_hash_pos{
                        m.fwd_hash = update_large_checksum_fwd(m.fwd_hash, trgt[cur_o_pos], trgt[cur_o_pos+9]);
                        m.fwd_pos = cur_o_pos+1;
                    }
                }
                cur_o_pos += 1;
                state = EncoderState::TryMatch;
            },
        }
    }
    // let _elapsed = _start.elapsed();
    // dbg!(_elapsed);
    assert!(cur_o_pos<=trgt_len);
    if max_trgt_match_len < trgt_len{
        //if we had prepended naive test, we need to place all of the src at the end.
        ops.push(InnerOp::MatchSrc { start: 0, length: src_len, o_pos: max_trgt_match_len });
    }

    ops

}
enum EncoderState{
    StartNewMatch,
    TryMatch,
    FoundMatch{match_len:usize},
    MoveForwardOneByte,
}

#[inline(always)]
fn find_initial_run_len(seg: &[u8], match_len: usize, run_byte: &mut u8) -> usize {
    let mut run_len = 0;
    let mut last_byte = *run_byte;
    for i in 0..match_len {
        let next_char = seg[i];
        if last_byte == next_char{
            run_len += 1;
        }else{
            run_len = 1;
            last_byte = next_char;
        }
    }
    *run_byte = last_byte;
    run_len
}

#[inline(always)]
fn clear_existing_ops(ops:&mut Vec<InnerOp>,gte_start:usize){
    while ops.last().map(|x|*x.o_pos()).unwrap_or(0) >= gte_start{
        let _evicted = ops.pop().unwrap();
        //dbg!(evicted.o_pos()+evicted.len());
    }
}



#[derive(Debug, Clone)]
pub struct GenericEncoderConfig{
    pub match_trgt: Option<TrgtMatcherConfig>,
    pub match_src: Option<SrcMatcherConfig>,
    /// If the current match is less than lazy_escape_len it steps byte by byte looking for more matches.
    pub lazy_escape_len: Option<usize>,
    pub naive_tests: Option<LargerTrgtNaiveTests>,

}

#[allow(unused)]
impl GenericEncoderConfig {
    pub fn new(match_trgt: Option<TrgtMatcherConfig>, match_src: Option<SrcMatcherConfig>, naive_tests:Option<LargerTrgtNaiveTests>,lazy_escape_len:Option<usize>) -> Self {
        Self { match_trgt, match_src,naive_tests,lazy_escape_len }
    }
}

impl Default for GenericEncoderConfig {
    fn default() -> Self {
        Self { match_trgt: None, match_src: Some(SrcMatcherConfig::comp_level(3)),naive_tests: Some(LargerTrgtNaiveTests::Append),lazy_escape_len: Some(45)}
    }
}