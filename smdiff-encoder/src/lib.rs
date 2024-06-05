
use std::{io::Write, ops::{Range, RangeInclusive}};

use encoder::{GenericEncoderConfig, LargerTrgtNaiveTests};
use op_maker::translate_inner_ops;
use smdiff_common::{AddOp, Copy, CopySrc, Format, Run, SectionHeader, MAX_INST_SIZE, MAX_WIN_SIZE};
use smdiff_writer::{write_section_header, write_ops};
pub use src_matcher::SrcMatcherConfig;
pub use trgt_matcher::TrgtMatcherConfig;



mod hasher;
mod hashmap;
mod trgt_matcher;
mod src_matcher;
mod op_maker;
mod encoder;

pub mod zstd{
    pub use zstd::stream::Encoder;
}
pub mod brotli {
    pub use brotlic::{encode::{BrotliEncoderOptions,CompressorWriter},BlockSize,CompressionMode,Quality,WindowSize};
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Add <'a> {
    bytes: &'a [u8],
}
impl AddOp for Add<'_> {
    fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}
pub type Op<'a> = smdiff_common::Op<Add<'a>>;

#[derive(Clone, Debug)]
pub enum SecondaryCompression {
    /// Default Value: TrgtMatcherConfig::new_from_compression_level(3)
    Smdiff(TrgtMatcherConfig),
    /// Value of 1..=22.
    /// Default Value: 3
    Zstd{level:i32},
    /// Default Value: BrotliEncoderOptions::default()
    Brotli{options: ::brotlic::BrotliEncoderOptions},
}

impl SecondaryCompression {
    pub fn new_smdiff_default() -> Self {
        SecondaryCompression::Smdiff (TrgtMatcherConfig::comp_level(3))
    }

    pub fn new_zstd_default() -> Self {
        SecondaryCompression::Zstd { level: 3 }
    }

    pub fn new_brotli_default() -> Self {
        SecondaryCompression::Brotli { options: ::brotlic::BrotliEncoderOptions::default() }
    }
    pub fn algo_value(&self) -> u8 {
        match self {
            SecondaryCompression::Smdiff { .. } => 1,
            SecondaryCompression::Zstd { .. } => 2,
            SecondaryCompression::Brotli { .. } => 3,
        }
    }
}
impl Default for SecondaryCompression {
    fn default() -> Self {
        Self::new_zstd_default()
    }
}

/// Configuration for the encoder.
/// Default values are:
/// - match_src: Some(SrcMatcherConfig::new_from_compression_level(3))
/// - match_target: None
/// - sec_comp: None
/// - format: Interleaved
#[derive(Clone, Debug)]
pub struct EncoderConfig {
    /// Do we consider the src file as a dictionary to find matches?
    /// If so (Some(_)), any preferences set in the MatcherConfig will be used.
    /// Default Value: Some(SrcMatcherConfig::new_from_compression_level(3))
    pub match_src: Option<SrcMatcherConfig>,
    /// Whether to use the output file in an attempt to compress itself.
    /// If so (Some(_)), any preferences set in the MatcherConfig will be used.
    /// Default Value: None
    pub match_trgt: Option<TrgtMatcherConfig>,
    /// None for no secondary compression.
    /// Default Value: None
    pub sec_comp: Option<SecondaryCompression>,
    /// Whether to interleave or segregate the Add bytes.
    /// Default Value: Interleaved
    pub format: Format,
    /// The size of the output window.
    /// Default Value: MAX_WIN_SIZE
    /// The minimum value is MAX_INST_SIZE.
    pub output_segment_size: usize,
    /// The types of naive tests to run.
    /// Default Value: None
    pub naive_tests: Option<LargerTrgtNaiveTests>,
    /// The length of a match that will end the lazy matching sequence.
    /// Default Value: Some(45)
    pub lazy_escape_len: Option<usize>,

}

