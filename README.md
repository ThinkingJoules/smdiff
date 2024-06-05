# smdiff
This project is an attempt at simplifying the VCDIFF spec without sacrificing too much.

# Rationale
There are only two real implementations of VCDIFF in existence. From my research and messing with them, the two implementations don't seem to be fully compatible with each other. This is due to the complexity of the VCDIFF specification. To this end, I wanted to simplify the spec so it is easier to ensure different implementations of a delta format could be used together. Thus I created SMDIFF.

# Differences
Mostly: I went without super string U for windows.

All windows are effectively `source_segment_position: 0` and `source_segment_size: size of file`. This change allows us to mix Copy operations from both the Dictionary (initial file) and the Output (target file) in a single 'window' (called sections in SMDIFF).

## Windows
Windows are now just output buffer boundaries (called sections). They really don't serve any purpose except to limit output size for any single group of instructions. Secondary Compression occurs inside of each section.

## Formats
I baked in the 'interleaved' vs 'segregated' flags that Google's open-vcdiff C++ impl added. Segregated usually helps secondary compressors achieve better compression of the unmatched Add bytes, but if you don't have very many, then it is better to just interleave them.

## No Sequence
I did away with the implicit sequence operation. I don't think either extant VCDIFF encoder implementations use them, and it really complicates the logic. Copy either comes from the Dictionary (initial) file or from the Output (target) file.

## Max sizes
Things have max sizes that are part of the spec. This makes it easier to reason about worst case encoding choices when trying to write a decoder.

# Spec
For the full spec see [./spec.md]

# Performance
The reference encoder is decent. It isn't as good as xdelta3, but it is way easier to read, and is in 100% safe Rust.

If this works for you, then great. If not, you need to write an encoder or use a VCDIFF encoder (not xdelta3 though) and translate it to SMDIFF (see the smdiff-vcdiff translator crate).

Note: I can't seem to read xdelta3 per the spec, so either I'm missing something or there is a bug. It would be really nice to be able to translate xd3 patches for when you need a wicked fast encoder. I suspect the issue is either in how I am decoding the address cache modes, or how xd3 is encoding them. I can decode open-vcdiff patch files, so I suspect xd3 is encoding the address value incorrectly. The other explanation is that open-vcdiff doesn't use all the modes, and I have an error in how I'm reading the modes that xd3 uses. I haven't dived deep in to the open-vcdiff src like I have xd3, but I still suspect xd3 has an error.