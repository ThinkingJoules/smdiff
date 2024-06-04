
use std::fs;
use std::io::{Cursor, Read, Seek};
use std::path::Path;
use std::time::Instant;
use colored::*;
use smdiff_common::MAX_INST_SIZE;
use smdiff_encoder::{EncoderConfig, SrcMatcherConfig, TrgtMatcherConfig};

use crate::DIR_PATH;

pub fn encode_test_gcc_2951_2952(config:&EncoderConfig)-> Result<(), Box<dyn std::error::Error>> {
    let mut f_2952 = fs::File::open(&Path::new(DIR_PATH).join("gcc-2.95.2.tar"))?;
    let mut f_2952_bytes = Vec::new();
    f_2952.read_to_end(&mut f_2952_bytes).unwrap();
    let mut src = Cursor::new(Vec::new());
    let mut trgt = Cursor::new(f_2952_bytes);
    let mut patch = Vec::new();
    let start = Instant::now();
    smdiff_encoder::encode(&mut src, &mut trgt, &mut patch,&config.clone().no_match_src())?;
    let duration = start.elapsed();
    println!("Time elapsed in encode() is: {:?}", duration);
    println!("Patch size Target Only (Compress): {}", patch.len());

    let mut decode_sm = Vec::new();
    let start = Instant::now();
    src.rewind()?;
    let mut reader = Cursor::new(patch);
    smdiff_decoder::apply_patch(&mut reader,Some(&mut src) , &mut decode_sm).unwrap();
    let duration = start.elapsed();
    println!("Time elapsed in apply_patch() is: {:?}", duration);
    let f_2952_bytes = trgt.into_inner();

    if decode_sm != f_2952_bytes{
        //find the first mismatch
        let mut i = 0;
        for (a,b) in decode_sm.iter().zip(f_2952_bytes.iter()){
            if a != b{
                eprintln!("{}", format!("Mismatch at index: {} | Decoded: {} | Target: {}", i, a, b).red());
                break;
            }
            i += 1;
        }
        //print len
        eprintln!("ERROR: Decoded: {} != Target: {}", decode_sm.len(), f_2952_bytes.len());
    }else{
        println!("{}","Patch SUCCESS!".green());
    }

    let mut f_2951 = fs::File::open(&Path::new(DIR_PATH).join("gcc-2.95.1.tar"))?;
    let mut f_2951_bytes = Vec::new();
    f_2951.read_to_end(&mut f_2951_bytes).unwrap();
    let mut src = Cursor::new(f_2951_bytes);
    let mut trgt = Cursor::new(f_2952_bytes);
    let mut patch = Vec::new();
    let start = Instant::now();
    smdiff_encoder::encode(&mut src, &mut trgt, &mut patch,&config.clone().no_match_target())?;
    let duration = start.elapsed();
    println!("Time elapsed in encode() is: {:?}", duration);
    println!("Patch size SRC only: {}", patch.len());
    let f_2952_bytes = trgt.into_inner();

    let mut decode_sm = Vec::new();
    let start = Instant::now();
    src.rewind()?;
    let mut reader = Cursor::new(patch);
    smdiff_decoder::apply_patch(&mut reader,Some(&mut src) , &mut decode_sm).unwrap();
    let duration = start.elapsed();
    println!("Time elapsed in apply_patch() is: {:?}", duration);

    if decode_sm != f_2952_bytes{
        //find the first mismatch
        let mut i = 0;
        for (a,b) in decode_sm.iter().zip(f_2952_bytes.iter()){
            if a != b{
                eprintln!("{}", format!("Mismatch at index: {} | Decoded: {} | Target: {}", i, a, b).red());
                break;
            }
            i += 1;
        }
        //print len
        eprintln!("ERROR: Decoded: {} != Target: {}", decode_sm.len(), f_2952_bytes.len());
    }else{
        println!("{}","Patch SUCCESS!".green());
    }

    src.rewind()?;
    let mut trgt = Cursor::new(f_2952_bytes);
    let mut patch = Vec::new();
    let start = Instant::now();
    smdiff_encoder::encode(&mut src, &mut trgt, &mut patch,config)?;
    let duration = start.elapsed();
    println!("Time elapsed in encode() is: {:?}", duration);
    println!("Patch size SRC+TRGT: {}", patch.len());
    let f_2952_bytes = trgt.into_inner();

    let mut decode_sm = Vec::new();
    let start = Instant::now();
    src.rewind()?;
    let mut reader = Cursor::new(patch);
    smdiff_decoder::apply_patch(&mut reader,Some(&mut src) , &mut decode_sm).unwrap();
    let duration = start.elapsed();
    println!("Time elapsed in apply_patch() is: {:?}", duration);

    if decode_sm != f_2952_bytes{
        //find the first mismatch
        let mut i = 0;
        for (a,b) in decode_sm.iter().zip(f_2952_bytes.iter()){
            if a != b{
                eprintln!("{}", format!("Mismatch at index: {} | Decoded: {} | Target: {}", i, a, b).red());
                break;
            }
            i += 1;
        }
        //print len
        eprintln!("ERROR: Decoded: {} != Target: {}", decode_sm.len(), f_2952_bytes.len());
    }else{
        println!("{}","Patch SUCCESS!".green());
    }

    Ok(())
}