impl EncoderConfig {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn no_match_src(mut self) -> Self {
        self.match_src = None;
        self
    }
    pub fn no_match_target(mut self) -> Self {
        self.match_trgt = None;
        self
    }
    pub fn no_sec_comp(mut self) -> Self {
        self.sec_comp = None;
        self
    }
    pub fn set_match_src(mut self, config: SrcMatcherConfig) -> Self {
        self.match_src = Some(config);
        self
    }
    pub fn set_sec_comp(mut self, sec_comp: SecondaryCompression) -> Self {
        self.sec_comp = Some(sec_comp);
        self
    }
    pub fn format_interleaved(mut self) -> Self {
        self.format = Format::Interleaved;
        self
    }
    pub fn format_segregated(mut self) -> Self {
        self.format = Format::Segregated;
        self
    }
    pub fn set_match_target(mut self, config: TrgtMatcherConfig) -> Self {
        self.match_trgt = Some(config);
        self
    }
    pub fn set_output_segment_size(mut self, size: usize) -> Self {
        self.output_segment_size = size;
        self
    }
    pub fn set_naive_tests(mut self, tests: LargerTrgtNaiveTests) -> Self {
        self.naive_tests = Some(tests);
        self
    }
    pub fn set_lazy_escape_len(mut self, len: usize) -> Self {
        self.lazy_escape_len = Some(len);
        self
    }
    pub fn comp_level(level: usize,match_trgt:bool,sec_comp:Option<SecondaryCompression>) -> Self {
        let match_trgt = if match_trgt {
            Some(TrgtMatcherConfig::comp_level(level))
        }else{
            None
        };
        Self {
            match_src: Some(SrcMatcherConfig::comp_level(level)),
            output_segment_size: MAX_WIN_SIZE,
            format: Format::Interleaved,
            match_trgt,
            sec_comp,
            naive_tests: None,
            lazy_escape_len: None,
        }
    }
}
impl Default for EncoderConfig {
    fn default() -> Self {
        Self {
            match_src: Some(SrcMatcherConfig::comp_level(3)),
            output_segment_size: MAX_WIN_SIZE,
            format: Format::Interleaved,
            match_trgt: None,
            sec_comp: None,
            naive_tests: None,
            lazy_escape_len: None,
        }
    }
}
pub fn encode<R: std::io::Read+std::io::Seek, W: std::io::Write>(dict: &mut R, output: &mut R, mut writer: &mut W,config:&EncoderConfig) -> std::io::Result<()> {
    //this simple encoder will just read all the bytes to memory.
    let mut src_bytes = Vec::new();
    dict.read_to_end(&mut src_bytes)?;
    let mut trgt_bytes = Vec::new();
    output.read_to_end(&mut trgt_bytes)?;
    let src = src_bytes.as_slice();
    let trgt = trgt_bytes.as_slice();
    let EncoderConfig { match_src, match_trgt, sec_comp, format,output_segment_size, naive_tests, lazy_escape_len } = config.clone();
    let segment_size = output_segment_size.min(MAX_WIN_SIZE).max(MAX_INST_SIZE);
    let mut inner_config = GenericEncoderConfig{
        match_trgt,
        match_src,
        lazy_escape_len,
        naive_tests,
    };
    let segments = encoder::encode_inner(&mut inner_config, src, trgt);
    dbg!(&inner_config);
    let ops = translate_inner_ops(&inner_config, trgt, segments);
    let mut cur_o_pos: usize = 0;
    let mut i = 0;
    let mut win_data = Vec::new();
    let segments = make_segments(&ops, segment_size);
    for mut header in segments{
        header.format = format;
        let end_range = i + header.num_operations as usize;
        let seg_ops = &ops[i..end_range];
        debug_assert!({
            let mut o = cur_o_pos;
            seg_ops.iter().all(
                |op| {
                    let len = op.oal() as usize;
                    let test = &trgt[o..o + len];
                    o += len;
                    match op{
                        Op::Add(Add { bytes }) => test == &bytes[..],
                        Op::Copy(Copy { src:CopySrc::Dict, addr, len }) => test == &src[*addr as usize..*addr as usize + *len as usize],
                        Op::Copy(Copy { src:CopySrc::Output, addr, len }) => test == &trgt[*addr as usize..*addr as usize + *len as usize],
                        Op::Run(Run{ byte, .. }) => test.iter().all(|b| b == byte),
                    }
                }
            )
        });
        i = end_range;
        cur_o_pos += header.output_size as usize;
        if sec_comp.is_some() {
            let comp = sec_comp.clone().unwrap();
            header.compression_algo = comp.algo_value();
            //dbg!(&header);
            write_section_header(&header, writer)?;
            write_ops(seg_ops,&header,&mut win_data)?;
            match comp{
                SecondaryCompression::Smdiff (config) => {
                    let mut crsr = std::io::Cursor::new(&mut win_data);
                    let inner_config = EncoderConfig::default().no_match_src().set_match_target(config);
                    encode(&mut std::io::Cursor::new(&mut Vec::new()), &mut crsr, writer, &inner_config)?;
                },
                SecondaryCompression::Zstd { level } => {
                    let mut a = ::zstd::Encoder::new(writer, level)?;
                    a.set_pledged_src_size(Some(win_data.len() as u64))?;
                    a.include_contentsize(true)?;
                    a.write_all(&win_data)?;
                    writer = a.finish()?;
                },
                SecondaryCompression::Brotli { mut options }=> {
                    options.size_hint(win_data.len() as u32);
                    let mut a = ::brotlic::CompressorWriter::with_encoder(options.build().unwrap(), writer);
                    a.write_all(&win_data)?;
                    writer = a.into_inner()?;
                },
            }
            win_data.clear();
        }else{
            write_section_header(&header, writer)?;
            write_ops(seg_ops,&header,writer)?;
        }
    }
    Ok(())
}


