# smdiff-merger

smdiff-merger is a library that provides utilities for merging SMDIFF files. SMDIFF (Delta) is a format for encoding differences between two files, commonly used for efficient binary patching. It is a simplified version of VCDIFF.

## Features
Used to create a summary patch between 2 or more patches.

This uses a Merger struct that will allow for early termination. Basically, if a merge patch no longer contains any Copy instructions, merging more patches will have no effect.

## Improvements
This does not try to use CopySrc::Output for long runs. It might be an optimization to include (see the encoder, it uses this trick). So byte runs longer than ~200 bytes would probably benefit. If we ever merge a long run from the encoder, we will output a ton of Run ops that are the max length. This is probably only worth the effort when we go to `write` to the delta format. If someone just want to use the `take_ops` directly, then having extra ops is probably fine.