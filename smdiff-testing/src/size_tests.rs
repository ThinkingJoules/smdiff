use std::collections::HashMap;
use std::fs;
use std::io::{Cursor, Read, Seek};
use std::path::Path;
use std::time::Instant;

use smdiff_encoder::{EncoderConfig, SrcMatcherConfig, TrgtMatcherConfig};
use smdiff_reader::SectionIterator;

use crate::{Stats, DIR_PATH};

use colored::*;

pub fn encode_test_large()-> Result<(), Box<dyn std::error::Error>> {
    let mut f_317 = fs::File::open(&Path::new(DIR_PATH).join("317.iso"))?;
    let mut src = Vec::new();
    f_317.read_to_end(&mut src).unwrap();
    let mut src = Cursor::new(src);
    let mut f_318 = fs::File::open(&Path::new(DIR_PATH).join("318.iso"))?;
    //open 318 and read to end
    let mut target = Vec::new();
    f_318.read_to_end(&mut target).unwrap();
    let mut trgt = Cursor::new(target);
    let mut patch = Vec::new();
    let start = Instant::now();
    smdiff_encoder::encode(&mut src, &mut trgt, &mut patch,&EncoderConfig::default().format_segregated().set_match_src(SrcMatcherConfig::comp_level(9)))?;
    //smdiff_encoder::encode(&mut Cursor::new(Vec::new()), &mut trgt, &mut patch,true)?;
    let duration = start.elapsed();
    println!("Time elapsed in encode() is: {:?}", duration);
    println!("Patch size: {}", patch.len());

    let mut sec = SectionIterator::new(Cursor::new(patch));
    while let Some(res) = sec.next_borrowed(){
        let (ops,_) = res?;
        let mut s_copy_lens = HashMap::new();
        let mut t_copy_lens = HashMap::new();
        let mut stats = Stats::new();
        for op in ops {
            let len = op.oal();
            match op {
                smdiff_reader::Op::Add(_) => {
                    stats.add(len as usize);
                }
                smdiff_reader::Op::Run(_) => {
                    stats.run(len as usize);
                }
                smdiff_reader::Op::Copy(copy) => {
                    if matches!(copy.src, smdiff_common::CopySrc::Dict){
                        stats.copy_s(len as usize);
                        *s_copy_lens.entry(len).or_insert(0) += 1;
                    }else{
                        stats.copy_t(len as usize);
                        *t_copy_lens.entry(len).or_insert(0) += 1;
                    }
                }
            }
        }
        //println!("{:?}", stats);
        //collect the s_copy_lens and sort ascending by key, then print out all the keys and values
        let mut s_copy_lens: Vec<_> = s_copy_lens.into_iter().collect();
        s_copy_lens.sort_by_key(|k| k.0);
        // println!("S Copy Lens:");
        // for (k,v) in s_copy_lens.iter().take(16) {
        //     //println!("{}: {}", k, v);
        // }
    }

    let mut decode_sm = Vec::new();
    let start = Instant::now();
    src.rewind()?;
    let mut reader = sec.into_inner();
    reader.rewind()?;
    let target = trgt.into_inner();
    smdiff_decoder::apply_patch(&mut reader,Some(&mut src) , &mut decode_sm).unwrap();
    let duration = start.elapsed();
    println!("Time elapsed in apply_patch() is: {:?}", duration);

    if decode_sm != target{
        //find the first mismatch
        let mut i = 0;
        for (a,b) in decode_sm.iter().zip(target.iter()){
            if a != b{
                eprintln!("{}", format!("Mismatch at index: {} | Decoded: {} | Target: {}", i, a, b).red());
                break;
            }
            i += 1;
        }
        //print len
        eprintln!("ERROR: Decoded: {} != Target: {}", decode_sm.len(), target.len());
    }else{
        println!("{}","Patch SUCCESS!".green());
    }
    Ok(())
}

