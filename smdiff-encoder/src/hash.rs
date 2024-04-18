use std::ops::Range;


const BASE: u64 = 256;
/// Number picked with u16::MAX as the upper bound for the pattern length.
const MODULUS: u64 = 4_294_967_291;

/// Calculates the value of `h` for the given pattern length and prime number.
fn calculate_h(pattern_length: usize) -> u64 {
    let mut h = 1_u64;
    for _ in 0..pattern_length - 1 {
        h = (h * BASE) % MODULUS;
    }
    h
}

/// Initializes the hash value for a byte slice (either pattern or text window).
fn initialize_hash(bytes: &[u8]) -> u64 {
    let mut hash = 0_u64;
    for &byte in bytes {
        hash = (hash * BASE + byte as u64) % MODULUS;
    }
    hash
}

/// Updates the hash value when the window slides by one character.
fn update_hash(old_hash: u64, old_char: u8, new_char: u8, h: u64) -> u64 {
    let new_hash = (old_hash + MODULUS - old_char as u64 * h % MODULUS) % MODULUS;
    (new_hash * BASE + new_char as u64) % MODULUS
}

/// Checks if the two byte slices have a common substring of length `len`.
/// Returns the starting indices of the common substring in the two slices.
pub fn has_common_substring(a: &[u8], b: &[u8], len: usize) -> Option<(usize, usize)> {
    if len == 0 {
        return None;
    }

    let mut a_hashes = std::collections::HashMap::new();
    let h = calculate_h(len);
    let mut hash= initialize_hash(&a[..len]);
    a_hashes.insert(hash, 0);

    for i in 1..=a.len() - len {
        hash = update_hash(hash, a[i - 1], a[i + len - 1], h);
        a_hashes.insert(hash, i);
    }

    let mut hash= initialize_hash(&b[..len]);
    if a_hashes.contains_key(&hash) {
        return Some((a_hashes[&hash], 0));
    }

    for i in 1..=b.len() - len {
        hash = update_hash(hash, b[i - 1], b[i + len - 1], h);
        if a_hashes.contains_key(&hash) {
            return Some((a_hashes[&hash], i));
        }
    }

    None
}

///Returns the longest common run range in `a`
pub fn longest_common_run(a: &[u8], b: &[u8]) -> Range<usize> {
    let mut low = 0;
    let mut high = usize::min(a.len(), b.len());
    let mut start_index = 0;
    let mut max_len = 0;

    while low <= high {
        let mid = (low + high) / 2;
        if let Some((start, _)) = has_common_substring(a, b, mid) {
            max_len = mid;
            start_index = start;
            low = mid + 1;
        } else {
            high = mid - 1;
        }
    }

    start_index..start_index + max_len
}

#[cfg(test)]
mod test_super {
    use super::*;

    #[test]
    fn test_() {
        let a = b"this is a simple example";
        let b = b"simple example of a text";
        let longest = longest_common_run(a, b);
        println!("Longest common run: {:?}", std::str::from_utf8(&a[longest]).unwrap());
    }
}
