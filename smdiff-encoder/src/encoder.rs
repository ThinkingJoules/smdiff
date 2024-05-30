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
use crate::{hasher::{HasherCusor, LargeHashCursor, SmallHashCursor}, src_matcher::{SrcMatcher, SrcMatcherConfig}, trgt_matcher::{self, TrgtMatcher, TrgtMatcherConfig}};


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
    }
    pub(crate) fn split_at(mut self,abs_split_pos:usize)->(Self,Self){
        let o_pos = *self.o_pos();
        assert!(abs_split_pos > o_pos && abs_split_pos < o_pos + *self.len());
        let mut new = self.clone();
        self.set_len(abs_split_pos - o_pos);
        new.set_o_pos(abs_split_pos);
        (self,new)
    }
    pub(crate) fn is_src(&self)->bool{
        match self {
            InnerOp::MatchSrc{..} => true,
            InnerOp::MatchTrgt{..} => false,
            InnerOp::Run{..} => false,
        }
    }
    pub(crate) fn is_trgt(&self)->bool{
        match self {
            InnerOp::MatchSrc{..} => false,
            InnerOp::MatchTrgt{..} => true,
            InnerOp::Run{..} => false,
        }
    }
    pub(crate) fn is_run(&self)->bool{
        match self {
            InnerOp::MatchSrc{..} => false,
            InnerOp::MatchTrgt{..} => false,
            InnerOp::Run{..} => true,
        }
    }
}

//we do not do equality check on src and trgt, that is the job of the caller.
///Returns all possible operations to encode src into trgt.
/// This means ops may overlap or not.
/// It is up to the caller to decide how to handle the gaps and overlaps
pub(crate) fn encode_inner(config:&mut EncoderConfig,src:&[u8],trgt:&[u8])->Vec<InnerOp>{
    let lazy_escape_len = config.lazy_escape_len;
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


    let trgt_matcher = config.match_trgt.as_mut().map(|c| c.build(trgt, cur_o_pos));
    let mut src_matcher = config.match_src.as_mut().map(|c| c.build(src, cur_o_pos, trgt_len));
    //now we decide our matcher configs, at least one of these will be Some.
    let min_match_value = src_matcher.as_ref()
        .map(|m| m.hash_win_len())
        .unwrap_or(usize::MAX)
        .min(trgt_matcher.as_ref()
            .map(|m| m.hash_win_len())
            .unwrap_or(usize::MAX)
        );

    let mut min_match = min_match_value;
    let mut large_hasher = src_matcher.as_ref().map(|m|LargeHashCursor::new(trgt,m.hash_win_len()));
    let mut small_hasher = trgt_matcher.as_ref().map(|m|SmallHashCursor::new(&trgt,m.hash_win_len()));
    let mut run_len ;
    let mut run_byte = 0;

    let try_lazy = |cur:&mut usize,match_len,mm:&mut usize|{
        let cont = lazy_escape_len > 0 && match_len < lazy_escape_len && *cur + match_len < max_trgt_match_len - 1;
        if cont {*mm = match_len;}else{*cur += match_len;}
        cont
    };
    //now we start the main loop.
    loop {
        //first we see if we are out of data.
        if cur_o_pos + min_match_value >= max_trgt_match_len {
            break;
        }
        let remaining_trgt = &trgt[cur_o_pos..];
        //we setup for trying to start a new match
        run_len = find_initial_run_len(remaining_trgt, min_match_value, &mut run_byte);
        small_hasher.as_mut().map(|h|h.seek(cur_o_pos));
        if let Some(matcher) = src_matcher.as_mut(){
            matcher.center_on(cur_o_pos);
            large_hasher.as_mut().map(|h|h.seek(cur_o_pos));
        };
        //adjust our min_match so lazy matching works.
        let last_match_pos = ops.last().map(|x|*x.o_pos()).unwrap_or(0);
        let last_match_size = ops.last().map(|x|*x.len()).unwrap_or(0);
        min_match = if last_match_pos > cur_o_pos {
            min_match_value.max(1 + (last_match_pos + last_match_size) - cur_o_pos)
        } else {min_match_value};

        //we attempt to find matches, one byte at a time on the remaining input
        'search: loop {
            if run_len == min_match_value{
                //try expanding this run
                remaining_trgt[run_len..].iter().take_while(|&x| x == &run_byte).for_each(|_|run_len+=1);
                if run_len >= min_match_value{
                    ops.push(InnerOp::Run{byte:run_byte as u8,length:run_len,o_pos:cur_o_pos});
                    if try_lazy(&mut cur_o_pos, run_len, &mut min_match){
                        advance_one_byte(&mut small_hasher, &mut large_hasher);
                        continue 'search;
                    }else{
                        break 'search;
                    }
                }
            }

            if let Some(hasher) = large_hasher.as_mut(){
                if cur_o_pos + hasher.win_size() <= max_trgt_match_len{
                    let matcher = src_matcher.as_ref().unwrap();
                    if let Some((src_start,pre_match,post_match)) = matcher.find_best_src_match(src, trgt, cur_o_pos, hasher.hash()) {
                        if post_match >= min_match{
                            let length = pre_match + post_match;
                            if pre_match > 0{
                                //remove all ops that are fully before this start position
                                clear_existing_ops(&mut ops, cur_o_pos - pre_match)
                            }
                            ops.push(InnerOp::MatchSrc{start:src_start, length, o_pos:cur_o_pos});
                            if try_lazy(&mut cur_o_pos,post_match, &mut min_match){
                                advance_one_byte(&mut small_hasher, &mut large_hasher);
                                continue 'search;
                            }else{
                                break 'search;
                            }
                        }
                    }
                }
            }

            if let Some(hasher) = small_hasher.as_mut(){
                if cur_o_pos + hasher.win_size() <= max_trgt_match_len{
                    let matcher = trgt_matcher.as_ref().unwrap();
                    if let Some((match_start,length)) = matcher.find_best_trgt_match( trgt, cur_o_pos, hasher.hash()) {
                        if length >= min_match{
                            ops.push(InnerOp::MatchTrgt{start:match_start, length, o_pos:cur_o_pos});
                            if try_lazy(&mut cur_o_pos,length, &mut min_match){
                                advance_one_byte(&mut small_hasher, &mut large_hasher);
                                continue 'search;
                            }else{
                                break 'search;
                            }
                        }
                    }
                }
            }
            //increment our encoder forward one byte
            cur_o_pos += 1;
            if min_match > min_match_value{
                //we only need to exceed our min match by one less to match new bytes
                //since we are moving forward.
                min_match -= 1;
            }

        }
    }
    assert!(cur_o_pos<=trgt_len);
    if max_trgt_match_len < trgt_len{
        //if we had prepended naive test, we need to place all of the src at the end.
        ops.push(InnerOp::MatchSrc { start: 0, length: src_len, o_pos: max_trgt_match_len });
    }

    ops

}

