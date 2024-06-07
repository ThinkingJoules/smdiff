
// mod params;
mod gcc_tests;
mod size_tests;
mod vc_to_sm;
mod merge_test;
mod sec_comp;
use sec_comp::{analyze_sec_comp_large_file_best, analyze_sec_comp_sentence_best};
use smdiff_common::Format;
use smdiff_encoder::{brotli::{BlockSize, BrotliEncoderOptions, CompressionMode, Quality, WindowSize}, EncoderConfig, SecondaryCompression, TrgtMatcherConfig};
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
    //merge_2951_2952_2953()?;

    // let config = EncoderConfig::comp_level(9, true, None);
    // println!("{:?}", config);
    // encode_test_gcc_2951_2952(&config)?;
    // encode_test_gcc_2952_2953(&config)?;

    // test_sec_comp_working()?;

    // encode_test_micro()?;
    // encode_test_small()?;
    // encode_test_large()?;
    // vc_to_sm_test()?;

    // ANALYSIS
    // vc_analysis()?;
    // analyze_sec_comp_large_file_worst()?;
    // analyze_sec_comp_large_file_best(true)?;
    // analyze_sec_comp_sentence_best()?;

    new_encode_test_gcc_2951_2952()?;
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

pub fn hash_test_gcc_2951_2952()-> Result<(), Box<dyn std::error::Error>> {
    let mut f_2951 = std::fs::File::open(&std::path::Path::new(DIR_PATH).join("gcc-2.95.1.tar"))?;
    let mut f_2951_bytes = Vec::new();
    f_2951.read_to_end(&mut f_2951_bytes).unwrap();

    println!("2951");
    hash_bytes_sequential(&f_2951_bytes);
    hash_bytes_parallel(&f_2951_bytes, 8);

    let mut f_2952 = std::fs::File::open(&std::path::Path::new(DIR_PATH).join("gcc-2.95.2.tar"))?;
    let mut f_2952_bytes = Vec::new();
    f_2952.read_to_end(&mut f_2952_bytes).unwrap();

    println!("2952");
    hash_bytes_sequential(&f_2952_bytes);
    hash_bytes_parallel(&f_2952_bytes, 8);

    Ok(())
}
use blake3::hash;
use std::{io::Read, ops::Range, sync::Arc, time::Instant};
use std::thread;

const CHUNK_SIZE: usize = u16::MAX as usize;

fn hash_bytes_sequential(input: &[u8]) {
    let start = Instant::now();
    let mut hasher = blake3::Hasher::new();
    hasher.update_rayon(input);
    let elapsed = start.elapsed();
    println!("Sequential hashing took {:?}", elapsed);
}

fn hash_bytes_parallel(input: &[u8], num_threads: usize) -> (Vec<blake3::Hash>, u128) {
    let input_arc = Arc::new(input.to_vec());
    let start = Instant::now();
    let len = input.len();
    let chunks: Vec<Range<usize>> =  (0..len).step_by(CHUNK_SIZE).map(|i| {
        let end = (i + CHUNK_SIZE).min(len);
        i..end
    }).collect();
    let chunk_count = chunks.len();
    let mut results = Vec::with_capacity(chunk_count);
    let mut handles = Vec::with_capacity(num_threads); // Create handles for only the specified number of threads

    for chunk_batch in chunks.chunks(chunks.len() / num_threads + 1) {
        let chunk_batch = chunk_batch.to_vec(); // Clone the chunk references
        let data = Arc::clone(&input_arc);
        let handle = thread::spawn(move || {
            let mut batch_results = Vec::with_capacity(chunk_batch.len());
            for chunk in chunk_batch {
                let slice = &data[chunk];
                batch_results.push(blake3::hash(slice));
            }
            batch_results // Return the results for this batch
        });
        handles.push(handle);
    }

    for handle in handles {
        results.extend(handle.join().unwrap()); // Combine results from all threads
    }

    let elapsed = start.elapsed().as_micros();
    println!("Parallel hashing took {} microseconds", elapsed);
    (results, elapsed)
}
