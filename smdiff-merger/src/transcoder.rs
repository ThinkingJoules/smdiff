use std::io::{Read, Seek, Write};

use smdiff_common::{CopySrc, MAX_INST_SIZE, MAX_RUN_LEN};
use smdiff_reader::{Add, Op};

use crate::extract_patch_instructions;

enum InnerOp{
    Add(Vec<u8>),
    Copy(smdiff_common::Copy), //we can't do anything with copy operations.
    Run{byte: u8, len: usize, output_start_pos: usize},
}

/// Transcodes a patch from one format to another.
/// This function also attempts to optimize the patch operations while it is transcoding. The routine is as follows:
/// * It groups adjacent Add operations together, and joins adjacent Run operations.
/// * It then makes sure that Add operations are no larger than the maximum instruction size.
/// * The Run operations are also optimized to be no larger than the maximum run length.
/// * If a Run is long enough, it this fn will encode them using progressively larger Copy operations, until the whole run length is covered.
///
///
/// # Arguments
/// * `input` - The input patch to transcode.
/// * `output` - The writer to write the transcoded patch to.
/// * `format` - The format to transcode the patch to.
/// * `sec_comp` - The secondary compression to use, if any.
/// * `output_segment_size` - The size of the output segments.
///
/// # Errors
/// Returns an error if there was an issue reading from the input or writing to the output.
/// Can also error if there are any invalid operations in the input patch.
pub fn transcode<R,W>(
    input: &mut R,
    output: &mut W,
    format: smdiff_common::Format,
    sec_comp: Option<smdiff_encoder::SecondaryCompression>,
    output_segment_size: usize,
) -> std::io::Result<()>
where
    R: Read+Seek,
    W: Write,
{
    let ops = optimize_and_convert_ops(read_ops_from_patch(input)?);
    let mut win_data = Vec::new();
    for (seg_ops, mut header) in crate::make_sections(&ops, output_segment_size) {
        header.format = format;
        smdiff_encoder::writer::section_writer(
            &sec_comp,
            header,
            output,
            &seg_ops,
            &mut win_data,
        )?;
    }

    Ok(())
}

fn read_ops_from_patch<R:Read+Seek>(input: &mut R) -> std::io::Result<Vec<InnerOp>> {
    let inners:Vec<InnerOp> = extract_patch_instructions(input)?.0.into_iter().map(|(out_addr,op)|
        match op {
            Op::Add(a) => InnerOp::Add(a.bytes.to_vec()),
            Op::Copy(c) => InnerOp::Copy(c),
            Op::Run(r) => InnerOp::Run{byte: r.byte, len: r.len as usize, output_start_pos: out_addr as usize},
        }
    ).collect();
    Ok(inners)
}

fn optimize_and_convert_ops(mut ops: Vec<InnerOp>)->Vec<Op> {
    join_adjacent_adds(&mut ops);
    join_adjacent_runs(&mut ops);
    let mut out_ops = Vec::with_capacity(ops.len());
    for iop in ops {
        match iop {
            InnerOp::Add(bytes) if !bytes.is_empty() => make_add_ops(bytes, &mut out_ops),
            InnerOp::Copy(copy) => out_ops.push(Op::Copy(copy)),
            InnerOp::Run{byte, len,output_start_pos} if len>0 => make_run_ops(byte, len, output_start_pos,&mut out_ops),
            _ => ()
        }
    }
    out_ops
}

fn join_adjacent_adds(ops: &mut Vec<InnerOp>) {
    let mut i = 0;
    while i < ops.len() - 1 {
        let (left, right) = ops.split_at_mut(i + 1);
        if let (Some(InnerOp::Add(first)), Some(InnerOp::Add(second))) = (left.last_mut(), right.first_mut()) {
            if !second.is_empty() && first.len() < MAX_INST_SIZE as usize {
                first.append(second);
            }
            i += 1;
        }
        i += 1;
    }
}

fn join_adjacent_runs(ops: &mut Vec<InnerOp>) {
    let mut i = 0;
    while i < ops.len() - 1 {
        let (left, right) = ops.split_at_mut(i + 1);
        if let (Some(InnerOp::Run { byte: byte1, len: len1,.. }), Some(InnerOp::Run { byte: byte2, len: len2, .. })) = (left.last_mut(), right.first_mut()) {
            if byte1 == byte2 {
                *len1 += *len2;
                *len2 = 0;
            }
            i += 1;
        }
        i += 1;
    }
}

pub fn make_add_ops<'a>(bytes: Vec<u8>, output: &mut Vec<Op>){
    let total_len = bytes.len();
    if total_len == 0{
        return;
    }else if total_len == 1{//emit a run of len 1
        output.push(Op::Run(smdiff_common::Run{len: 1, byte: bytes[0]}));
        return;
    }else if total_len == MAX_INST_SIZE as usize{ //no op if this is already the max size
        output.push(Op::Add(Add{bytes}));
        return;
    }//else we are 2..MAX_INST_SIZE || MAX_INST_SIZE+1..?
    let mut processed = 0;
    loop{
        if processed == total_len{
            break;
        }
        let to_add = total_len - processed;
        let chunk_size = to_add.min(MAX_INST_SIZE as usize);
        let op = Add{bytes: bytes[processed..processed+chunk_size].to_vec()};
        processed += chunk_size;
        output.push(Op::Add(op));
    }
}

