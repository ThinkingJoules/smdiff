use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::DIR_PATH;


pub fn best_params_v2() {
    for inst_len in 1..7 {
         for address_cost in 1..=5u8 {
             let inline_cost_copy = calculate_inline_cost_v2(address_cost, inst_len);
             let inline_cost_add = calculate_inline_cost_v2(0, inst_len);
             let best = if inline_cost_copy < inline_cost_add {
                 //this is our categorical rule on when to encode add vs copy
                 assert!(1+address_cost  as u16<= inst_len,"inst_len: {} address_cost: {}",inst_len,address_cost);
                 "Copy"
             } else {
                 "Add"
             };
             println!("size: {} addr_len: {} Best Op= {} | CostCopy: {} CostAdd: {}",
                 inst_len, address_cost,best, inline_cost_copy, inline_cost_add);
         }
     }
 }

 pub fn calculate_inline_cost_v2(address_cost: u8, inst_len: u16) -> u32 {
     let size_indicator_cost = match inst_len {
         0..=62 => 0,
         63..=317 => 1,
         _ => 2,
     };
     let add_cost = if address_cost > 0 { 0 } else { inst_len };
     1 + size_indicator_cost + address_cost as u32 + add_cost as u32
 }

 #[derive(Debug, Clone)]
 struct EncodingResult {
     pattern_len: u8,
     seq_len: u16,
     address_cost: u8,
     seq_cost_copy: u32,
     seq_cost_add: u32,
     inline_cost_copy: u32,
     inline_cost_add: u32,
     best_method: String,
     min_cost: u32,
 }

 impl EncodingResult {
     pub fn new(pattern_len: u8, seq_len: u16, address_cost: u8, seq_cost_copy: u32, seq_cost_add: u32, inline_cost_copy: u32, inline_cost_add: u32) -> Self {
         let costs = [
             (seq_cost_copy, "CopySeq", 1),
             (seq_cost_add, "AddSeq", 1),
             (inline_cost_copy, "CopyInline", 0),
             (inline_cost_add, "AddInline", 0),
         ];

         let (min_cost, best_method) = costs
             .iter()
             .min_by_key(|&&(cost, _, priority)| (cost, priority))
             .map(|&(cost, method, _)| (cost, method.to_string()))
             .unwrap();

         EncodingResult {
             pattern_len,
             seq_len,
             address_cost,
             seq_cost_copy,
             seq_cost_add,
             inline_cost_copy,
             inline_cost_add,
             best_method,
             min_cost,
         }
     }
 }
 pub fn best_params_v1() {
     /*
     My Summary:
     Get rid of the idea of sequence. We only use it with Add, and mostly on pattern len = 1.
     Rework Op to reflect this.
     */
     let mut results: HashMap<String,Vec<EncodingResult>> = HashMap::new().into();
     let tot = 255u32 * u16::MAX as u32 * 4;
     let mut count = 0;
     let mut last = EncodingResult::new(0, 0, 0, 0, 0, 0, 0);
     for seq_len in 1..=u16::MAX {
         for pattern_len in 1..=255u8 {
             if pattern_len as u16 >= seq_len {
                 continue;
             }
             for address_cost in 1..=4u8 {
                 let seq_cost_copy = calculate_seq_cost_v1(address_cost, pattern_len) as u32;
                 let seq_cost_add = calculate_seq_cost_v1(0, pattern_len) as u32;
                 let inline_cost_copy = calculate_inline_cost_v1(address_cost, seq_len);
                 let inline_cost_add = calculate_inline_cost_v1(0, seq_len);

                 let result = EncodingResult::new(pattern_len, seq_len, address_cost, seq_cost_copy, seq_cost_add, inline_cost_copy, inline_cost_add);
                 if last.best_method != result.best_method {
                     let list = results.entry(result.best_method.clone()).or_insert_with(Vec::new);
                     list.push(result.clone());
                     last = result;
                 }
                 count += 1;
                 if count % 10000000 == 0 {
                     println!("Progress: {}%", (count as f64 / tot as f64) * 100.0);
                 }
             }
         }
     }
     //collect only the two seq types and print those
     let mut file = String::new();
     //write the results to a file
     let seq_results = results.get("CopySeq");
     if let Some(seq_results) =  seq_results {
         for r in seq_results {
             file.push_str(&format!("Method: CopySeq | Pattern: {} Seq: {} Address: {} SeqCostCopy: {} SeqCostAdd: {} InlineCostCopy: {} InlineCostAdd: {} BestMethod: {} MinCost: {}\n",
                 r.pattern_len, r.seq_len, r.address_cost, r.seq_cost_copy, r.seq_cost_add, r.inline_cost_copy, r.inline_cost_add, r.best_method, r.min_cost));
         }
     }
     let seq_results = results.get("AddSeq");
     if let Some(seq_results) =  seq_results {
         for r in seq_results {
             file.push_str(&format!("Method: AddSeq | Pattern: {} Seq: {} Address: {} SeqCostCopy: {} SeqCostAdd: {} InlineCostCopy: {} InlineCostAdd: {} BestMethod: {} MinCost: {}\n",
                 r.pattern_len, r.seq_len, r.address_cost, r.seq_cost_copy, r.seq_cost_add, r.inline_cost_copy, r.inline_cost_add, r.best_method, r.min_cost));

         }
     }

     //write to dir_path + "results.txt"
     fs::write(&Path::new(DIR_PATH).join("seq_results_v1.txt"), file).unwrap();

     let mut file = String::new();
     //write the results to a file
     let seq_results = results.get("CopyInline");
     if let Some(seq_results) =  seq_results {
         for r in seq_results {
             file.push_str(&format!("Method: CopyInline | Pattern: {} Seq: {} Address: {} SeqCostCopy: {} SeqCostAdd: {} InlineCostCopy: {} InlineCostAdd: {} BestMethod: {} MinCost: {}\n",
                 r.pattern_len, r.seq_len, r.address_cost, r.seq_cost_copy, r.seq_cost_add, r.inline_cost_copy, r.inline_cost_add, r.best_method, r.min_cost));
         }
     }
     let seq_results = results.get("AddInline");
     if let Some(seq_results) =  seq_results {
         for r in seq_results {
             file.push_str(&format!("Method: AddInline | Pattern: {} Seq: {} Address: {} SeqCostCopy: {} SeqCostAdd: {} InlineCostCopy: {} InlineCostAdd: {} BestMethod: {} MinCost: {}\n",
                 r.pattern_len, r.seq_len, r.address_cost, r.seq_cost_copy, r.seq_cost_add, r.inline_cost_copy, r.inline_cost_add, r.best_method, r.min_cost));

         }
     }

     //write to dir_path + "results.txt"
     fs::write(&Path::new(DIR_PATH).join("inline_results_v1.txt"), file).unwrap();

 }
 pub fn calculate_seq_cost_v1(address_cost: u8, pattern_len: u8) -> u16 {
     let seq_len_cost = 2;
     if address_cost > 0 {
         let size_indicator_cost = if pattern_len > 31 { 1 } else { 0 };
         (1 + size_indicator_cost + address_cost as u16 + seq_len_cost) as u16
     } else {
         let size_indicator_cost = if pattern_len > 63 { 1 } else { 0 };
         (1 + size_indicator_cost + pattern_len as u16 + seq_len_cost) as u16
     }
 }

 pub fn calculate_inline_cost_v1(address_cost: u8, seq_len: u16) -> u32 {
     if address_cost > 0 {
         let size_indicator_cost = if seq_len > 31 { 2 } else { 0 };
         1 + size_indicator_cost + address_cost as u32
     } else {
         let size_indicator_cost = if seq_len > 63 { 2 } else { 0 } as u32;
         1 + size_indicator_cost + seq_len as u32
     }
 }