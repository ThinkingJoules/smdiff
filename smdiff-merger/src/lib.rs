use std::{io::{Read, Seek, Write}, ops::Range};

use smdiff_common::{Run, WindowHeader, MAX_INST_SIZE, MAX_WIN_SIZE, MICRO_MAX_INST_COUNT};
use smdiff_reader::{Op, SectionReader};



///Extracted Instruction with a starting position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SparseOps{
    pub o_pos_start:u64,
    pub op:Op,
}
impl SparseOps {
    fn o_start(&self)->u64{
        self.o_pos_start
    }
}
impl MergeOp for SparseOps{
    fn skip(&mut self,amt:u32) {
        self.o_pos_start += amt as u64;
        self.op.skip(amt);
    }

    fn trunc(&mut self,amt:u32) {
        self.op.trunc(amt);
    }

    fn src_range(&self)->Option<Range<u64>> {
        self.op.src_range()
    }
    fn oal(&self)->u16 {
        self.op.oal()
    }
}
impl MergeOp for Run {
    fn skip(&mut self,amt:u32) {
        self.len -= amt as u8;
    }
    fn trunc(&mut self,amt:u32) {
        self.len -= amt as u8;
    }
    fn src_range(&self)->Option<Range<u64>> {
        None
    }
    fn oal(&self)->u16 {
        self.len as u16
    }
}
impl MergeOp for smdiff_common::Copy {
    fn skip(&mut self,amt:u32) {
        self.addr += amt as u64;
        self.len -= amt as u16;
    }
    fn trunc(&mut self,amt:u32) {
        self.len -= amt as u16;
    }
    fn src_range(&self)->Option<Range<u64>> {
        Some(self.addr..self.addr + self.len as u64)
    }
    fn oal(&self)->u16 {
        self.len
    }
}
impl MergeOp for smdiff_reader::Add {
    fn skip(&mut self,amt:u32){
        self.bytes = self.bytes.split_off(amt as usize);
    }
    fn trunc(&mut self,amt:u32){
        self.bytes.truncate(self.bytes.len() - amt as usize);
    }
    fn src_range(&self)->Option<Range<u64>> {
        None
    }
    fn oal(&self)->u16 {
        self.bytes.len() as u16
    }
}
impl MergeOp for Op{
    fn skip(&mut self,amt:u32){
        match self {
            Op::Run(run) => run.skip(amt),
            Op::Copy(copy) => copy.skip(amt),
            Op::Add(add) => add.skip(amt),
        }
    }
    fn trunc(&mut self,amt:u32){
        match self {
            Op::Run(run) => run.trunc(amt),
            Op::Copy(copy) => copy.trunc(amt),
            Op::Add(add) => add.trunc(amt),
        }
    }
    fn src_range(&self)->Option<Range<u64>> {
        match self {
            Op::Run(run) => run.src_range(),
            Op::Copy(copy) => copy.src_range(),
            Op::Add(add) => add.src_range(),
        }
    }
    fn oal(&self)->u16 {
        match self {
            Op::Run(run) => run.oal(),
            Op::Copy(copy) => copy.oal(),
            Op::Add(add) => add.oal(),
        }
    }
}

// pub trait PosInst:MergeInst{
//     fn o_start(&self)->u64;
// }
pub trait MergeOp:Clone+Sized{
    ///Shorten the 'front' of the instruction
    fn skip(&mut self,amt:u32);
    ///Truncate off the 'back' of the instruction
    fn trunc(&mut self,amt:u32);
    ///If this is a Copy, what is the source byte range that would contain exactly this one instruction.
    fn src_range(&self)->Option<Range<u64>>;
    fn oal(&self)->u16;
    // fn split_at(mut self,first_inst_len:u32)->(Self,Self){
    //     let mut second = self.clone();
    //     self.trunc(self.oal() as u32 - first_inst_len);
    //     second.skip(first_inst_len);
    //     (self,second)
    // }
}

