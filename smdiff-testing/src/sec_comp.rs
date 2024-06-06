use smdiff_common::Format;
use smdiff_decoder::zstd;
use smdiff_encoder::{brotli::{BlockSize, BrotliEncoderOptions, CompressionMode, Quality, WindowSize}, EncoderConfig, SecondaryCompression, SrcMatcherConfig, TrgtMatcherConfig};

use std::{fs, io::Write, time::Duration};
use std::io::{Cursor, Read, Seek};
use std::path::Path;
use std::time::Instant;
use colored::*;
use crate::{encode_test_gcc_2951_2952, encode_test_gcc_2952_2953, DIR_PATH};

pub fn test_sec_comp_working() -> Result<(), Box<dyn std::error::Error>> {
    let config = EncoderConfig::default().format_segregated().set_sec_comp(SecondaryCompression::Smdiff(TrgtMatcherConfig::default()));
    println!("{:?}", config);
    encode_test_gcc_2951_2952(&config)?;
    encode_test_gcc_2952_2953(&config)?;
    let config = EncoderConfig::default().format_segregated().set_sec_comp(SecondaryCompression::Zstd { level: 1 });
    println!("{:?}", config);
    encode_test_gcc_2951_2952(&config)?;
    encode_test_gcc_2952_2953(&config)?;
    let mut options = BrotliEncoderOptions::new();
    options.quality(Quality::worst());
    options.block_size(BlockSize::worst());
    options.window_size(WindowSize::worst());
    options.mode(CompressionMode::Generic);
    let config = EncoderConfig::default().format_segregated().set_sec_comp(SecondaryCompression::Brotli { options });
    println!("{:?}", config);
    encode_test_gcc_2951_2952(&config)?;
    encode_test_gcc_2952_2953(&config)?;
    Ok(())
}

