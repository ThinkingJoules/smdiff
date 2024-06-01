
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;
use std::time::Instant;

use crate::DIR_PATH;

use colored::*;
use smdiff_encoder::{EncoderConfig, TrgtMatcherConfig};



pub fn merge_2951_2952_2953()-> Result<(), Box<dyn std::error::Error>> {
    let mut f_2952 = fs::File::open(&Path::new(DIR_PATH).join("gcc-2.95.2.tar"))?;
    let mut f_2952_bytes = Vec::new();
    f_2952.read_to_end(&mut f_2952_bytes).unwrap();
    let mut f_2951 = fs::File::open(&Path::new(DIR_PATH).join("gcc-2.95.1.tar"))?;
    let mut f_2951_bytes = Vec::new();
    f_2951.read_to_end(&mut f_2951_bytes).unwrap();
    let mut src = Cursor::new(f_2951_bytes);
    let mut trgt = Cursor::new(f_2952_bytes);
    let mut patch_a = Vec::new();
    let start = Instant::now();
    smdiff_encoder::encode(&mut src, &mut trgt, &mut patch_a,&EncoderConfig::default().set_match_target(TrgtMatcherConfig::default()))?;
    let duration = start.elapsed();
    println!("Time elapsed in encode() is: {:?}", duration);
    println!("Patch size SRC+TRGT: {}", patch_a.len());
    let f_2951_bytes = src.into_inner();
    let f_2952_bytes = trgt.into_inner();
    let mut f_2953 = fs::File::open(&Path::new(DIR_PATH).join("gcc-2.95.3.tar"))?;
    let mut f_2953_bytes = Vec::new();
    f_2953.read_to_end(&mut f_2953_bytes).unwrap();
    let mut src = Cursor::new(f_2952_bytes);
    let mut trgt = Cursor::new(f_2953_bytes);
    let mut patch_b = Vec::new();
    let start = Instant::now();
    smdiff_encoder::encode(&mut src, &mut trgt, &mut patch_b,&EncoderConfig::default().set_match_target(TrgtMatcherConfig::default()))?;
    let duration = start.elapsed();
    println!("Time elapsed in encode() is: {:?}", duration);
    println!("Patch size SRC+TRGT: {}", patch_b.len());
    let f_2953_bytes = trgt.into_inner();

    let start = Instant::now();
    let merger = smdiff_merger::Merger::new(Cursor::new(patch_b))?.unwrap();
    let writer = merger.merge(Cursor::new(patch_a))?.unwrap().finish();
    let summary_patch = writer.write(Vec::new(), None).unwrap();
    let duration = start.elapsed();
    println!("Time elapsed in merge() is: {:?}", duration);
    println!("Summary Patch size: {}", summary_patch.len());
    println!("Merge Throughput: {} MB/s", (summary_patch.len() as f64 / duration.as_secs_f64()) / 1_000_000.0);

    let mut decode_sm = Vec::new();
    let start = Instant::now();
    let mut src = Cursor::new(f_2951_bytes);
    let mut reader = Cursor::new(summary_patch);
    smdiff_decoder::apply_patch(&mut reader,Some(&mut src) , &mut decode_sm).unwrap();
    let duration = start.elapsed();
    println!("Time elapsed in apply_patch() is: {:?}", duration);

    if decode_sm != f_2953_bytes{
        //find the first mismatch
        let mut i = 0;
        for (a,b) in decode_sm.iter().zip(f_2953_bytes.iter()){
            if a != b{
                eprintln!("{}", format!("Mismatch at index: {} | Decoded: {} | Target: {}", i, a, b).red());
                break;
            }
            i += 1;
        }
        //print len
        eprintln!("ERROR: Decoded: {} != Target: {}", decode_sm.len(), f_2953_bytes.len());
    }else{
        println!("{}","Merge Patch SUCCESS!".green());
    }

    Ok(())
}