///Finds the index of the instruction that controls the given output position.
/// # Arguments
/// * `insts` - The list of instructions to search.
/// * `o_pos` - The output position to find the controlling instruction for.
/// # Returns
/// The index of the controlling instruction, or None if no such instruction exists.
pub fn find_controlling_inst(insts:&[SparseOps],o_pos:u64)->Option<usize>{
    let inst = insts.binary_search_by(|probe|{
        let end = probe.o_start() + probe.oal() as u64;
        if (probe.o_start()..end).contains(&o_pos){
            return std::cmp::Ordering::Equal
        }else if probe.o_start() > o_pos {
            return std::cmp::Ordering::Greater
        }else{
            return std::cmp::Ordering::Less
        }
    });
    if let Ok(idx) = inst {
        Some(idx)
    }else {
        None
    }
}

///Returns a cloned and clipped subslice of instructions that exactly covers the requested output range.
/// # Arguments
/// * `instructions` - The list of instructions to extract from.
/// * `start` - The output position (output byte offset) to start the slice at.
/// * `len` - The length of the slice in output bytes.
/// # Returns
/// A vector containing the cloned and clipped instructions that exactly cover the requested output range.
/// If the output range is not covered by the instructions, None is returned.
///
/// This does not check that the instructions are sequential.
pub fn get_exact_slice(instructions:&[SparseOps],start:u64,len:u32)->Option<Vec<SparseOps>>{
    let start_idx = find_controlling_inst(instructions,start)?;
    let end_pos = start + len as u64;
    let mut slice = Vec::new();
    let mut complete = false;

    for inst in instructions[start_idx..].iter() {
        let inst_len = inst.oal();
        let o_start = inst.o_start();
        let cur_inst_end = o_start + inst_len as u64;
        let mut cur_inst = inst.clone();
        if start > o_start {
            let skip = start - o_start;
            cur_inst.skip(skip as u32);
        }
        if end_pos < cur_inst_end {
            let trunc = cur_inst_end - end_pos;
            cur_inst.trunc(trunc as u32);
        }
        debug_assert!(cur_inst.oal() > 0, "The instruction length is zero");
        slice.push(cur_inst);

        if cur_inst_end >= end_pos {
            complete = true;
            //debug_assert!(sum_len_in_o(&slice)==len as u64,"{} != {} start:{} end_pos:{} ... {:?} from {:?}",sum_len_in_o(&slice),len,start,end_pos,&slice,instructions);
            break;
        }
    }
    if !complete {
        return None;
    }
    Some(slice)
}

//Should maybe move this to Reader?
///Stats about the patch file.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Stats{
    pub add_bytes:usize,
    pub run_bytes:usize,
    pub copy_bytes:usize,
    pub add_cnt:usize,
    pub run_cnt:usize,
    pub copy_d_cnt:usize,
    pub copy_o_cnt:usize,
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
    pub fn copy_d(&mut self, len:usize){
        self.copy_bytes += len;
        self.copy_d_cnt += 1;
        self.output_size += len;
    }
    pub fn copy_o(&mut self, len:usize){
        self.copy_bytes += len;
        self.copy_o_cnt += 1;
        self.output_size += len;
    }
    pub fn has_copy(&self)->bool{
        self.copy_bytes > 0
    }
}

///Extracts all instructions from all windows.
///Memory consumption may be 2-4x the size of the encoded (uncompressed) patch.
pub fn extract_patch_instructions<R:Read + Seek>(patch:R)->std::io::Result<(Vec<SparseOps>, Stats)>{
    let mut output = Vec::new();
    let mut reader = SectionReader::new(patch)?;
    let mut o_pos_start = 0;
    let mut stats = Stats::new();
    while let Ok(Some((insts,_output_size))) = reader.next() {
        for inst in insts{
            let oal_len = inst.oal() as usize;
            match &inst{
                smdiff_common::Op::Run(_) => {
                    output.push(SparseOps{o_pos_start,op: inst});
                    stats.run(oal_len);
                },
                smdiff_common::Op::Copy(c) => {
                    match c.src{
                        smdiff_common::CopySrc::Dict => {
                            stats.copy_d(oal_len);
                        },
                        smdiff_common::CopySrc::Output => {
                            stats.copy_o(oal_len);
                        },
                    }
                    output.push(SparseOps{o_pos_start,op: inst});
                    stats.copy_d(oal_len);

                },
                smdiff_common::Op::Add(_) => {
                    output.push(SparseOps{o_pos_start,op: inst});
                    stats.add(oal_len);
                },
            }
            o_pos_start += oal_len as u64;
        }
    }

    Ok((output,stats))
}

