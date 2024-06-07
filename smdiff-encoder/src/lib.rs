
use std::ops::{Range, RangeInclusive};

use encoder::{GenericEncoderConfig, LargerTrgtNaiveTests};
use op_maker::translate_inner_ops;
use smdiff_common::{AddOp, Copy, CopySrc, Format, Run, MAX_INST_SIZE, MAX_WIN_SIZE};
use smdiff_writer::make_sections;
pub use src_matcher::SrcMatcherConfig;
pub use trgt_matcher::TrgtMatcherConfig;
use writer::section_writer;



mod hasher;
mod hashmap;
mod trgt_matcher;
mod src_matcher;
mod op_maker;
mod encoder;
pub mod writer;

pub mod zstd{
//! This module is a re-export of the zstd encoder used in the secondary compression.
    pub use zstd::stream::Encoder;
}
pub mod brotli {
//! This module is a re-export of the brotli encoder used in the secondary compression.
//! It also exports the config options.
    pub use brotlic::{encode::{BrotliEncoderOptions,CompressorWriter},BlockSize,CompressionMode,Quality,WindowSize};
}

/// The Add operation for the encoder.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct Add <'a> {
    bytes: &'a [u8],
}
impl AddOp for Add<'_> {
    fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

type Op<'a> = smdiff_common::Op<Add<'a>>;

/// The secondary compression algorithm to use.
/// Default Value: Zstd { level: 3 }
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
    /// Returns the value to use in the header. Per the spec.
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
///
/// Default values are:
/// - match_src: Some(SrcMatcherConfig::new_from_compression_level(3))
/// - match_target: None
/// - sec_comp: None
/// - format: Interleaved
/// - output_segment_size: MAX_WIN_SIZE
/// - naive_tests: None
/// - lazy_escape_len: Some(45)
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
    /// Use the short hand compression level.
    /// If match_trgt is true, the same compression level will be used to set the TrgtMatcherConfig.
    /// If secondary compression is Some(_), the format will be Segregated, else Interleaved.
    pub fn comp_level(level: usize,match_trgt:bool,sec_comp:Option<SecondaryCompression>) -> Self {
        let match_trgt = if match_trgt {
            Some(TrgtMatcherConfig::comp_level(level))
        }else{
            None
        };
        let format = if sec_comp.is_some() {
            Format::Segregated
        }else{
            Format::Interleaved
        };
        Self {
            match_src: Some(SrcMatcherConfig::comp_level(level)),
            output_segment_size: MAX_WIN_SIZE,
            format,
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
/// Encodes a delta file based on the given configuration and inputs.
/// # Arguments
/// * `dict` - The source file to use as a dictionary. If None, the source file will not be used.
/// * `output` - The target file to encode.
/// * `writer` - The writer to write the encoded data to.
/// * `config` - The configuration to use for the encoder.
/// # Errors
/// Returns an error if there was an issue reading the source or target files, or writing the encoded data.
pub fn encode<R: std::io::Read+std::io::Seek, W: std::io::Write>(dict: Option<&mut R>, output: &mut R, writer: &mut W,config:&EncoderConfig) -> std::io::Result<()> {
    //this simple encoder will just read all the bytes to memory.
    let mut src_bytes = Vec::new();
    if let Some(r) = dict {
        r.read_to_end(&mut src_bytes)?;
    }
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
    // dbg!(&inner_config);
    let ops = translate_inner_ops(trgt, segments);
    let mut cur_o_pos: usize = 0;
    let mut win_data = Vec::new();
    for (seg_ops,mut header) in make_sections(&ops, segment_size){
        header.format = format;
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
        cur_o_pos += header.output_size as usize;
        section_writer(&sec_comp, header, writer, seg_ops, &mut win_data)?; //write the section
    }
    Ok(())
}


/// This just simplifies mapping a 0..9 comp_level to various ranges for various settings.
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