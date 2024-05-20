# smdiff
This project is an attempt at simplifying the VCDIFF spec without sacrificing too much.

# Rationale
There are only two real implementations of VCDIFF in existence. From my research and messing with them, the two implementations don't seem to be fully compatible with each other. This is due to the complexity of the VCDIFF specification. To this end, I wanted to simplify the spec so it is easier to ensure different implementations of a delta format could be used together. Thus I created SMDIFF.

# Differences
Mostly: I went without super string U for windows.

All windows are effectively `source_segment_position: 0` and `source_segment_size: size of file`. This change allows us to mix Copy operations from both the Dictionary (initial file) and the Output (target file) in a single window.

## Windows
Windows are now just output buffer boundaries. They really don't serve any purpose except to limit any single group of instructions. This allows compression of the windows independently.

## Micro Format
I also introduced a different format that is used for extremely succinct patch files. These can technically be appended one after another as windows if you want to interleave the add bytes with the operations (Window format puts all the add bytes after all the operations). A patch file cannot contain a mixture of formats in the same file.

## No Sequence
I did away with the implicit sequence operation. I don't think either extant VCDIFF encoder implementations use them, and it really complicates the logic. Copy either comes from the Dictionary (initial) file or from the Output (target) file.

## Max sizes
Things have max sizes that are part of the spec. This makes it easier to reason about worst case encoding choices when trying to write a decoder.

# Spec
For the full spec see [spec.md]