pub fn encode_test_small()-> Result<(), Box<dyn std::error::Error>> {
    let src = generate_rand_vec(15_000_000, [0u8;32]);
    let mut src = Cursor::new(src);
    let target = generate_rand_vec(15_500_000, [1u8;32]);
    let mut trgt = Cursor::new(target);
    let mut patch = Vec::new();
    let start = Instant::now();
    smdiff_encoder::encode(&mut src, &mut trgt, &mut patch,&EncoderConfig::default().set_match_src(SrcMatcherConfig::comp_level(9)).set_match_target(TrgtMatcherConfig::comp_level(9)))?;
    //smdiff_encoder::encode(&mut Cursor::new(Vec::new()), &mut trgt, &mut patch,true)?;
    let duration = start.elapsed();
    println!("Time elapsed in encode() is: {:?}", duration);
    println!("Patch size: {}", patch.len());

    let mut sec = SectionIterator::new(Cursor::new(patch));
    while let Some(res) = sec.next_borrowed(){
        let (ops,_) = res?;
        let mut s_copy_lens = HashMap::new();
        let mut t_copy_lens = HashMap::new();
        let mut stats = Stats::new();
        for op in ops {
            let len = op.oal();
            match op {
                smdiff_reader::Op::Add(_) => {
                    stats.add(len as usize);
                }
                smdiff_reader::Op::Run(_) => {
                    stats.run(len as usize);
                }
                smdiff_reader::Op::Copy(copy) => {
                    if matches!(copy.src, smdiff_common::CopySrc::Dict){
                        stats.copy_s(len as usize);
                        *s_copy_lens.entry(len).or_insert(0) += 1;
                    }else{
                        stats.copy_t(len as usize);
                        *t_copy_lens.entry(len).or_insert(0) += 1;
                    }
                }
            }
        }
        println!("{:?}", stats);
        //collect the s_copy_lens and sort ascending by key, then print out all the keys and values
        let mut s_copy_lens: Vec<_> = s_copy_lens.into_iter().collect();
        s_copy_lens.sort_by_key(|k| k.0);
        println!("S Copy Lens:");
        for (k,v) in s_copy_lens.iter() {
            println!("{}: {}", k, v);
        }
    }

    let mut decode_sm = Vec::new();
    let start = Instant::now();
    src.rewind()?;
    let mut reader = sec.into_inner();
    reader.rewind()?;
    let target = trgt.into_inner();
    smdiff_decoder::apply_patch(&mut reader,Some(&mut src) , &mut decode_sm).unwrap();
    let duration = start.elapsed();
    println!("Time elapsed in apply_patch() is: {:?}", duration);

    if decode_sm != target{
        //find the first mismatch
        let mut i = 0;
        for (a,b) in decode_sm.iter().zip(target.iter()){
            if a != b{
                eprintln!("{}", format!("Mismatch at index: {} | Decoded: {} | Target: {}", i, a, b).red());
                break;
            }
            i += 1;
        }
        //print len
        eprintln!("ERROR: Decoded: {} != Target: {}", decode_sm.len(), target.len());
    }else{
        println!("{}","Patch SUCCESS!".green());
    }
    Ok(())
}

pub fn encode_test_micro()-> Result<(), Box<dyn std::error::Error>> {
    let src = generate_rand_vec(1_000, [0u8;32]);
    let mut src = Cursor::new(src);
    let target = generate_rand_vec(1_500, [0u8;32]);
    let mut trgt = Cursor::new(target);
    let mut patch = Vec::new();
    let start = Instant::now();
    smdiff_encoder::encode(&mut src, &mut trgt, &mut patch,&EncoderConfig::default().set_match_src(SrcMatcherConfig::comp_level(9)).set_match_target(TrgtMatcherConfig::comp_level(9)))?;
    //smdiff_encoder::encode(&mut Cursor::new(Vec::new()), &mut trgt, &mut patch,true)?;
    let duration = start.elapsed();
    println!("Time elapsed in encode() is: {:?}", duration);
    println!("Patch size: {}", patch.len());

    let mut sec = SectionIterator::new(Cursor::new(patch));
    while let Some(res) = sec.next_borrowed(){
        let (ops,_) = res?;
        let mut s_copy_lens = HashMap::new();
        let mut t_copy_lens = HashMap::new();
        let mut stats = Stats::new();
        for op in ops {
            let len = op.oal();
            match op {
                smdiff_reader::Op::Add(_) => {
                    stats.add(len as usize);
                }
                smdiff_reader::Op::Run(_) => {
                    stats.run(len as usize);
                }
                smdiff_reader::Op::Copy(copy) => {
                    if matches!(copy.src, smdiff_common::CopySrc::Dict){
                        stats.copy_s(len as usize);
                        *s_copy_lens.entry(len).or_insert(0) += 1;
                    }else{
                        stats.copy_t(len as usize);
                        *t_copy_lens.entry(len).or_insert(0) += 1;
                    }
                }
            }
        }
        println!("{:?}", stats);
        //collect the s_copy_lens and sort ascending by key, then print out all the keys and values
        let mut s_copy_lens: Vec<_> = s_copy_lens.into_iter().collect();
        s_copy_lens.sort_by_key(|k| k.0);
        println!("S Copy Lens:");
        for (k,v) in s_copy_lens.iter() {
            println!("{}: {}", k, v);
        }
    }

    let mut decode_sm = Vec::new();
    let start = Instant::now();
    src.rewind()?;
    let mut reader = sec.into_inner();
    reader.rewind()?;
    let target = trgt.into_inner();
    smdiff_decoder::apply_patch(&mut reader,Some(&mut src) , &mut decode_sm).unwrap();
    let duration = start.elapsed();
    println!("Time elapsed in apply_patch() is: {:?}", duration);

    if decode_sm != target{
        //find the first mismatch
        let mut i = 0;
        for (a,b) in decode_sm.iter().zip(target.iter()){
            if a != b{
                eprintln!("{}", format!("Mismatch at index: {} | Decoded: {} | Target: {}", i, a, b).red());
                break;
            }
            i += 1;
        }
        //print len
        eprintln!("ERROR: Decoded: {} != Target: {}", decode_sm.len(), target.len());
    }else{
        println!("{}","Patch SUCCESS!".green());
    }
    Ok(())
}
pub fn generate_rand_vec(size:usize,seed:[u8;32]) -> Vec<u8> {
    use rand::SeedableRng;
    use rand::Rng;
    let mut rng = rand::rngs::StdRng::from_seed(seed);
    (0..size).map(|_| rng.gen()).collect()

}