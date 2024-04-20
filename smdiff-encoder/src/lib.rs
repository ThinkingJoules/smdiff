use smdiff_common::AddOp;



mod run;
mod copy;
mod suffix;
mod window_encoder;
/*
The biggest challenge is memory management.
While this is optimized for small files, we need to be able to handle large files.
And 'large' is relative to the available memory.
I want to make sure this is adaptable to different memory constraints.

To this end we should first talk a bit about the nature of delta encoding.

We want to generate a particular output, with or without a 'starting state' (the source).
If we have a source file, we should think of this as a 'dictionary'.
We can reference it to generate Copy commands that are very small relative to the number of bytes copied.

It's all about the ratios. Our delta file is smallest when the ratio of the Copied length to the Operation encoded size is largest.
We get long lengths by having a dictionary that has lots of long sequences that are exactly found in the output we want to generate.

The problem arises when we cannot fit the whole dictionary in memory.
Because we need to match any substring, this requires either a lot of memory,
or imperfect matches using other methods (rolling hash).

This encoder attempts to do both.
Because the smdiff spec is designed to be simple and small oriented, we can make some assumptions.
The max output size for a window in window mode is 2^24 (16MB). This is what we will work around.

The assumption is that we can always fit the entire output window in to memory.
Even if the overhead is 16x, this is still only 256MB. This is reasonable.

The dictionary is a bit more tricky. In theory we want to consider the whole of the input.
But if the input is large, or we don't have much additional memory available, we need to have a strategy.

We need to be smart but also not take too long to encode.
The Dictionary is the biggest factor in our compression ratio, so we must take it seriously.

If the Dictionary cannot fit in memory we need to determine a few things:
1. How much of the dictionary can we fit in memory?
2. How do we select the best parts of the dictionary to fit in memory?

The first is implementation based and we should be able to calculate how many bytes we can fit in memory given how we handle them.
The second is a bit more tricky. We need to be able to select the best parts of the dictionary to fit in memory.

Since we know the encoding of the output is linear and start with the first 16MB (at most) we need to select based on those bytes.
We need to find a dictionary window that has the best copy ratios.
This could be tunable, as wanting to have 'best' compression might take longer to find better windows.
I think some sort of 'time-based' (cycles technically) limit would be good.
We basically search the best we can in the 'time' we have.

To me this feels like number of different rolling hashes we can calculate in a given time.
This too is also memory bound, so it needs to fit within the memory constraints that will eventually be used for the dictionary indexing.

The longer matches provide the best compression.
So we need to model scoring of a window based on the total windows match length vs its number of copy operation overhead.
Basically if a window had lots of short matches (as most will, statistically) it would be less valuable than a window with a few long matches.
Generating a score that accurately predicts the best window is the goal.

If we assume our hash is 8 bytes then we will have 8x overhead on the length of the *target* to find the reference hashes.
So depending on available memory, we determine how many different hashes we can store as a reference.
For illustration, lets say we have another 256MB available to use for the dictionary.
This would allow us to store 2 hashes (16 bytes per trgt byte) for scoring against.
Clearly memory is the limiting factor here.
Which two lengths to we choose? Long are better, but we might not match any.
Since we will match every suffix, long matches will be capture indirectly by lots of short matches.
So we still have some sort of signal.

So maybe MIN_MATCH_BYTES and MIN_MATCH_BYTES*2 are good choices?
Since we don't know the actual address, we can just use the cost to encode the absolute address value.
Then we can score each match using the addr length as 'negative' and the match length as 'positive'.
We also need to consider the total match length not just the relative scores.
A window that doesn't have any matches might have the best score, but it is useless.

Scoring Rules:
Calculate Average Positive Score: Compute the average of the positive scores from all windows.
Thresholding Positive Scores: Consider only those windows where the positive score exceeds this average. (filter empty windows with high composite scores)
Optimizing Composite Score: From the subset identified in the previous step, select the window that has the highest composite score.

Then we can roll through the src file and start scoring it. This should be pure computation.

The time-based would allow us to pick a second set of hashes to score against.
We really want the best window we can find in the time we have, not just 'a complete' round trip of the src file.
Not sure how that would work in practice. To start we will just do one set of hashes and pick from that.
Would need more benchmarking as I'm not familiar with the performance characteristics of the rolling hash.

Once we have selected a window, we can then start encoding the output.
This would just use the encode_window fn that will generate a suffix array for the chosen dictionary and the trgt window.
We keep doing the above process until we reach the end of the target file.


*/
include!(concat!(env!("OUT_DIR"), "/memory_config.rs"));

const MIN_MATCH_BYTES: usize = 2; //two because we are trying to optimize for small files.
const MIN_ADD_LEN: usize = 2; //we need to have at least 2 bytes to make an add instruction.

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct Add <'a> {
    bytes: &'a [u8],
}
impl AddOp for Add<'_> {
    fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}
pub type Op<'a> = smdiff_common::Op<Add<'a>>;