





## Notes on Secondary Compressors
In theory compression algorithms should know when they have reached the end of the compressed data to decompress. My issues were finding rust libraries that didn't simply read to EOF.

For some reason the zstd rust lib doesn't behave well. I enabled single frame, but it keeps trying to read non-compressed data. I ended up using the ruzstd crate.