fn make_segments(ops: &[Op], segment_size: usize) -> Vec<SectionHeader> {
    let (mut segments, last_sum,last_adds, last_start) = ops.iter().enumerate().fold(
        (Vec::new(), 0, 0, 0), // (segments, total_bytes, num_add_bytes, start_index)
        |(mut segments, current_sum, add_bytes, start_index), (i, op)| {
            let op_len = op.oal() as usize;
            let add_len = if op.is_add() { op_len } else { 0 };
            let new_sum = current_sum + op_len as usize;
            if new_sum > segment_size {
                segments.push(SectionHeader::new(
                    (i-start_index) as u32,
                    add_bytes as u32,
                    current_sum as u32
                    ).set_more_sections(true));
                (segments,op_len, add_len, i)
            } else {
                (segments, new_sum, add_bytes+add_len, start_index)
            }
        },
    );

    // Check if the last segment was added or if the elements end naturally
    if last_sum > 0 {
        segments.push(SectionHeader::new(
            (ops.len()-last_start) as u32,
            last_adds as u32,
            last_sum as u32
        ).set_more_sections(false));
    }else{
        segments.last_mut().unwrap().set_more_sections(false);
    }

    segments
}

fn max_unique_substrings_gt_hash_len(hash_win_len: usize, win_size: usize, l_step: usize) -> usize {
    let m = win_size / l_step;
    let mut x = 0;
    while m / (hash_win_len + x) > 256usize.pow(x as u32) - 1 {
        x += 1;
    }
    std::cmp::min(m / (hash_win_len + x),256usize.pow(x as u32))
}

/// Returns the cost of the next address based on the absolute distance from `cur_addr`.
fn _addr_cost(cur_addr: u64, next_addr: u64) -> isize {
    let mut diff = cur_addr as i64 - next_addr as i64;
    if diff<0 {
        diff += 1 //to get the correct range for negative values
    }
    let diff = diff.abs() as u64;

    match diff {
        0..=63 => -1,            // closest range
        64..=8191 => -2,         // second closest range
        8192..=1048575 => -3,    // third closest range
        1048576..=134217727 => -4, // fourth closest range
        _ => -5,                // beyond expected range
    }
}


// #[derive(Copy, Clone, Debug, Default, Ord, Eq)]
// struct CopyScore{
//     pub score: isize,
//     pub size: usize,
//     pub addr_cost: isize,
//     pub size_cost: isize,
//     pub start: usize,
// }
// impl PartialEq for CopyScore {
//     fn eq(&self, other: &Self) -> bool {
//         self.score == other.score
//             && self.size == other.size
//             && self.addr_cost == other.addr_cost
//             && self.size_cost == other.size_cost
//     }
// }

// impl PartialOrd for CopyScore {
//     fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
//         let score_cmp = self.score.partial_cmp(&other.score);
//         if score_cmp != Some(Ordering::Equal) {
//             return score_cmp;
//         }

//         let size_cmp = self.size.partial_cmp(&other.size);
//         if size_cmp != Some(Ordering::Equal) {
//             return size_cmp;
//         }

//         let addr_cost_cmp = self.addr_cost.partial_cmp(&other.addr_cost);
//         if addr_cost_cmp != Some(Ordering::Equal) {
//             return addr_cost_cmp;
//         }

//         let size_cost_cmp = self.size_cost.partial_cmp(&other.size_cost);
//         if size_cost_cmp != Some(Ordering::Equal) {
//             return size_cost_cmp;
//         }

//         Some(Ordering::Equal)
//     }
// }

