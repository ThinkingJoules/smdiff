
use std::fs;
use std::io::{Cursor, Read, Seek};
use std::path::Path;
use std::time::Instant;

use crate::DIR_PATH;

pub fn encode_test_gcc_2951_2952()-> Result<(), Box<dyn std::error::Error>> {
    let mut src = Cursor::new(Vec::new());
    let mut f_2952 = fs::File::open(&Path::new(DIR_PATH).join("gcc-2.95.2.tar"))?;
    let mut f_2952_bytes = Vec::new();
    f_2952.read_to_end(&mut f_2952_bytes).unwrap();
    let mut trgt = Cursor::new(f_2952_bytes);
    let mut patch = Vec::new();
    let start = Instant::now();
    smdiff_encoder::encode(&mut src, &mut trgt, &mut patch,true)?;
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
                println!("Mismatch at index: {} | Decoded: {} | Target: {}",i,a,b);
                break;
            }
            i += 1;
        }
        //print len
        println!("ERROR: Decoded: {} != Target: {}", decode_sm.len(), f_2952_bytes.len());
    }else{
        println!("Patch SUCCESS!");
    }

    let mut f_2951 = fs::File::open(&Path::new(DIR_PATH).join("gcc-2.95.1.tar"))?;
    let mut f_2951_bytes = Vec::new();
    f_2951.read_to_end(&mut f_2951_bytes).unwrap();
    let mut src = Cursor::new(f_2951_bytes);
    let mut trgt = Cursor::new(f_2952_bytes);
    let mut patch = Vec::new();
    let start = Instant::now();
    smdiff_encoder::encode(&mut src, &mut trgt, &mut patch,false)?;
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
                println!("Mismatch at index: {} | Decoded: {} | Target: {}",i,a,b);
                break;
            }
            i += 1;
        }
        //print len
        println!("ERROR: Decoded: {} != Target: {}", decode_sm.len(), f_2952_bytes.len());
    }else{
        println!("Patch SUCCESS!");
    }

    src.rewind()?;
    let mut trgt = Cursor::new(f_2952_bytes);
    let mut patch = Vec::new();
    let start = Instant::now();
    smdiff_encoder::encode(&mut src, &mut trgt, &mut patch,true)?;
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
                println!("Mismatch at index: {} | Decoded: {} | Target: {}",i,a,b);
                break;
            }
            i += 1;
        }
        //print len
        println!("ERROR: Decoded: {} != Target: {}", decode_sm.len(), f_2952_bytes.len());
    }else{
        println!("Patch SUCCESS!");
    }

    Ok(())
}

pub fn encode_test_gcc_2952_2953()-> Result<(), Box<dyn std::error::Error>> {
    let mut src = Cursor::new(Vec::new());
    let mut f_2953 = fs::File::open(&Path::new(DIR_PATH).join("gcc-2.95.3.tar"))?;
    let mut f_2953_bytes = Vec::new();
    f_2953.read_to_end(&mut f_2953_bytes).unwrap();
    let mut trgt = Cursor::new(f_2953_bytes);
    let mut patch = Vec::new();
    let start = Instant::now();
    smdiff_encoder::encode(&mut src, &mut trgt, &mut patch,true).unwrap();
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
                println!("Mismatch at index: {} | Decoded: {} | Target: {}",i,a,b);
                break;
            }
            i += 1;
        }
        //print len
        println!("ERROR: Decoded: {} != Target: {}", decode_sm.len(), f_2953_bytes.len());
    }else{
        println!("Patch SUCCESS!");
    }

    let mut f_2952 = fs::File::open(&Path::new(DIR_PATH).join("gcc-2.95.1.tar"))?;
    let mut f_2952_bytes = Vec::new();
    f_2952.read_to_end(&mut f_2952_bytes).unwrap();
    let mut src = Cursor::new(f_2952_bytes);
    let mut trgt = Cursor::new(f_2953_bytes);
    let mut patch = Vec::new();
    let start = Instant::now();
    smdiff_encoder::encode(&mut src, &mut trgt, &mut patch,false)?;
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
                println!("Mismatch at index: {} | Decoded: {} | Target: {}",i,a,b);
                break;
            }
            i += 1;
        }
        //print len
        println!("ERROR: Decoded: {} != Target: {}", decode_sm.len(), f_2953_bytes.len());
    }else{
        println!("Patch SUCCESS!");
    }

    src.rewind()?;
    let mut trgt = Cursor::new(f_2953_bytes);
    let mut patch = Vec::new();
    let start = Instant::now();
    smdiff_encoder::encode(&mut src, &mut trgt, &mut patch,true)?;
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
                println!("Mismatch at index: {} | Decoded: {} | Target: {}",i,a,b);
                break;
            }
            i += 1;
        }
        //print len
        println!("ERROR: Decoded: {} != Target: {}", decode_sm.len(), f_2953_bytes.len());
    }else{
        println!("Patch SUCCESS!");
    }

    Ok(())
}