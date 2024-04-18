use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("memory_config.rs");
    let mut f = File::create(&dest_path).unwrap();

    let max_memory = env::var("MAX_MEMORY").unwrap_or_else(|_| "128".to_string());
    // parse the string and interpret it as mb
    let max_memory: usize = max_memory.parse().unwrap();
    let max_memory = max_memory * 1024 * 1024;
    // Assuming the environment variable is always valid integer
    writeln!(f, "pub const MAX_MEMORY: usize = {};", max_memory).unwrap();
}