/// This function will dereference all Copy_Output instructions in the extracted instructions.
pub fn deref_copy_o(extracted:Vec<SparseOps>)->Vec<SparseOps>{
    //TODO: We could optimize by having get_exact_slice return *what to do* to dereference the copy.
    // The advantage would be we wouldn't clone any Ops.
    // We would point to the first and last index in `extracted` and the skip/trunc values for those two ops.
    // This is faster and more memory efficient.
    // However, we need to deal with an enum type that will make a mess of things when we get to the main merge fn.
    let mut output:Vec<SparseOps> = Vec::with_capacity(extracted.len());
    let mut cur_o_pos = 0;
    for SparseOps { op: inst, ..  } in extracted {
        match inst {
            Op::Copy(copy) if matches!(copy.src, smdiff_common::CopySrc::Output) => {
                //let copy = copy.clone();
                let o_start = copy.addr;
                let resolved = get_exact_slice(output.as_slice(), o_start, copy.oal() as u32).unwrap();
                for resolved_inst in resolved {
                    let o_pos_start = cur_o_pos;
                    cur_o_pos += resolved_inst.op.oal() as u64;
                    output.push(SparseOps { o_pos_start, op: resolved_inst.op });
                }
            },
            _ => {
                let o_pos_start = cur_o_pos;
                cur_o_pos += inst.oal() as u64;
                output.push(SparseOps { o_pos_start, op: inst })

            },
        }
    }
    output
}

fn find_mergeable_copies(extract:&[SparseOps],shift:usize,dest:&mut Vec<usize>){
    for (i,ext) in extract.iter().enumerate(){
        match ext.op {
            Op::Copy(copy) if matches!(copy.src, smdiff_common::CopySrc::Dict) => {
                dest.push(i+shift);
            },
            _ => (),
        }
    }
}
//Merger struct that can accept merging of additional patches.
#[derive(Clone, Debug)]
pub struct Merger{
    ///The summary patch that will be written to the output.
    terminal_patch: Vec<SparseOps>,
    ///If this is empty, merging a patch will have no effect.
    ///These are where TerminalInst::CopySS are found.
    terminal_copy_indices: Vec<usize>,
    //final_size: u64,
}

