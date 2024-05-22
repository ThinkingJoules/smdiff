use std::collections::HashMap;
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;
use std::time::Instant;

use smdiff_reader::{read_ops, read_section, SectionReader};

use crate::{Stats, DIR_PATH};
use colored::*;

pub fn vc_analysis()-> Result<(), Box<dyn std::error::Error>> {
    let mut file = fs::File::open(&Path::new(DIR_PATH).join("patch_a.ovcd.vcdiff"))?;
    let mut patch_a = Vec::new();
    file.read_to_end(&mut patch_a)?;
    let mut converted_a = Vec::new();
    let mut reader = Cursor::new(patch_a);
    let start = Instant::now();
    smdiff_vcdiff::convert_vcdiff_to_smdiff(&mut reader, &mut converted_a)?;
    let sm_patch = Cursor::new(converted_a);
    let mut reader = SectionReader::new(sm_patch);
    while let Ok(Some((ops,_))) = reader.next(){
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
        for (k,v) in s_copy_lens {
            println!("{}: {}", k, v);
        }
        //pause for 10 sec
        std::thread::sleep(std::time::Duration::from_secs(10));
    }


    Ok(())
}
pub fn vc_to_sm_test()-> Result<(), Box<dyn std::error::Error>> {
    let mut file = fs::File::open(&Path::new(DIR_PATH).join("patch_a.ovcd.vcdiff")).unwrap();
    let mut patch_a = Vec::new();
    file.read_to_end(&mut patch_a)?;
    let mut converted_a = Vec::new();
    let mut reader = Cursor::new(patch_a);
    let start = Instant::now();
    smdiff_vcdiff::convert_vcdiff_to_smdiff(&mut reader, &mut converted_a)?;
    let duration = start.elapsed();
    println!("Time elapsed in convert_vcdiff_to_smdiff() is: {:?}", duration);
    //compare lengths and print results
    println!("Original: {}\nConverted: {}", reader.get_ref().len(), converted_a.len());
    //print difference in length
    println!("Difference: {}", reader.get_ref().len() as i64 - converted_a.len() as i64);
    //print converted as % size of original
    println!("Converted as % of original: {}", (converted_a.len() as f64 / reader.get_ref().len() as f64) * 100.0);

    println!("Original: {}\nConverted: {}", reader.get_ref().len(), converted_a.len());

    //read 317.iso
    let mut file = fs::File::open(&Path::new(DIR_PATH).join("317.iso")).unwrap();
    let mut src = Vec::new();
    file.read_to_end(&mut src)?;
    let mut src = Cursor::new(src);
    let mut decode_sm = Vec::new();
    let mut reader = Cursor::new(converted_a);
    let start = Instant::now();
    smdiff_decoder::apply_patch(&mut reader,Some(&mut src) , &mut decode_sm)?;
    let duration = start.elapsed();
    //open 318 and read to end
    let mut file = fs::File::open(&Path::new(DIR_PATH).join("318.iso")).unwrap();
    let mut target = Vec::new();
    file.read_to_end(&mut target)?;
    println!("Time elapsed in apply_patch() is: {:?}", duration);
    if decode_sm != target{
        //print len
        eprintln!("ERROR: Decoded: {} != Target: {}", decode_sm.len(), target.len());
    }else{
        println!("{}","Translate and apply SUCCESS!".green());
    }


    let mut file = fs::File::open(&Path::new(DIR_PATH).join("patch_b.ovcd.vcdiff")).unwrap();
    let mut patch_b = Vec::new();
    file.read_to_end(&mut patch_b)?;
    let mut converted_b = Vec::new();
    let mut reader = Cursor::new(patch_b);
    let start = Instant::now();
    smdiff_vcdiff::convert_vcdiff_to_smdiff(&mut reader, &mut converted_b)?;
    let duration = start.elapsed();
    println!("Time elapsed in convert_vcdiff_to_smdiff() is: {:?}", duration);
    //compare lengths and print results
    println!("Original: {}\nConverted: {}", reader.get_ref().len(), converted_b.len());
    //print difference in length
    println!("Difference: {}", reader.get_ref().len() as i64 - converted_b.len() as i64);
    //print converted as % size of original
    println!("Converted as % of original: {}", (converted_b.len() as f64 / reader.get_ref().len() as f64) * 100.0);

    let vcdiff_small = vec![
        214,195,196,0, //magic
        0, //hdr_indicator
        1, //win_indicator Src
        11, //SSS
        1, //SSP
        14, //delta window size
        7, //target window size
        0, //delta indicator
        1, //length of data for ADDs and RUN/
        5, //length of instructions and size
        3, //length of addr
        72, //data section 'H'
        163, //ADD1 COPY4_mode0
        19, //COPY0_mode0
        1, //..size
        19, //COPY0_mode0
        1, //..size
        0, //addr 0
        10, //addr 1
        4, //addr 2
        2, //win_indicator VCD_TARGET
        7, //SSS
        0, //SSP
        14, //delta window size
        14, //target window size
        0, //delta indicator
        1, //length of data for ADDs and RUN/
        5, //length of instructions and size
        3, //length of addr
        46, //data section '.'
        23, //COPY0_mode0 noop
        28, //..size
        2, //Add1 NOOP
        19, //COPY0_mode0
        1, //..size
        0, //addr 0
        7, //addr 1
        13, //addr 2
    ];
    let mut converted_c = Vec::new();
    let mut reader = Cursor::new(vcdiff_small);
    let start = Instant::now();
    smdiff_vcdiff::convert_vcdiff_to_smdiff(&mut reader, &mut converted_c)?;
    let duration = start.elapsed();
    println!("Time elapsed in convert_vcdiff_to_smdiff() is: {:?}", duration);
    //compare lengths and print results
    println!("Original: {}\nConverted: {}", reader.get_ref().len(), converted_c.len());
    //print difference in length
    println!("Difference: {}", reader.get_ref().len() as i64 - converted_c.len() as i64);
    //print converted as % size of original
    println!("Converted as % of original: {}", (converted_c.len() as f64 / reader.get_ref().len() as f64) * 100.0);

    println!("{:?}", converted_c);
    let mut reader = Cursor::new(converted_c);
    let header = smdiff_reader::read_section_header(&mut reader)?;
    let ops = read_ops(&mut reader,&header)?;
    for op in ops {
        println!("{:?}", op);
    }
    println!("output_size {:?}", header.output_size);
    Ok(())
}