pub fn encode_test_gcc_2952_2953(config:&EncoderConfig)-> Result<(), Box<dyn std::error::Error>> {
    let mut src = Cursor::new(Vec::new());
    let mut f_2953 = fs::File::open(&Path::new(DIR_PATH).join("gcc-2.95.3.tar"))?;
    let mut f_2953_bytes = Vec::new();
    f_2953.read_to_end(&mut f_2953_bytes).unwrap();
    let mut trgt = Cursor::new(f_2953_bytes);
    let mut patch = Vec::new();
    let start = Instant::now();
    smdiff_encoder::encode(&mut src, &mut trgt, &mut patch,&config.clone().no_match_src()).unwrap();
    let duration = start.elapsed();
    println!("Time elapsed in encode() is: {:?}", duration);
    println!("Patch size Target Only (Compress): {}", patch.len());

    let mut decode_sm = Vec::new();
    let start = Instant::now();
    src.rewind()?;
    let mut reader = Cursor::new(patch);
    smdiff_decoder::apply_patch(&mut reader,Some(&mut src) , &mut decode_sm).unwrap();
    let duration = start.elapsed();
    println!("Time elapsed in apply_patch() is: {:?}", duration);
    let f_2953_bytes = trgt.into_inner();

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
        println!("{}","Patch SUCCESS!".green());
    }

    let mut f_2952 = fs::File::open(&Path::new(DIR_PATH).join("gcc-2.95.1.tar"))?;
    let mut f_2952_bytes = Vec::new();
    f_2952.read_to_end(&mut f_2952_bytes).unwrap();
    let mut src = Cursor::new(f_2952_bytes);
    let mut trgt = Cursor::new(f_2953_bytes);
    let mut patch = Vec::new();
    let start = Instant::now();
    smdiff_encoder::encode(&mut src, &mut trgt, &mut patch,&config.clone().no_match_target())?;
    let duration = start.elapsed();
    println!("Time elapsed in encode() is: {:?}", duration);
    println!("Patch size SRC only: {}", patch.len());
    let f_2953_bytes = trgt.into_inner();

    let mut decode_sm = Vec::new();
    let start = Instant::now();
    src.rewind()?;
    let mut reader = Cursor::new(patch);
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
        println!("{}","Patch SUCCESS!".green());
    }

    src.rewind()?;
    let mut trgt = Cursor::new(f_2953_bytes);
    let mut patch = Vec::new();
    let start = Instant::now();
    smdiff_encoder::encode(&mut src, &mut trgt, &mut patch,config)?;
    let duration = start.elapsed();
    println!("Time elapsed in encode() is: {:?}", duration);
    println!("Patch size SRC+TRGT: {}", patch.len());
    let f_2953_bytes = trgt.into_inner();

    let mut decode_sm = Vec::new();
    let start = Instant::now();
    src.rewind()?;
    let mut reader = Cursor::new(patch);
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
        println!("{}","Patch SUCCESS!".green());
    }

    Ok(())
}



pub fn new_encode_test_gcc_2951_2952()-> Result<(), Box<dyn std::error::Error>> {
    let mut f_2952 = fs::File::open(&Path::new(DIR_PATH).join("gcc-2.95.2.tar"))?;
    let mut f_2952_bytes = Vec::new();
    f_2952.read_to_end(&mut f_2952_bytes).unwrap();
    let mut f_2951 = fs::File::open(&Path::new(DIR_PATH).join("gcc-2.95.1.tar"))?;
    let mut f_2951_bytes = Vec::new();
    f_2951.read_to_end(&mut f_2951_bytes).unwrap();
    let mut src = Cursor::new(f_2951_bytes);
    let mut trgt = Cursor::new(f_2952_bytes);
    let mut patch = Vec::new();
    let start = Instant::now();
    smdiff_encoder::encode(
        &mut src,
        &mut trgt,
        &mut patch,
        &EncoderConfig::default()

        .no_match_src().set_match_target(TrgtMatcherConfig {
            compress_early_exit: 70,
            chain_check: 44,
            prev_table_capacity: None,
            hash_win_len: Some(4)
        })


        // .set_match_src(
        //     SrcMatcherConfig {
        //         l_step: 1,
        //         chain_check: 10,
        //         prev_table_capacity: Some(1 << 16),
        //         max_src_win_size: Some(1 << 26),//None, //Some(1 << 24),
        //         hash_win_len:Some(8)
        //     }).set_lazy_escape_len(90)
        ).unwrap();
    let duration = start.elapsed();
    println!("Time elapsed in encode() is: {:?}", duration);
    println!("Patch size SRC only: {}", patch.len());
    let f_2952_bytes = trgt.into_inner();

    let mut decode_sm = Vec::new();
    let start = Instant::now();
    let mut reader = Cursor::new(patch);
    src.rewind()?;
    smdiff_decoder::apply_patch(&mut reader,Some(&mut src) , &mut decode_sm).unwrap();
    let duration = start.elapsed();
    println!("Time elapsed in apply_patch() is: {:?}", duration);
    if decode_sm != f_2952_bytes{
        //find the first mismatch
        let mut i = 0;
        for (a,b) in decode_sm.iter().zip(f_2952_bytes.iter()){
            if a != b{

                eprintln!("{}", format!("Mismatch at index: {} | Decoded: {} | Target: {}", i, a, b).red());
                break;
            }
            i += 1;
        }
        //print len
        eprintln!("ERROR: Decoded: {} != Target: {}", decode_sm.len(), f_2952_bytes.len());
    }else{
        println!("{}","Patch SUCCESS!".green());
    }

    Ok(())
}