#[inline]
fn find_initial_run_len(seg: &[u8], match_len: usize, run_byte: &mut u8) -> usize {
    *run_byte = seg[0];
    seg.iter().take(match_len).take_while(|&x| x == run_byte).count()
}
// #[inline]
// fn try_lazy(lazy_escape_len:usize,match_len: usize,cur_o_pos:usize,max_pos:usize)->bool{
//     lazy_escape_len > 0 && match_len < lazy_escape_len && cur_o_pos + match_len < max_pos - 1
// }
#[inline]
fn advance_one_byte(small_hasher: &mut Option<SmallHashCursor>,large_hasher: &mut Option<LargeHashCursor>){
    if let Some(h) = small_hasher{
        h.next();
    }
    if let Some(h) = large_hasher{
        h.next();
    }
}
#[inline]
fn clear_existing_ops(ops:&mut Vec<InnerOp>,gte_start:usize){
    while ops.last().map(|x|*x.o_pos()).unwrap_or(0) >= gte_start{
        ops.pop();
    }
}
struct EncoderState{
    out_pos:usize,
    min_match:usize,
}





#[derive(Debug, Clone)]
pub struct EncoderConfig{
    pub match_trgt: Option<TrgtMatcherConfig>,
    pub match_src: Option<SrcMatcherConfig>,
    /// If the current match is less than lazy_escape_len it steps byte by byte looking for more matches.
    pub lazy_escape_len: usize,
    pub naive_tests: Option<LargerTrgtNaiveTests>,

}

impl EncoderConfig {
    pub fn new(match_trgt: Option<TrgtMatcherConfig>, match_src: Option<SrcMatcherConfig>, naive_tests:Option<LargerTrgtNaiveTests>,lazy_escape_len:usize) -> Self {
        Self { match_trgt, match_src,naive_tests,lazy_escape_len }
    }
}

impl Default for EncoderConfig {
    fn default() -> Self {
        Self { match_trgt: None, match_src: Some(SrcMatcherConfig::new_from_compression_level(3)),naive_tests: Some(LargerTrgtNaiveTests::Append),lazy_escape_len: 16}
    }
}