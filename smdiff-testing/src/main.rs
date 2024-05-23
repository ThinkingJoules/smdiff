
// mod params;
mod gcc_tests;
mod size_tests;
mod vc_to_sm;
mod merge_test;
mod sec_comp;
use sec_comp::{analyze_sec_comp_large_file_best, analyze_sec_comp_sentence_best};
use smdiff_common::Format;
use smdiff_encoder::{brotli::{BlockSize, BrotliEncoderOptions, CompressionMode, Quality, WindowSize}, EncoderConfig, SecondaryCompression};
// use params::*;
use vc_to_sm::*;
use size_tests::*;
use gcc_tests::*;
use merge_test::*;

use crate::sec_comp::{analyze_sec_comp_large_file_worst, test_sec_comp_working};
/*
Xdelta3 seems to not produce valid patches.
Alternatively both open-vcdiff and my impl made the same error..
*/
const DIR_PATH: &str = "../target/downloads";
const _URLS: [&str; 6] = [
    "https://dl-cdn.alpinelinux.org/alpine/v3.17/releases/x86_64/alpine-standard-3.17.0-x86_64.iso",
    "https://dl-cdn.alpinelinux.org/alpine/v3.18/releases/x86_64/alpine-standard-3.18.0-x86_64.iso",
    "https://dl-cdn.alpinelinux.org/alpine/v3.19/releases/x86_64/alpine-standard-3.19.0-x86_64.iso",
    "https://mirrors.concertpass.com/gcc/releases/gcc-2.95.1/gcc-2.95.1.tar.gz",
    "https://mirrors.concertpass.com/gcc/releases/gcc-2.95.1/gcc-2.95.2.tar.gz",
    "https://mirrors.concertpass.com/gcc/releases/gcc-2.95.1/gcc-2.95.3.tar.gz",
];
fn main()-> Result<(), Box<dyn std::error::Error>> {
    // TESTS
    merge_2951_2952_2953()?;

    let config = EncoderConfig::new(true, 16, None, Format::Interleaved);
    println!("{:?}", config);
    encode_test_gcc_2951_2952(&config)?;
    encode_test_gcc_2952_2953(&config)?;

    test_sec_comp_working()?;

    encode_test_micro()?;
    encode_test_small()?;
    encode_test_large()?;
    vc_to_sm_test()?;

    // ANALYSIS
    // vc_analysis()?;
    // analyze_sec_comp_large_file_worst()?;
    // analyze_sec_comp_large_file_best()?;
    // analyze_sec_comp_sentence_best()?;
    Ok(())
}


#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Stats{
    pub add_bytes:usize,
    pub run_bytes:usize,
    pub copy_bytes:usize,
    pub add_cnt:usize,
    pub run_cnt:usize,
    pub copy_s_cnt:usize,
    pub copy_t_cnt:usize,
    pub output_size:usize,
}

impl Stats {
    pub fn new() -> Self {
        Default::default()
    }
    pub fn add(&mut self, len:usize){
        self.add_bytes += len;
        self.add_cnt += 1;
        self.output_size += len;
    }
    pub fn run(&mut self, len:usize){
        self.run_bytes += len;
        self.run_cnt += 1;
        self.output_size += len;
    }
    pub fn copy_s(&mut self, len:usize){
        self.copy_bytes += len;
        self.copy_s_cnt += 1;
        self.output_size += len;
    }
    pub fn copy_t(&mut self, len:usize){
        self.copy_bytes += len;
        self.copy_t_cnt += 1;
        self.output_size += len;
    }
    pub fn has_copy(&self)->bool{
        self.copy_bytes > 0
    }
}