// This is directly lifted from the smdiff-encoder crate, so check there for more details/tests
const RUN_LIMIT: usize = (MAX_RUN_LEN as usize) * 6;
const COPY_LIMIT: usize = RUN_LIMIT/2;
fn make_run_ops(byte:u8, len:usize, run_start_pos:usize, output: &mut Vec<Op>){
    if len < RUN_LIMIT {
        let mut processed = 0;
        while processed < len {
            let remaining = len - processed;
            let chunk_size = (MAX_RUN_LEN as usize).min(remaining);
            let op = Op::Run(smdiff_common::Run{byte, len: chunk_size as u8});
            output.push(op);
            processed += chunk_size;
        };
    }else{
        //we can use one or more copies on 3 runs.
        //we need to emit the three runs, then make the copies from the stack
        output.extend(std::iter::repeat_with(|| Op::Run(smdiff_common::Run{byte, len: MAX_RUN_LEN})).take(3));

        let copy_bytes = len - COPY_LIMIT;
        let mut processed = 0;
        let mut max_copy_size = COPY_LIMIT;
        while processed < copy_bytes{
            let copy_size = max_copy_size.min(copy_bytes - processed).min(MAX_INST_SIZE);
            let op = Op::Copy(smdiff_common::Copy{src: CopySrc::Output, addr: run_start_pos as u64, len: copy_size as u16});
            output.push(op);
            processed += copy_size;
            max_copy_size += copy_size;
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use smdiff_common::{Copy, CopySrc, Run, Format};
    use smdiff_encoder::TrgtMatcherConfig;
    use smdiff_reader::{Add, Op};
    use std::io::Cursor;


    fn make_ops() -> Vec<Op> {
        [
            Op::Add(Add{bytes: b"ABC".to_vec()}),
            Op::Add(Add{bytes: b"DE".to_vec()}),
            Op::Run(Run{byte: b'X', len: 3}),
            Op::Run(Run{ byte: b'X', len: 2}),
            Op::Copy(Copy { src: CopySrc::Dict, addr: 0, len: 3 }),
        ].to_vec()
    }
    fn make_correct_ops() -> Vec<Op> {
        [
            Op::Add(Add{bytes: b"ABCDE".to_vec()}),
            Op::Run(Run{byte: b'X', len: 5}),
            Op::Copy(Copy { src: CopySrc::Dict, addr: 0, len: 3 }),
        ].to_vec()
    }

    fn create_test_patch() -> Cursor<Vec<u8>> {
        let mut sink = Cursor::new(Vec::new());
        for (ops, mut header) in crate::make_sections(&make_ops(), smdiff_common::MAX_WIN_SIZE) {
            header.format = Format::Segregated;
            smdiff_encoder::writer::section_writer(&Some(smdiff_encoder::SecondaryCompression::Smdiff(TrgtMatcherConfig::default())), header, &mut sink, ops, &mut Vec::new()).unwrap();
        }
        sink.rewind().unwrap();
        sink
    }

    #[test]
    fn test_optimize_and_convert_ops() {
        let inner_ops = vec![
            InnerOp::Add(b"ABC".to_vec()),
            InnerOp::Add(b"DE".to_vec()),
            InnerOp::Run { byte: b'X', len: 3, output_start_pos: 5 },
            InnerOp::Run { byte: b'X', len: 2, output_start_pos: 8 },
            InnerOp::Copy(Copy { src: CopySrc::Dict, addr: 0, len: 3 }),
        ];

        let optimized_ops = optimize_and_convert_ops(inner_ops);

        assert_eq!(optimized_ops.len(), 3);
        match &optimized_ops[0] {
            Op::Add(add) => assert_eq!(add.bytes, b"ABCDE"),
            _ => panic!("Expected Add op"),
        }
        match &optimized_ops[1] {
            Op::Run(run) => {
                assert_eq!(run.byte, b'X');
                assert_eq!(run.len, 5);
            },
            _ => panic!("Expected Run op"),
        }
    }

    #[test]
    fn test_transcode() {
        let mut input = create_test_patch();
        let mut output = Cursor::new(Vec::new());

        transcode( //transcode from Segregated to Interleaved and sec_comp from smdiff to zstd
            &mut input,
            &mut output,
            Format::Interleaved,
            Some(smdiff_encoder::SecondaryCompression::Zstd { level: 3 }),
            smdiff_common::MAX_WIN_SIZE,
        ).unwrap();

        output.rewind().unwrap();
        let transcoded_ops = smdiff_decoder::reader::SectionIterator::new(output)
            .next()
            .unwrap()
            .unwrap()
            .0;

        // Add assertions to verify the transcoded output
        assert_eq!(transcoded_ops,make_correct_ops()); // Expected number of ops after optimization
        // Add more specific assertions based on expected optimizations
    }
}