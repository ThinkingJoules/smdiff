
#![allow(dead_code)]
const HASH_MULTIPLIER_32_BIT: u32 = 1597334677;
const HASH_MULTIPLIER_64_BIT: u64 = 1181783497276652981;

#[cfg(target_pointer_width = "64")]
const MOD_INV: usize = 13515856136758413469;
#[cfg(target_pointer_width = "32")]
const MOD_INV: usize = 851723965;

#[cfg(target_pointer_width = "64")]
const HASH_MULT: usize = HASH_MULTIPLIER_64_BIT as usize;
#[cfg(target_pointer_width = "32")]
const HASH_MULT: usize = HASH_MULTIPLIER_32_BIT as usize;

#[cfg(target_pointer_width = "64")]
const FIRST_BYTE_WEIGHT: usize = 2694756932615366293;

#[cfg(target_pointer_width = "32")]
const FIRST_BYTE_WEIGHT: usize = 1614047349;

#[cfg(target_pointer_width = "64")]
const POWERS: [usize; 9] =[
    12309708155212419425,
    10962798122732597373,
    17101839937587700905,
    4214492644603347877,
    1752871083341145137,
    13959273915025570317,
    12787978306174927353,
    1181783497276652981,
    1,
];

#[cfg(target_pointer_width = "32")]
const POWERS: [usize; 9] = [
    785718369,
    2237430173,
    2226829033,
    2554380293,
    438510001,
    1152009645,
    1664532153,
    1597334677,
    1,
];

#[inline(always)]
pub(crate) fn calculate_large_checksum(data: &[u8]) -> usize {
    data.iter()
        .zip(POWERS.iter())
        .fold(0, |acc, (byte, power)|{
            acc.wrapping_add(power.wrapping_mul(*byte as usize))
        })
}

#[inline(always)]
pub(crate) fn update_large_checksum_fwd(checksum: usize, old:u8, new:u8) -> usize {
    (HASH_MULT).wrapping_mul(checksum)//multiply to 'shift values' left
    .wrapping_sub(FIRST_BYTE_WEIGHT.wrapping_mul(old as usize))
    .wrapping_add(new as usize)
}

#[inline(always)]
pub(crate) fn update_large_checksum_bwd(checksum: usize, old:u8, new:u8) -> usize {
    checksum.wrapping_sub(old as usize)
    .wrapping_add(FIRST_BYTE_WEIGHT.wrapping_mul(new as usize))
    .wrapping_mul(MOD_INV)
}
const SMALL_POWERS: [u32; 4] = [
    1143961941,
    1940812585,
    3141592653,
    1,
];
const SMALL_FIRST_BYTE_WEIGHT: u32 = 2508840081;
#[inline(always)]
pub(crate) fn calculate_small_checksum(data: &[u8]) -> u32 {
    data.iter()
        .zip(SMALL_POWERS.iter())
        .fold(0, |acc, (byte, power)|{
            acc.wrapping_add(power.wrapping_mul(*byte as u32))
        })
}
//I tested a couple multipliers but none seemed to have much effect on speed of final encoding.
//Probably needs to be looked at since my trgt matcher sucks.
#[inline(always)]
pub(crate) fn update_small_checksum_fwd(checksum: u32, old:u8, new:u8) -> u32 {
    (3141592653u32).wrapping_mul(checksum)//multiply to 'shift values' left
    .wrapping_sub(SMALL_FIRST_BYTE_WEIGHT.wrapping_mul(old as u32))
    .wrapping_add(new as u32)
}

// This is about a wash compared to the rolling method.
// #[inline(always)]
// pub(crate) fn calculate_small_checksum_direct(data: &[u8]) -> u32 {
//     let state = u32::from_ne_bytes(data[0..4].try_into().unwrap());
//     state.wrapping_mul(HASH_MULTIPLIER_32_BIT)
// }





#[cfg(test)]
mod test_super {
    use super::*;

    #[test]
    fn test_rolling_hash_fwd() {
        let initial_data = b"hello world, this is a test of the rolling hash"; // Longer example
        // Simulate a rolling window
        let mut hash = calculate_large_checksum(&initial_data[..]);
        for i in 1..(initial_data.len() - 20) {
            let old_char = initial_data[i - 1];
            let new_char = initial_data[i + 8];
            let expected_hash = calculate_large_checksum(&initial_data[i..]);
            hash = update_large_checksum_fwd(hash, old_char, new_char);
            assert_eq!(hash,expected_hash, "config.update Failed at starting index {}", i);
        }
    }

    #[test]
    fn test_rolling_hash_bwd() {
        let initial_data = b"hello world, this is a test of the rolling hash"; // Longer example
        let start_i = (initial_data.len() - 20)-9;
        // Simulate a rolling window
        let mut hash = calculate_large_checksum(&initial_data[start_i..]);
        for i in (0..start_i).rev() {
            let hash_win = &initial_data[i..];
            let new_char = initial_data[i];
            let old_char = initial_data[i + 9];
            let expected_hash = calculate_large_checksum(hash_win);
            hash = update_large_checksum_bwd(hash, old_char, new_char);
            assert_eq!(hash,expected_hash, "config.update Failed at starting index {}", i);
        }
    }
}