pub fn analyze_sec_comp_large_file_worst()-> Result<(), Box<dyn std::error::Error>> {
    let mut f_2952 = fs::File::open(&Path::new(DIR_PATH).join("gcc-2.95.2.tar"))?;
    let mut f_2952_bytes = Vec::new();
    f_2952.read_to_end(&mut f_2952_bytes).unwrap();
    let raw_size = f_2952_bytes.len();
    let mut values = Vec::new();

    let mut src = Cursor::new(Vec::new());
    let mut trgt = Cursor::new(f_2952_bytes);
    let mut patch = Vec::new();
    let start = Instant::now();
    smdiff_encoder::encode(None, &mut trgt, &mut patch,&EncoderConfig::default().no_match_src().set_match_target(TrgtMatcherConfig::comp_level(0)))?;
    let duration = start.elapsed();
    let f_2952_bytes = trgt.into_inner();
    let mut decode_sm = Cursor::new(Vec::new());
    let start = Instant::now();
    src.rewind()?;
    let mut reader = Cursor::new(&patch);
    smdiff_decoder::apply_patch(&mut reader,Some(&mut src) , &mut decode_sm).unwrap();
    let duration_2 = start.elapsed();
    let decode_sm = decode_sm.into_inner();
    assert_eq!(decode_sm, f_2952_bytes);
    values.push(("smdiff(trgt) (compress)",(duration,duration_2,patch.len())));

    let start = Instant::now();
    let mut zstd_2952 = smdiff_encoder::zstd::Encoder::new(Vec::new(), 1).unwrap();
    zstd_2952.write_all(&f_2952_bytes).unwrap();
    let zstd_comp_bytes = zstd_2952.finish().unwrap();
    let zstd_enc_dur = start.elapsed();
    let start = Instant::now();
    let mut zstd_2952 = smdiff_decoder::zstd::StreamingDecoder::new(std::io::Cursor::new(&zstd_comp_bytes)).unwrap();
    let mut zstd_decomp = Vec::new();
    zstd_2952.read_to_end(&mut zstd_decomp).unwrap();
    let zstd_dec_dur = start.elapsed();
    assert_eq!(f_2952_bytes, zstd_decomp);
    values.push(("zstd compress trgt",(zstd_enc_dur, zstd_dec_dur, zstd_comp_bytes.len())));

    let mut options = BrotliEncoderOptions::new();
    options.quality(Quality::worst());
    options.block_size(BlockSize::worst());
    options.window_size(WindowSize::worst());
    options.mode(CompressionMode::Generic);
    let start = Instant::now();
    let mut brotli_2952 = smdiff_encoder::brotli::CompressorWriter::with_encoder(options.build().unwrap(),Vec::new());
    brotli_2952.write_all(&f_2952_bytes).unwrap();
    let brotli_comp_bytes = brotli_2952.into_inner().unwrap();
    let brotli_enc_dur = start.elapsed();
    let start = Instant::now();
    let mut brotli_2952 = smdiff_decoder::brotli::DecompressorReader::new(std::io::Cursor::new(&brotli_comp_bytes));
    let mut brotli_decomp = Vec::new();
    brotli_2952.read_to_end(&mut brotli_decomp).unwrap();
    let brotli_dec_dur = start.elapsed();
    assert_eq!(f_2952_bytes, brotli_decomp);
    values.push(("brotli compress trgt",(brotli_enc_dur, brotli_dec_dur, brotli_comp_bytes.len())));




    let config = EncoderConfig::default().format_segregated().set_match_src(SrcMatcherConfig::comp_level(0)).set_match_target(TrgtMatcherConfig::comp_level(0));
    let r_none = sec_comp_gcc_2951_2952(&config)?;
    values.push(("Smdiff-d (no sec)",(r_none[0].0, r_none[0].1, r_none[0].2)));
    let config = config.clone().set_sec_comp(SecondaryCompression::Smdiff(TrgtMatcherConfig::comp_level(0)));
    let r_sm = sec_comp_gcc_2951_2952(&config)?;
    values.push(("Smdiff-d + Smdiff",(r_sm[0].0, r_sm[0].1, r_sm[0].2)));
    let config = config.clone().set_sec_comp(SecondaryCompression::Zstd { level: 1 });
    let r_z = sec_comp_gcc_2951_2952(&config)?;
    values.push(("Smdiff-d + zstd",(r_z[0].0, r_z[0].1, r_z[0].2)));
    let config = config.clone().set_sec_comp(SecondaryCompression::Brotli { options  });
    let r_b = sec_comp_gcc_2951_2952(&config)?;
    values.push(("Smdiff-d + brotli",(r_b[0].0, r_b[0].1, r_b[0].2)));
    values.push(("Smdiff-dcw (no sec)",(r_none[1].0, r_none[1].1, r_none[1].2)));
    values.push(("Smdiff-dcw + smdiff",(r_sm[1].0, r_sm[1].1, r_sm[1].2)));
    values.push(("Smdiff-dcw + zstd",(r_z[1].0, r_z[1].1, r_z[1].2)));
    values.push(("Smdiff-dcw + brotli",(r_b[1].0, r_b[1].1, r_b[1].2)));
    println!("Raw Target File Size: {}", raw_size);
    print_table_s(values, raw_size);
    Ok(())
}
pub fn analyze_sec_comp_large_file_best()-> Result<(), Box<dyn std::error::Error>> {
    let mut f_2952 = fs::File::open(&Path::new(DIR_PATH).join("gcc-2.95.2.tar"))?;
    let mut f_2952_bytes = Vec::new();
    f_2952.read_to_end(&mut f_2952_bytes).unwrap();
    let raw_size = f_2952_bytes.len();
    let mut values = Vec::new();

    let mut src = Cursor::new(Vec::new());
    let mut trgt = Cursor::new(f_2952_bytes);
    let mut patch = Vec::new();
    let start = Instant::now();
    smdiff_encoder::encode(None, &mut trgt, &mut patch,&EncoderConfig::default().no_match_src().set_match_target(TrgtMatcherConfig::comp_level(9)))?;
    let duration = start.elapsed();
    let f_2952_bytes = trgt.into_inner();
    let mut decode_sm = Cursor::new(Vec::new());
    let start = Instant::now();
    src.rewind()?;
    let mut reader = Cursor::new(&patch);
    smdiff_decoder::apply_patch(&mut reader,Some(&mut src) , &mut decode_sm).unwrap();
    let duration_2 = start.elapsed();
    let decode_sm = decode_sm.into_inner();
    assert_eq!(decode_sm, f_2952_bytes);
    values.push(("smdiff(trgt) (compress)",(duration,duration_2,patch.len())));

    let start = Instant::now();
    let mut zstd_2952 = smdiff_encoder::zstd::Encoder::new(Vec::new(), 22).unwrap();
    zstd_2952.write_all(&f_2952_bytes).unwrap();
    let zstd_comp_bytes = zstd_2952.finish().unwrap();
    let zstd_enc_dur = start.elapsed();
    let start = Instant::now();
    let mut zstd_2952 = smdiff_decoder::zstd::StreamingDecoder::new(std::io::Cursor::new(&zstd_comp_bytes)).unwrap();
    let mut zstd_decomp = Vec::new();
    zstd_2952.read_to_end(&mut zstd_decomp).unwrap();
    let zstd_dec_dur = start.elapsed();
    assert_eq!(f_2952_bytes, zstd_decomp);
    values.push(("zstd compress trgt",(zstd_enc_dur, zstd_dec_dur, zstd_comp_bytes.len())));

    let mut options = BrotliEncoderOptions::new();
    options.quality(Quality::best());
    options.block_size(BlockSize::best());
    options.window_size(WindowSize::best());
    options.mode(CompressionMode::Generic);
    let start = Instant::now();
    let mut brotli_2952 = smdiff_encoder::brotli::CompressorWriter::with_encoder(options.build().unwrap(),Vec::new());
    brotli_2952.write_all(&f_2952_bytes).unwrap();
    let brotli_comp_bytes = brotli_2952.into_inner().unwrap();
    let brotli_enc_dur = start.elapsed();
    let start = Instant::now();
    let mut brotli_2952 = smdiff_decoder::brotli::DecompressorReader::new(std::io::Cursor::new(&brotli_comp_bytes));
    let mut brotli_decomp = Vec::new();
    brotli_2952.read_to_end(&mut brotli_decomp).unwrap();
    let brotli_dec_dur = start.elapsed();
    assert_eq!(f_2952_bytes, brotli_decomp);
    values.push(("brotli compress trgt",(brotli_enc_dur, brotli_dec_dur, brotli_comp_bytes.len())));




    let config = EncoderConfig::default().format_segregated().set_match_src(SrcMatcherConfig::comp_level(9)).set_match_target(TrgtMatcherConfig::comp_level(9));
    let r_none = sec_comp_gcc_2951_2952(&config)?;
    values.push(("Smdiff-d (no sec)",(r_none[0].0, r_none[0].1, r_none[0].2)));
    let config = config.clone().set_sec_comp(SecondaryCompression::Smdiff(TrgtMatcherConfig::comp_level(9)));
    let r_sm = sec_comp_gcc_2951_2952(&config)?;
    values.push(("Smdiff-d + smdiff",(r_sm[0].0, r_sm[0].1, r_sm[0].2)));
    let config = config.clone().set_sec_comp(SecondaryCompression::Zstd { level: 22 });
    let r_z = sec_comp_gcc_2951_2952(&config)?;
    values.push(("Smdiff-d + zstd",(r_z[0].0, r_z[0].1, r_z[0].2)));
    let config = config.clone().set_sec_comp(SecondaryCompression::Brotli { options  });
    let r_b = sec_comp_gcc_2951_2952(&config)?;
    values.push(("Smdiff-d + brotli",(r_b[0].0, r_b[0].1, r_b[0].2)));
    values.push(("Smdiff-dcw (no sec)",(r_none[1].0, r_none[1].1, r_none[1].2)));
    values.push(("Smdiff-dcw + smdiff",(r_sm[1].0, r_sm[1].1, r_sm[1].2)));
    values.push(("Smdiff-dcw + zstd",(r_z[1].0, r_z[1].1, r_z[1].2)));
    values.push(("Smdiff-dcw + brotli",(r_b[1].0, r_b[1].1, r_b[1].2)));
    println!("Raw Target File Size: {}", raw_size);
    print_table_s(values, raw_size);
    Ok(())
}
fn sec_comp_gcc_2951_2952(config:&EncoderConfig)-> Result<[(Duration,Duration,usize);2], Box<dyn std::error::Error>> {
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
    smdiff_encoder::encode(Some(&mut src), &mut trgt, &mut patch,&config.clone().no_match_target())?;
    let duration_1 = start.elapsed();
    let size_1 = patch.len();
    let f_2952_bytes = trgt.into_inner();

    let mut decode_sm = Cursor::new(Vec::new());
    let start = Instant::now();
    src.rewind()?;
    let mut reader = Cursor::new(patch);
    smdiff_decoder::apply_patch(&mut reader,Some(&mut src) , &mut decode_sm).unwrap();
    let duration_2 = start.elapsed();
    let decode_sm = decode_sm.into_inner();
    assert_eq!(decode_sm, f_2952_bytes);

    src.rewind()?;
    let mut trgt = Cursor::new(f_2952_bytes);
    let mut patch = Vec::new();
    let start = Instant::now();
    smdiff_encoder::encode(Some(&mut src), &mut trgt, &mut patch,config)?;
    let duration_3 = start.elapsed();
    let f_2952_bytes = trgt.into_inner();
    let size_2 = patch.len();

    let mut decode_sm = Cursor::new(Vec::new());
    let start = Instant::now();
    src.rewind()?;
    let mut reader = Cursor::new(patch);
    smdiff_decoder::apply_patch(&mut reader,Some(&mut src) , &mut decode_sm).unwrap();
    let duration_4 = start.elapsed();
    let decode_sm = decode_sm.into_inner();
    assert_eq!(decode_sm, f_2952_bytes);

    Ok([(duration_1,duration_2,size_1),(duration_3,duration_4,size_2)])
}


