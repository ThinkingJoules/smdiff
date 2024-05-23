# smdiff-testing
This is where I throw all the more complicated tests, as well as any not-quite-benchmarking-sanity-checking-and-comparison scripts to analyze performance. Some of those results are below.



# Performance

## Compression and Secondary Compressors
Using the first two files from original VCDIFF spec (gcc-2.95.1 and gcc-2.95.2), I have ran some comparisons. The encoding routines are different, and I am using secondary encoders, so the VCDIFF values are just for reference. My encoding sizes are pretty good, but I need to work on the performance.

```
                gcc-2.95.1      Encode (s) |   Decode (s) | 2.92.2 Delta | Compression (%)
-------------------------------------------------------------------------------------------
1. raw size      55,746,560                                   55,797,760
2. compress         -                                         19,939,390
3. gzip             -                                         12,973,443
4. zstd(22)                 |       26.554 |        0.193 |    7,983,434 |         14.308
5. brotli(best)             |       65.742 |        0.109 |    8,097,690 |         14.513
6. Vcdiff           -                                         15,358,786
7. Smdiff           -       |        8.318 |        0.162 |   15,827,699 |         28.366
8. Vcdiff-d         -                                            100,971
9. Smdiff-d                 |        2.643 |        0.078 |       96,655 |          0.173
10.Vcdiff-dcw       -                                            256,445
11.Smdiff-dcw               |        5.109 |        0.050 |       86,312 |          0.155
(the following are with secondary compressors, to highest compression)
----------------------------+--------------+--------------+--------------+-----------------
Smdiff-d + smdiff           |        2.605 |        0.051 |       78,311 |          0.140
Smdiff-d + zstd             |        2.590 |        0.041 |       59,084 |          0.106
smdiff-d + brotli           |        2.728 |        0.041 |       52,835 |          0.095
smdiff-dcw + smdiff         |        5.030 |        0.018 |       83,406 |          0.149
smdiff-dcw + zstd           |        5.053 |        0.073 |       64,883 |          0.116
smdiff-dcw + brotli         |        5.195 |        0.053 |       58,554 |          0.105
```

For reference xdelta 3 does -d in only 407ms with a vcdiff output of 66,687. Something to strive for I suppose. Currently 30% larger and about 6x slower. If you use lzma as secondary and use max setting (-9) it is only 33,734 output size, and about the same speed.

However I have tested on larger files (317.iso -> 318.iso) and my encoder beats it my a few seconds. So mine doesn't quite scale down as well.

The verbose output for xdelta3 is:
```
xdelta3 -e -9 -f -n -R -D -v -v  -S none -s gcc-2.95.1.tar gcc-2.95.2.tar patch.xdelta3

xdelta3: input gcc-2.95.2.tar window size 8.00 MiB
xdelta3: source gcc-2.95.1.tar source size 53.2 MiB [55746560] blksize 64.0 MiB window 64.0 MiB #bufs 1 (FIFO)
xdelta3: output patch.xdelta3
xdelta3: 0: in 8.00 MiB (28.3 MiB/s): out 8.56 KiB (30.2 KiB/s): total in 8.00 MiB: out 8.56 KiB: 283 ms: srcpos 53.2 MiB
xdelta3: 1: in 8.00 MiB (1.12 GiB/s): out 4.31 KiB (615 KiB/s): total in 16.0 MiB: out 12.9 KiB: 7 ms: srcpos 53.2 MiB
xdelta3: 2: in 8.00 MiB (1.12 GiB/s): out 4.51 KiB (644 KiB/s): total in 24.0 MiB: out 17.4 KiB: 7 ms: srcpos 53.2 MiB
xdelta3: 3: in 8.00 MiB (666 MiB/s): out 7.67 KiB (639 KiB/s): total in 32.0 MiB: out 25.1 KiB: 12 ms: srcpos 53.2 MiB
xdelta3: 4: in 8.00 MiB (533 MiB/s): out 11.5 KiB (766 KiB/s): total in 40.0 MiB: out 36.5 KiB: 15 ms: srcpos 53.2 MiB
xdelta3: 5: in 8.00 MiB (533 MiB/s): out 15.6 KiB (1.02 MiB/s): total in 48.0 MiB: out 52.2 KiB: 15 ms: srcpos 53.2 MiB
xdelta3: 6: in 5.21 MiB (400 MiB/s): out 13.0 KiB (996 KiB/s): total in 53.2 MiB: out 65.1 KiB: 13 ms: srcpos 53.2 MiB
xdelta3: scanner configuration: slow
xdelta3: target hash table size: 8388608
xdelta3: source hash table size: 33554432
xdelta3: finished in 407 ms; input 55797760 output 66687 bytes (0.12%)
```

Going to have to figure out how they do it (besides a bunch of unsafe C that I can't do).

Should probably setup some more standard tests like all zeros, then all random, then some text, then some binary. I suspect different encoders will do better given different content.