// impl CopyScore {
//     fn new(addr_cost:isize, size:usize,start:usize)->Self{
//         let size_cost = size_cost(size);
//         let score = addr_cost.saturating_add(size_cost).saturating_add(size as isize).saturating_sub(-1);
//         CopyScore{score,addr_cost,size_cost,size,start}
//     }
//     // fn compression_ratio(&self)->f64{
//     //     self.size as f64 / ((self.addr_cost + self.size_cost -1)*-1) as f64
//     // }
// }
// fn size_cost(size: usize) -> isize {
//     -((size > 62) as isize) - (size > 317) as isize
// }

struct Ranger {
    input_range: Range<usize>,
    output_range: RangeInclusive<usize>,
    input_span: usize,
    output_span: usize,
    is_inverted: bool,
}

impl Ranger {
    fn new(input_range: Range<usize>, output_range: RangeInclusive<usize>) -> Self {
        let input_span = input_range.end - input_range.start - 1;
        let is_inverted = output_range.start() > output_range.end();
        let output_span = output_range.end().abs_diff(*output_range.start());

        Self { input_range, output_range, input_span, output_span, is_inverted }
    }

    fn map(&self, input_value: usize) -> usize {
        let input_value = input_value.clamp(self.input_range.start, self.input_range.end-1);
        let b = self.output_range.start().min(self.output_range.end());
        let m = input_value - self.input_range.start;
        //let m = if self.is_inverted {self.input_range.end - input_value}else{input_value-self.input_range.start};
        let output = b + ((self.output_span * m) / self.input_span);
        if self.is_inverted{
            self.output_span+self.output_range.end() - output + self.output_range.end()
        }else{
            output
        }
        //Some(output.clamp(*b, b+self.output_span))
    }
}



#[cfg(test)]
mod test_super {
    use super::*;

    #[test]
    fn test_calculate_prev_list_size() {
        let test_cases = [
            (3, 1 << 26, 2, 4),
            (5, 1 << 26, 2, 4),
            (7, 1 << 26, 2, 4),
            (9, 1 << 26, 2, 4),
            (4, 1 << 28, 2, 4),
            (5, 1 << 28, 2, 4),
            (6, 1 << 28, 2, 4),
            (7, 1 << 28, 2, 4),
            (8, 1 << 28, 2, 4),
            (3, 1 << 7, 2, 4),
            (4, 16_383, 2, 4),
            (5, 2_097_151, 2, 4),
            (6, 6_998_841, 2, 4),
            (7, 23_541_202, 2, 4),
            (8, 79_182_851, 2, 4),
        ];

        for (hash_win_len, win_size, l_step, _expected_chain_len) in test_cases {
            let chain_len = max_unique_substrings_gt_hash_len(hash_win_len, win_size, l_step);
            println!("hash_win_len={}, win_size={}, l_step={}, chain_len={} ({})", hash_win_len, win_size, l_step, chain_len,(chain_len + chain_len/2).next_power_of_two()>>1);
            // assert_eq!(chain_len, expected_chain_len,
            //            "Failed for hash_win_len={}, win_size={}, l_step={}",
            //            hash_win_len, win_size, l_step);
        }
    }

    #[test]
    fn test_regular_mapping() {
        let input_range = 1..11;
        let output_range = 1..=100;
        let interpolator = Ranger::new(input_range, output_range);

        assert_eq!(interpolator.map(1), 1);
        assert_eq!(interpolator.map(2), 12);
        assert_eq!(interpolator.map(3), 23);
        assert_eq!(interpolator.map(4), 34);
        assert_eq!(interpolator.map(5), 45);
        assert_eq!(interpolator.map(6), 56);
        assert_eq!(interpolator.map(7), 67);
        assert_eq!(interpolator.map(8), 78);
        assert_eq!(interpolator.map(9), 89);
        assert_eq!(interpolator.map(10), 100);
    }
    #[test]
    fn test_regular_mapping2() {
        let input_range = 0..10;
        let output_range = 26..=2;
        let interpolator = Ranger::new(input_range, output_range);

        assert_eq!(interpolator.map(9), 2);

    }

    #[test]
    fn test_inverted_mapping() {
        let input_range = 1..11;
        let output_range = 100..=1; // Inverted range
        let interpolator = Ranger::new(input_range, output_range);

        assert_eq!(interpolator.map(1), 100);
        assert_eq!(interpolator.map(5), 56);
        assert_eq!(interpolator.map(10), 1);
    }

    #[test]
    fn test_out_of_range_input() {
        let input_range = 3..10;
        let output_range = 0..=100;
        let interpolator = Ranger::new(input_range, output_range);

        assert_eq!(interpolator.map(0), interpolator.map(3)); // Below range
        assert_eq!(interpolator.map(11), interpolator.map(10)); // Above range
    }

}