pub fn analyze_sec_comp_sentence_best()-> Result<(), Box<dyn std::error::Error>> {
    let trgt_bytes = b"The playful brown fox jumps over the old dog dozing in the sun.".to_vec();
    let raw_size = trgt_bytes.len();
    let mut values = Vec::new();

    let mut src = Cursor::new(Vec::new());
    let mut trgt = Cursor::new(trgt_bytes);
    let mut patch = Vec::new();
    let start = Instant::now();
    smdiff_encoder::encode(None, &mut trgt, &mut patch,&EncoderConfig::default().no_match_src().set_match_target(TrgtMatcherConfig::comp_level(9)))?;
    let duration = start.elapsed();
    let trgt_bytes = trgt.into_inner();
    let mut decode_sm = Cursor::new(Vec::new());
    let start = Instant::now();
    src.rewind()?;
    let mut reader = Cursor::new(&patch);
    smdiff_decoder::apply_patch(&mut reader,Some(&mut src) , &mut decode_sm).unwrap();
    let duration_2 = start.elapsed();
    let decode_sm = decode_sm.into_inner();
    assert_eq!(decode_sm, trgt_bytes);
    values.push(("smdiff(trgt) (compress)",(duration,duration_2,patch.len())));

    let start = Instant::now();
    let mut zstd_trgt = smdiff_encoder::zstd::Encoder::new(Vec::new(), 22).unwrap();
    zstd_trgt.write_all(&trgt_bytes).unwrap();
    let zstd_comp_bytes = zstd_trgt.finish().unwrap();
    let zstd_enc_dur = start.elapsed();
    let start = Instant::now();
    let mut zstd_trgt = smdiff_decoder::zstd::StreamingDecoder::new(std::io::Cursor::new(&zstd_comp_bytes)).unwrap();
    let mut zstd_decomp = Vec::new();
    zstd_trgt.read_to_end(&mut zstd_decomp).unwrap();
    let zstd_dec_dur = start.elapsed();
    assert_eq!(trgt_bytes, zstd_decomp);
    values.push(("zstd compress trgt",(zstd_enc_dur, zstd_dec_dur, zstd_comp_bytes.len())));

    let mut options = BrotliEncoderOptions::new();
    options.quality(Quality::best());
    options.block_size(BlockSize::best());
    options.window_size(WindowSize::best());
    options.mode(CompressionMode::Generic);
    let start = Instant::now();
    let mut brotli_trgt = smdiff_encoder::brotli::CompressorWriter::with_encoder(options.build().unwrap(),Vec::new());
    brotli_trgt.write_all(&trgt_bytes).unwrap();
    let brotli_comp_bytes = brotli_trgt.into_inner().unwrap();
    let brotli_enc_dur = start.elapsed();
    let start = Instant::now();
    let mut brotli_trgt = smdiff_decoder::brotli::DecompressorReader::new(std::io::Cursor::new(&brotli_comp_bytes));
    let mut brotli_decomp = Vec::new();
    brotli_trgt.read_to_end(&mut brotli_decomp).unwrap();
    let brotli_dec_dur = start.elapsed();
    assert_eq!(trgt_bytes, brotli_decomp);
    values.push(("brotli compress trgt",(brotli_enc_dur, brotli_dec_dur, brotli_comp_bytes.len())));




    let config = EncoderConfig::default().set_match_src(SrcMatcherConfig::comp_level(9)).set_match_target(TrgtMatcherConfig::comp_level(9));
    let r_none = sec_comp_sentence(&config)?;
    values.push(("smdiff(src) (no sec)",(r_none[0].0, r_none[0].1, r_none[0].2)));
    let config = config.clone().set_sec_comp(SecondaryCompression::Smdiff(TrgtMatcherConfig::comp_level(9)));
    let r_sm = sec_comp_sentence(&config)?;
    values.push(("smdiff(src) + smdiff",(r_sm[0].0, r_sm[0].1, r_sm[0].2)));
    let config = config.clone().set_sec_comp(SecondaryCompression::Zstd { level: 22 });
    let r_z = sec_comp_sentence(&config)?;
    values.push(("smdiff(src) + zstd",(r_z[0].0, r_z[0].1, r_z[0].2)));
    let config = config.clone().set_sec_comp(SecondaryCompression::Brotli { options  });
    let r_b = sec_comp_sentence(&config)?;
    values.push(("smdiff(src) + brotli",(r_b[0].0, r_b[0].1, r_b[0].2)));
    values.push(("smdiff(src+trgt) (no sec)",(r_none[1].0, r_none[1].1, r_none[1].2)));
    values.push(("smdiff(src+trgt) + smdiff",(r_sm[1].0, r_sm[1].1, r_sm[1].2)));
    values.push(("smdiff(src+trgt) + zstd",(r_z[1].0, r_z[1].1, r_z[1].2)));
    values.push(("smdiff(src+trgt) + brotli",(r_b[1].0, r_b[1].1, r_b[1].2)));
    println!("Raw Target File Size: {}", raw_size);
    print_table_ms(values, raw_size);
    Ok(())
}
fn sec_comp_sentence(config:&EncoderConfig)-> Result<[(Duration,Duration,usize);2], Box<dyn std::error::Error>> {
    let src_bytes = b"the quick brown fox jumps over the lazy dog".to_vec();
    let trgt_bytes = b"The playful brown fox jumps over the old dog dozing in the sun.".to_vec();
    let mut src = Cursor::new(src_bytes);
    let mut trgt = Cursor::new(trgt_bytes);
    let mut patch = Vec::new();
    let start = Instant::now();
    smdiff_encoder::encode(Some(&mut src), &mut trgt, &mut patch,&config.clone().no_match_target())?;
    let duration_1 = start.elapsed();
    let size_1 = patch.len();
    let trgt_bytes = trgt.into_inner();

    let mut decode_sm = Cursor::new(Vec::new());
    let start = Instant::now();
    let mut reader = Cursor::new(patch);
    smdiff_decoder::apply_patch(&mut reader,Some(&mut src) , &mut decode_sm).unwrap();
    let duration_2 = start.elapsed();
    let decode_sm = decode_sm.into_inner();
    assert_eq!(decode_sm, trgt_bytes);

    src.rewind()?;
    let mut trgt = Cursor::new(trgt_bytes);
    let mut patch = Vec::new();
    let start = Instant::now();
    smdiff_encoder::encode(Some(&mut src), &mut trgt, &mut patch,config)?;
    let duration_3 = start.elapsed();
    let trgt_bytes = trgt.into_inner();
    let size_2 = patch.len();

    let mut decode_sm = Cursor::new(Vec::new());
    let start = Instant::now();
    src.rewind()?;
    let mut reader = Cursor::new(patch);
    smdiff_decoder::apply_patch(&mut reader,Some(&mut src) , &mut decode_sm).unwrap();
    let duration_4 = start.elapsed();
    let decode_sm = decode_sm.into_inner();
    assert_eq!(decode_sm, trgt_bytes);

    Ok([(duration_1,duration_2,size_1),(duration_3,duration_4,size_2)])
}