impl Merger {
    ///Creates a new Merger from a terminal patch.
    ///This should only be called using the patch that generates the output file you want.
    /// # Arguments
    /// * `terminal_patch` - The terminal patch that will serve as the core set of instructions.
    /// # Returns
    /// If the terminal patch has no Copy instructions, a SummaryPatch is returned.
    /// If the terminal patch has even a single Copy instructions, a Merger is returned.
    pub fn new<R:Read + Seek>(terminal_patch:R) -> std::io::Result<Result<Merger,SummaryPatch>> {
        let (terminal_patch,stats) = extract_patch_instructions(terminal_patch)?;
        if stats.copy_bytes == 0{
            return Ok(Err(SummaryPatch(terminal_patch.into_iter().map(|s|s.op).collect())));
        }
        let mut terminal_copy_indices = Vec::new();
        //we for sure need to translate local. I think translate global isn't needed??
        //will need to check this.
        let terminal_patch = deref_copy_o(terminal_patch);
        find_mergeable_copies(&terminal_patch,0,&mut terminal_copy_indices);
        debug_assert!(!terminal_copy_indices.is_empty(), "terminal_copy_indices should not be empty");
        Ok(Ok(Merger{
            terminal_patch,
            terminal_copy_indices,
            //final_size:stats.output_size as u64
        }))
    }
    ///Merges a predecessor patch into the terminal patch.
    ///This should be called using proper order of patches.
    /// # Arguments
    /// * `predecessor_patch` - The patch to merge into the current summary patch.
    /// # Returns
    /// If the resulting patch has no Copy instructions, a SummaryPatch is returned.
    /// If the resulting patch has even a single Copy instructions, a Merger is returned.
    pub fn merge<R:Read + Seek>(mut self, predecessor_patch:R) -> std::io::Result<Result<Merger,SummaryPatch>> {
        debug_assert!({
            let mut x = 0;
            for inst in self.terminal_patch.iter(){
                assert_eq!(x,inst.o_pos_start);
                x += inst.op.oal() as u64;
            }
            true
        });
        let (mut predecessor_patch,stats) = extract_patch_instructions(predecessor_patch)?;
        if stats.has_copy(){
            predecessor_patch = deref_copy_o(predecessor_patch);
        }
        let mut terminal_copy_indices = Vec::with_capacity(self.terminal_copy_indices.len());
        let mut inserts = Vec::with_capacity(self.terminal_copy_indices.len());
        let mut shift = 0;
        for i in self.terminal_copy_indices{
            let SparseOps { op: inst,.. } = self.terminal_patch[i].clone();
            let copy = inst.take_copy().expect("Expected Copy");
            //this a src window copy that we need to resolve from the predecessor patch.
            debug_assert!(matches!(copy.src, smdiff_common::CopySrc::Dict));
            let o_start = copy.addr; //ssp is o_pos, u is offset from that.
            let resolved = get_exact_slice(&predecessor_patch, o_start, copy.oal() as u32).unwrap();
            //debug_assert_eq!(sum_len_in_o(&resolved), copy.len_in_o() as u64, "resolved: {:?} copy: {:?}",resolved,copy);
            find_mergeable_copies(&resolved, i+shift, &mut terminal_copy_indices);
            shift += resolved.len() - 1;
            inserts.push((i, resolved));

        }
        //now we expand the old copy values with the derefd instructions.
        //debug_assert_eq!(sum_len_in_o(&self.terminal_patch), self.final_size, "final size: {} sum_len: {}",self.final_size,sum_len_in_o(&self.terminal_patch));
        if terminal_copy_indices.is_empty(){

            Ok(Err(SummaryPatch(expand_to(self.terminal_patch, inserts, |s|s.op))))
        }else{
            self.terminal_patch = expand_to(self.terminal_patch, inserts, |s|s);
            self.terminal_copy_indices = terminal_copy_indices;
            Ok(Ok(self))
        }
    }
    pub fn finish(self)->SummaryPatch{
        SummaryPatch(self.terminal_patch.into_iter().map(|s|s.op).collect())
    }

}

