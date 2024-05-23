# smdiff
This project is an attempt at simplifying the VCDIFF spec without sacrificing too much.

# Rationale
There are only two real implementations of VCDIFF in existence. From my research and messing with them, the two implementations don't seem to be fully compatible with each other. This is due to the complexity of the VCDIFF specification. To this end, I wanted to simplify the spec so it is easier to ensure different implementations of a delta format could be used together. Thus I created SMDIFF.

# Differences
Mostly: I went without super string U for windows.

All windows are effectively `source_segment_position: 0` and `source_segment_size: size of file`. This change allows us to mix Copy operations from both the Dictionary (initial file) and the Output (target file) in a single 'window' (called sections in SMDIFF).

## Windows
Windows are now just output buffer boundaries (called sections). They really don't serve any purpose except to limit output size for any single group of instructions.

## Formats
I baked in the 'interleaved' vs 'segregated' flags that Google's open-vcdiff C++ impl added. Segregated usually helps secondary compressors achieve better compression of the unmatched Add bytes, but if you don't have very many, then it is better to just interleave them.

## No Sequence
I did away with the implicit sequence operation. I don't think either extant VCDIFF encoder implementations use them, and it really complicates the logic. Copy either comes from the Dictionary (initial) file or from the Output (target) file.

## Max sizes
Things have max sizes that are part of the spec. This makes it easier to reason about worst case encoding choices when trying to write a decoder.

# Spec
For the full spec see [spec.md]

# Performance
The reference encoder is a memory hog. To keep it simpler to build I skipped window selection and put it all into RAM. This means the reference encoder can do a file as large as you have RAM for (there is a lot of overhead, so not too big of file). On large files (120mb) the reference encoder beats either xdelta3 or open-vcdiff, but I must have something a miss since mine is slower for files around 50mb in size. Needs some work, but those other two encoders had a lot more time and energy put in to them. xdelta3 is really fast and gets good matches. However, I can't seem to decode those files using open-vcdiff.


If this works for you, then great, you also gain parallelization for super throughput. If not, you need to write an encoder or use a VCDIFF encoder and translate it to SMDIFF (see the smdiff-vcdiff translator crate).