fn print_table_s(data: Vec<(&str, (Duration, Duration, usize))>, raw_size: usize) {
    // Header
    println!("{:<30} | {:>12} | {:>12} | {:>12} | {:>14}",
             "Label", "Encode (s)", "Decode (s)", "Encoded Size", "Compression (%)");

    println!("{:-<30}-+-{:-<12}-+-{:-<12}-+-{:-<12}-+---{:-<14}", "", "", "", "", "");

    // Rows
    for (label, (encode_time, decode_time, encoded_size)) in data {
        let encode_secs = encode_time.as_secs_f64();
        let decode_secs = decode_time.as_secs_f64();
        let compression_percent = encoded_size as f64 / raw_size as f64 * 100.0;

        println!("{:<30} | {:>12.3} | {:>12.3} | {:>12} | {:>14.3}",
                 label, encode_secs, decode_secs, encoded_size, compression_percent);
    }
}
fn print_table_ms(data: Vec<(&str, (Duration, Duration, usize))>, raw_size: usize) {
    // Header
    println!("{:<30} | {:>12} | {:>12} | {:>12} | {:>14}",
             "Label", "Encode (us)", "Decode (us)", "Encoded Size", "Compression (%)");

    println!("{:-<30}-+-{:-<12}-+-{:-<12}-+-{:-<12}-+---{:-<14}", "", "", "", "", "");

    // Rows
    for (label, (encode_time, decode_time, encoded_size)) in data {
        let encode_secs = encode_time.as_micros();
        let decode_secs = decode_time.as_micros();
        let compression_percent = encoded_size as f64 / raw_size as f64 * 100.0;

        println!("{:<30} | {:>12} | {:>12} | {:>12} | {:>14.3}",
                 label, encode_secs, decode_secs, encoded_size, compression_percent);
    }
}
// fn print_table(data: Vec<(&str, (Duration, Duration, usize))>, raw_size: usize) {
//     // Determine maximum column widths for formatting
//     let max_label_width = data.iter().map(|&(label, _)| label.len()).max().unwrap_or(0);
//     let max_duration_width = 20; // Adjust for desired format

//     // Header
//     println!("{:<width$} | {:>width$} | {:>width$} | {:>width$} | {:>10}",
//              "Label", "Encode Time", "Decode Time", "Encoded Size", "Compression Ratio",
//              width = max_label_width);

//     println!("{:-<width$}-+-{:-<width$}-+-{:-<width$}-+-{:-<10}-+---", "", "", "", "",
//              width = max_label_width);

//     // Rows
//     for (label, (encode_time, decode_time, encoded_size)) in data {
//         let compression_ratio = encoded_size as f64/raw_size as f64 ;

//         println!("{:<width$} | {:>width$} | {:>width$} | {:>width$} | {:>10.1}",
//                  label, format!("{:?}", encode_time), format!("{:?}", decode_time), encoded_size, compression_ratio,
//                  width = max_label_width);
//     }
// }