///This is returned when the current summary patch contains no Copy instructions, OR when you are finished with the Merger.
#[derive(Debug)]
pub struct SummaryPatch(Vec<Op>);
impl SummaryPatch{
    ///Writes the summary patch to a sink.
    /// # Arguments
    /// * `sink` - The sink to write the summary patch to.
    /// * `max_u_size` - The maximum size of the super string U. If None, the default is 256MB. This is used to help determine when new windows are created.
    /// # Returns
    /// The sink that was passed in.
    pub fn write<W:Write>(self,mut sink:W,max_win_size:Option<usize>)->std::io::Result<W>{
        //window needs to be MAX_INST_SIZE..=MAX_WIN_SIZE
        let max_win_size = max_win_size.unwrap_or(MAX_WIN_SIZE).min(MAX_WIN_SIZE).max(MAX_INST_SIZE);
        //first figure out if this is a micro format or window format.
        //if it's a micro format, we just write the instructions.
        //if it's a window format, we need to figure out the windows.
        let num_ops = self.0.len();
        if num_ops <= MICRO_MAX_INST_COUNT{
            //sum the oal of all instructions.
            let mut oal_sum = 0;
            for inst in self.0.iter(){
                oal_sum += inst.oal() as u64;
            }
            if oal_sum <= max_win_size as u64{
                let header = smdiff_common::FileHeader::new_micro(num_ops as u8);
                smdiff_writer::write_file_header(&header, &mut sink)?;
                smdiff_writer::write_micro_section(&self.0, &mut sink)?;
                return Ok(sink);
            }
        }
        //we need to write a window patch.
        let header = smdiff_common::FileHeader::new_window();
        smdiff_writer::write_file_header(&header, &mut sink)?;
        for (ops,header) in make_op_windows(&self.0, max_win_size){
            smdiff_writer::write_win_section(ops, header, &mut sink)?;
        }
        Ok(sink)
    }
}
fn expand_to<T, F>(
    mut target: Vec<SparseOps>,
    inserts: Vec<(usize, Vec<SparseOps>)>,
    mut converter: F,
) -> Vec<T>
where
    F: FnMut(SparseOps) -> T,
{
    // Calculate the total number of elements to be inserted to determine the new vector's length.
    let total_insertions: usize = inserts.iter().map(|(_, ins)| ins.len()).sum();
    let final_length = target.len() + total_insertions;

    // Allocate a new vector with the final required size.
    let mut result = Vec::with_capacity(final_length);

    // Sort inserts by position to process them in order.
    let mut sorted_inserts = inserts;
    sorted_inserts.sort_by_key(|k| k.0);

    target.reverse();
    // Trackers for the current position in the original vector and the inserts.
    let mut cur_idx = 0;
    let mut cur_o_pos = 0;
    for (insert_pos, insert_vec) in sorted_inserts {
        // Copy elements from the current position up to the insert position.
        while cur_idx < insert_pos {
            match target.pop() {
                Some(mut elem) => {
                    let len = elem.oal();
                    elem.o_pos_start = cur_o_pos;
                    cur_o_pos += len as u64;
                    result.push(converter(elem));
                    cur_idx += 1;
                }
                None => break,
            }
        }
        // Insert the new elements.
        for mut elem in insert_vec {
            let len = elem.oal();
            elem.o_pos_start = cur_o_pos;
            cur_o_pos += len as u64;
            result.push(converter(elem));
        }
        target.pop();//we get rid of the expanded element.
        cur_idx += 1;
    }

    // After processing all inserts, copy any remaining elements from the original vector.
    while let Some(mut elem) = target.pop() {
        let len = elem.oal();
        elem.o_pos_start = cur_o_pos;
        cur_o_pos += len as u64;
        result.push(converter(elem));
    }
    result

}

fn make_op_windows(ops: &[Op], max_win_size: usize) -> Vec<(&[Op],WindowHeader)> {
    let max_win_size = max_win_size as u32;
    let mut result = Vec::new();
    let mut output_size = 0;
    let mut num_add_bytes = 0;
    let mut start_index = 0;

    for (end_index, op) in ops.iter().enumerate() {
        // Check if adding the current op exceeds the window size
        let op_size = op.oal() as u32;
        if output_size + op_size > max_win_size {
            result.push((&ops[start_index..end_index],WindowHeader{ num_operations: (end_index-start_index) as u32, num_add_bytes, output_size}));
            start_index = end_index;
            output_size = 0;
            num_add_bytes = 0;
        }
        if op.is_add() {
            num_add_bytes += op_size;
        }
        output_size += op_size;
    }

    // Add the last group
    result.push((&ops[start_index..],WindowHeader{ num_operations: (ops.len()-start_index) as u32, num_add_bytes, output_size}));

    result
}



#[cfg(test)]
mod test_super {

