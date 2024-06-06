//! A writer for an smdiff section.
//! This handles writing the header and the operations, and optionally secondary compression.
use std::io::Write;

use crate::{encode, EncoderConfig, SecondaryCompression};

use smdiff_common::AddOp;
use smdiff_writer::{write_ops, write_section_header};


/// Writes a section to a writer, with secondary compression if requested.
pub fn section_writer<W:Write,A:AddOp>(
    sec_comp: &Option<SecondaryCompression>,
    mut header: smdiff_common::SectionHeader,
    writer: &mut W,
    seg_ops: &[smdiff_common::Op<A>],
    mut sec_data_buffer: &mut Vec<u8>)
-> std::io::Result<()> {
    Ok(if sec_comp.is_some() {
        let comp = sec_comp.clone().unwrap();
        header.compression_algo = comp.algo_value();
        //dbg!(&header);
        write_section_header(&header, writer)?;
        write_ops(seg_ops,&header,sec_data_buffer)?;
        match comp{
            SecondaryCompression::Smdiff (config) => {
                let mut crsr = std::io::Cursor::new(sec_data_buffer);
                let inner_config = EncoderConfig::default().no_match_src().set_match_target(config);
                encode(None, &mut crsr, writer, &inner_config)?;
                sec_data_buffer = crsr.into_inner();
            },
            SecondaryCompression::Zstd { level } => {
                let mut a = ::zstd::Encoder::new(writer, level)?;
                a.set_pledged_src_size(Some(sec_data_buffer.len() as u64))?;
                a.include_contentsize(true)?;
                a.write_all(&*sec_data_buffer)?;
                a.finish()?;
            },
            SecondaryCompression::Brotli { mut options }=> {
                options.size_hint(sec_data_buffer.len() as u32);
                let mut a = ::brotlic::CompressorWriter::with_encoder(options.build().unwrap(), writer);
                a.write_all(&*sec_data_buffer)?;
                a.into_inner()?;
            },
        }
        sec_data_buffer.clear();
    }else{
        write_section_header(&header, writer)?;
        write_ops(seg_ops,&header,writer)?;
    })
}