    use smdiff_common::{Copy, CopySrc, FileHeader};
    use smdiff_decoder::apply_patch;
    use smdiff_reader::Add;
    use super::*;
    /*
    Basic merger tests will start with a src file of '01234'
    We will then create a series of patches that will make certain *changes* to the file.
    That is, we want to be able to apply them in different orders for different effects.
    To this end, all of the target windows must be the same size.
    We will pick 10 bytes as our target window size. This is twice the length of 'hello'

    We need to test the following:
    Copy Passthrough
    Add/Run precedence

    For the copy:
    We will make a patch that will copy the first five bytes to the last five bytes.
    This should turn '01234' into '0123401234'

    For the add/run:
    We will make a patch that will insert 'A' (ADD) at first pos Copy next 2, Then 'XXX'(Run) + 'YZ'(Add) The COpy rem
    This should turn '01234' into 'A12XXXYZ34'

    Then we do a patch with multiple transforms internally
    Complex:
    We will Add 'Y' Run(2) 'Z' CopyD 4,1 CopyO 2,2 (Z4) Copy u_pos 1 len 4
    This should turn '01234' into 'YZZ4Z41234'

    We can then mix and match these patches and we should be able to reason about the outputs.
    */
    const HDR:FileHeader = FileHeader{ compression_algo: 0, format: smdiff_common::Format::WindowFormat };
    use std::io::Cursor;
    fn copy_patch() -> Cursor<Vec<u8>> {
        let mut sink = Cursor::new(Vec::new());
        smdiff_writer::write_file_header(&HDR, &mut sink).unwrap();
        let ops = &[
            Op::Copy(Copy { src: CopySrc::Dict, addr: 0, len: 5}),
            Op::Copy(Copy { src: CopySrc::Dict, addr: 0, len: 5}),
        ];
        smdiff_writer::write_win_section(ops, WindowHeader { num_operations:2, num_add_bytes: 0, output_size: 10 }, &mut sink).unwrap();
        sink.rewind().unwrap();
        sink
    }
    fn add_run_patch() -> Cursor<Vec<u8>> {
        let mut sink = Cursor::new(Vec::new());
        smdiff_writer::write_file_header(&HDR, &mut sink).unwrap();
        let ops = &[
            Op::Add(Add{bytes:b"A".to_vec()}),
            Op::Copy(Copy { src: CopySrc::Dict, addr: 1, len: 2}),
            Op::Run(Run { byte: b'X', len: 3}),
            Op::Add(Add{bytes:b"YZ".to_vec()}),
            Op::Copy(Copy { src: CopySrc::Dict, addr: 3, len: 2}),
        ];
        smdiff_writer::write_win_section(ops, WindowHeader { num_operations:5, num_add_bytes: 3, output_size: 10 }, &mut sink).unwrap();
        sink.rewind().unwrap();
        sink
    }
    fn complex_patch()->Cursor<Vec<u8>>{
        let mut sink = Cursor::new(Vec::new());
        smdiff_writer::write_file_header(&HDR, &mut sink).unwrap();
        let ops = &[
            Op::Add(Add{bytes:b"Y".to_vec()}),
            Op::Run(Run { byte: b'Z', len: 2}),
            Op::Copy(Copy { src: CopySrc::Dict, addr: 4, len: 1}),
            Op::Copy(Copy { src: CopySrc::Output, addr: 2, len: 2}),
            Op::Copy(Copy { src: CopySrc::Dict, addr: 1, len: 4}),
        ];
        smdiff_writer::write_win_section(ops, WindowHeader { num_operations:5, num_add_bytes: 1, output_size: 10 }, &mut sink).unwrap();
        sink.rewind().unwrap();
        sink
    }
    const SRC:&[u8] = b"01234";
    #[test]
    fn test_copy_add(){
        //01234 Copy 0123401234 Add-> A12XXXYZ34
        let answer = b"A12XXXYZ34";
        let copy = copy_patch();
        let add_run = add_run_patch();
        let merger = Merger::new(add_run).unwrap().unwrap();
        let merger = merger.merge(copy).unwrap().unwrap();
        let merged_patch = merger.finish().write(Vec::new(), None).unwrap();
        let mut cursor = Cursor::new(merged_patch);
        let mut output = Vec::new();
        apply_patch(&mut cursor, Some(&mut Cursor::new(SRC.to_vec())), &mut output).unwrap();
        //print output as a string
        let as_str = std::str::from_utf8(&output).unwrap();
        println!("{}",as_str);
        assert_eq!(output,answer);
    }
    #[test]
    fn test_add_copy(){
        //01234 Add -> A12XXXYZ34 Copy-> A12XXA12XX
        let answer = b"A12XXA12XX";
        let copy = copy_patch();
        let add_run = add_run_patch();
        let merger = Merger::new(copy).unwrap().unwrap();
        let merger = merger.merge(add_run).unwrap().unwrap();
        let merged_patch = merger.finish().write(Vec::new(), None).unwrap();
        let mut cursor = Cursor::new(merged_patch);
        let mut output = Vec::new();
        apply_patch(&mut cursor, Some(&mut Cursor::new(SRC.to_vec())), &mut output).unwrap();
        //print output as a string
        let as_str = std::str::from_utf8(&output).unwrap();
        println!("{}",as_str);
        assert_eq!(output,answer);
    }
    #[test]
    fn test_add_complex(){
        //01234 Add-> A12XXXYZ34 Compl YZZXZX12XX
        let answer = b"YZZXZX12XX";
        let add_run = add_run_patch();
        let comp = complex_patch();
        let merger = Merger::new(comp).unwrap().unwrap();
        let merger = merger.merge(add_run).unwrap().unwrap();
        let merged_patch = merger.finish().write(Vec::new(), None).unwrap();
        let mut cursor = Cursor::new(merged_patch);
        let mut output = Vec::new();
        apply_patch(&mut cursor, Some(&mut Cursor::new(SRC.to_vec())), &mut output).unwrap();
        //print output as a string
        let as_str = std::str::from_utf8(&output).unwrap();
        println!("{}",as_str);
        assert_eq!(output,answer);
    }
    #[test]
    fn test_complex_add(){
        //01234 Compl-> YZZ4Z41234 Add AZZXXXYZ4Z
        let answer = b"AZZXXXYZ4Z";
        let add_run = add_run_patch();
        let comp = complex_patch();
        let merger = Merger::new(add_run).unwrap().unwrap();
        let merger = merger.merge(comp).unwrap().unwrap();
        let merged_patch = merger.finish().write(Vec::new(), None).unwrap();
        let mut cursor = Cursor::new(merged_patch);
        let mut output = Vec::new();
        apply_patch(&mut cursor, Some(&mut Cursor::new(SRC.to_vec())), &mut output).unwrap();
        //print output as a string
        let as_str = std::str::from_utf8(&output).unwrap();
        println!("{}",as_str);
        assert_eq!(output,answer);
    }
    #[test]
    fn test_all_seq(){
        //01234 Add-> A12XXXYZ34 Compl YZZXZX12XX -> Copy YZZXZYZZXZ
        let answer = b"YZZXZYZZXZ";
        let add_run = add_run_patch();
        let comp = complex_patch();
        let copy = copy_patch();
        let merger = Merger::new(copy).unwrap().unwrap();
        let merger = merger.merge(comp).unwrap().unwrap();
        let merger = merger.merge(add_run).unwrap().unwrap_err();
        let merged_patch = merger.write(Vec::new(), None).unwrap();
        let mut cursor = Cursor::new(merged_patch);
        let mut output = Vec::new();
        //We don't need Src, since the last merge yielded SummaryPatch
        apply_patch::<_, Cursor<Vec<u8>>,_>(&mut cursor, None, &mut output).unwrap();
        //print output as a string
        let as_str = std::str::from_utf8(&output).unwrap();
        println!("{}",as_str);
        assert_eq!(output,answer);
    }
    #[test]
    fn test_kitchen_sink(){
        //"hello" -> "hello world!" -> "Hello! Hello! Hello. hello. hello..."
        //we need to use a series of VCD_TARGET windows and Sequences across multiple patches
        //we should use copy/seq excessively since add/run is simple in the code paths.
        let src = b"hello!";
        let mut sink = Cursor::new(Vec::new());
        smdiff_writer::write_file_header(&HDR, &mut sink).unwrap();
        let ops = &[
            Op::Copy(Copy { src: CopySrc::Dict, addr: 0, len: 5}),
        ];
        smdiff_writer::write_win_section(ops, WindowHeader { num_operations:1, num_add_bytes: 0, output_size: 5 }, &mut sink).unwrap();
        let ops = &[
            Op::Add(Add{bytes:b" w".to_vec()}),
            Op::Copy(Copy { src: CopySrc::Output, addr: 4, len: 1}),
            Op::Add(Add{bytes:b"rld".to_vec()}),
        ];
        smdiff_writer::write_win_section(ops, WindowHeader { num_operations:3, num_add_bytes: 5, output_size: 6 }, &mut sink).unwrap();
        let ops = &[
            Op::Copy(Copy { src: CopySrc::Dict, addr: 5, len: 1}),
        ];
        smdiff_writer::write_win_section(ops, WindowHeader { num_operations:1, num_add_bytes: 0, output_size: 1 }, &mut sink).unwrap();
        let p1 = sink.into_inner();
        let p1_answer = b"hello world!";
        let mut cursor = Cursor::new(p1.clone());
        let mut output = Vec::new();
        apply_patch(&mut cursor, Some(&mut Cursor::new(src.to_vec())), &mut output).unwrap();
        println!("{}",std::str::from_utf8(&output).unwrap());
        assert_eq!(output,p1_answer); //ensure our instructions do what we think they are.
        let patch_1 = Cursor::new(p1);
        let mut sink = Cursor::new(Vec::new());
        smdiff_writer::write_file_header(&HDR, &mut sink).unwrap();
        let ops = &[
            Op::Add(Add{bytes:b"H".to_vec()}),
            Op::Copy(Copy { src: CopySrc::Dict, addr: 1, len: 4}), //ello
            Op::Copy(Copy { src: CopySrc::Dict, addr: 11, len: 1}), //'!'
            Op::Copy(Copy { src: CopySrc::Dict, addr: 5, len: 1}), //' '
        ];
        smdiff_writer::write_win_section(ops, WindowHeader { num_operations:4, num_add_bytes: 1, output_size: 7 }, &mut sink).unwrap();
        let ops = &[
            Op::Copy(Copy { src: CopySrc::Output, addr: 0, len: 7}), //'Hello! '
            Op::Copy(Copy { src: CopySrc::Output, addr: 7, len: 5}),  //'Hello'
            Op::Add(Add{bytes:b".".to_vec()}),
            Op::Copy(Copy { src: CopySrc::Output, addr: 13, len: 1}), // ' '
        ];
        smdiff_writer::write_win_section(ops, WindowHeader { num_operations:4, num_add_bytes: 1, output_size: 14 }, &mut sink).unwrap();
        let ops = &[
            Op::Add(Add{bytes:b"h".to_vec()}),
            Op::Copy(Copy { src: CopySrc::Output, addr: 15, len: 6}),  //'ello. '
        ];
        smdiff_writer::write_win_section(ops, WindowHeader { num_operations:2, num_add_bytes: 1, output_size: 7 }, &mut sink).unwrap();
        let ops = &[
            Op::Copy(Copy { src: CopySrc::Output, addr: 21, len: 5}),  //'hello'
            Op::Run(Run { byte: b'.', len: 3}),
        ];
        smdiff_writer::write_win_section(ops, WindowHeader { num_operations:2, num_add_bytes: 0, output_size: 8 }, &mut sink).unwrap();
        let p2 = sink.into_inner();
        let p2_answer = b"Hello! Hello! Hello. hello. hello...";
        let mut cursor = Cursor::new(p2.clone());
        let mut output = Vec::new();
        apply_patch(&mut cursor, Some(&mut Cursor::new(p1_answer.to_vec())), &mut output).unwrap();
        println!("{}",std::str::from_utf8(&output).unwrap());
        assert_eq!(output,p2_answer); //ensure our instructions do what we think they are.
        let patch_2 = Cursor::new(p2);
        let merger = Merger::new(patch_2).unwrap().unwrap();
        let merger = merger.merge(patch_1).unwrap().unwrap();
        let merged_patch = merger.finish().write(Vec::new(), None).unwrap();
        let mut cursor = Cursor::new(merged_patch);
        let mut output = Vec::new();
        let answer = b"Hello! Hello! Hello. hello. hello...";
        apply_patch(&mut cursor, Some(&mut Cursor::new(src.to_vec())), &mut output).unwrap();
        //print output as a string
        let as_str = std::str::from_utf8(&output).unwrap();
        println!("{}",as_str);
        assert_eq!(output,answer);

    }

}