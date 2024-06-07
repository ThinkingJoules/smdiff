# smdiff-testing
This is where I throw all the more complicated tests, as well as any not-quite-benchmarking-sanity-checking-and-comparison scripts to analyze performance. Some of those results are below.



# Performance

## Compression and Secondary Compressors
Using the first two files from original VCDIFF spec (gcc-2.95.1 and gcc-2.95.2), I have ran some comparisons.
-d is source only -dcw is src+trgt, the trailing + is the secondary encoder. Settings were set to max compression.

```
                gcc-2.95.1         Encode (s) |   Decode (s) | 2.92.2 Delta | Compression (%)
-------------------------------------------------------------------------------------------
1. raw size      55,746,560    |              |              |   55,797,760 |
2. compress         -          |              |              |   19,939,390 |
3. gzip             -          |              |              |   12,973,443 |
4. zstd(22)                    |       26.554 |        0.193 |    7,983,434 |         14.308
5. brotli(best)                |       65.742 |        0.109 |    8,097,690 |         14.513
6. Vcdiff           -          |              |              |   15,358,786 |
7. Smdiff           -          |        5.148 |        0.217 |   16,594,811 |         29.741
8. xdelta3          -          |        5.200 |          -   |   13,913,641 |         24.940
9. Vcdiff-d         -          |              |              |      100,971 |
10.Smdiff-d                    |        0.735 |        0.035 |       82,296 |          0.147
11.xdelta3-d                   |        0.360 |          -   |       73,174 |          0.130
12.Vcdiff-dcw       -          |              |              |      256,445 |
13.Smdiff-dcw                  |        1.009 |        0.034 |       73,887 |          0.132
14.xdelta3-dcw                 |        0.407 |          -   |       66,687 |          0.120
(the following are with secondary compressors, to highest compression)
----------------------------+--------------+--------------+--------------+-----------------
Smdiff-d + smdiff              |        0.753 |        0.033 |        82336 |          0.148
Smdiff-d + zstd                |        0.760 |        0.036 |        41142 |          0.074
Smdiff-d + brotli              |        0.842 |        0.035 |        38198 |          0.068
Smdiff-dcw + smdiff            |        0.827 |        0.034 |        73917 |          0.132
Smdiff-dcw + zstd              |        0.848 |        0.034 |        40645 |          0.073
Smdiff-dcw + brotli            |        0.921 |        0.034 |        38040 |          0.068
xdelta3-dcw + lzma             |        0.407 |          -   |        35734 |          0.064
```

In broad strokes my encoder is not too bad for such young (and safe) codebase. The target matcher (compression) routine isn't very good and needs work. It must be missing matches somehow. The source matcher finds almost all the matches that xdelta3 does. Seems my encoder is 2.5-3x slower than xd3. Still room for improvement.


The verbose output for xdelta3 (source only delta encoding) is:
```
xdelta3 -e -9 -f -n -R -D -v -v -N  -S none -s gcc-2.95.1.tar gcc-2.95.2.tar patch.xdelta3
xdelta3: input gcc-2.95.2.tar window size 8.00 MiB
xdelta3: source gcc-2.95.1.tar source size 53.2 MiB [55746560] blksize 64.0 MiB window 64.0 MiB #bufs 1 (FIFO)
xdelta3: output patch.xdelta3
xdelta3: 0: in 8.00 MiB (30.5 MiB/s): out 9.28 KiB (35.4 KiB/s): total in 8.00 MiB: out 9.28 KiB: 262 ms: srcpos 53.2 MiB
xdelta3: 1: in 8.00 MiB (1.30 GiB/s): out 4.73 KiB (788 KiB/s): total in 16.0 MiB: out 14.0 KiB: 6 ms: srcpos 53.2 MiB
xdelta3: 2: in 8.00 MiB (1.30 GiB/s): out 4.68 KiB (780 KiB/s): total in 24.0 MiB: out 18.7 KiB: 6 ms: srcpos 53.2 MiB
xdelta3: 3: in 8.00 MiB (1.12 GiB/s): out 7.93 KiB (1.11 MiB/s): total in 32.0 MiB: out 26.6 KiB: 7 ms: srcpos 53.2 MiB
xdelta3: 4: in 8.00 MiB (888 MiB/s): out 12.3 KiB (1.33 MiB/s): total in 40.0 MiB: out 38.9 KiB: 9 ms: srcpos 53.2 MiB
xdelta3: 5: in 8.00 MiB (727 MiB/s): out 16.6 KiB (1.48 MiB/s): total in 48.0 MiB: out 55.5 KiB: 11 ms: srcpos 53.2 MiB
xdelta3: 6: in 5.21 MiB (744 MiB/s): out 15.9 KiB (2.22 MiB/s): total in 53.2 MiB: out 71.5 KiB: 7 ms: srcpos 53.2 MiB
xdelta3: scanner configuration: slow
xdelta3: target hash table size: 0
xdelta3: source hash table size: 33554432
xdelta3: finished in 360 ms; input 55797760 output 73174 bytes (0.13%)

```
The verbose output for xdelta3 (source+trgt delta encoding) is:
```
xdelta3 -e -9 -f -n -R -D -v -v -S none -s gcc-2.95.1.tar gcc-2.95.2.tar patch.xdelta3

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

The verbose output for xdelta3 (source+trgt+lzma delta encoding) is:
```
xdelta3 -e -9 -f -n -R -D -v -v -S lzma -s gcc-2.95.1.tar gcc-2.95.2.tar patch.xdelta3
xdelta3: input gcc-2.95.2.tar window size 8.00 MiB
xdelta3: source gcc-2.95.1.tar source size 53.2 MiB [55746560] blksize 64.0 MiB window 64.0 MiB #bufs 1 (FIFO)
xdelta3: output patch.xdelta3
xdelta3: 0: in 8.00 MiB (28.7 MiB/s): out 6.52 KiB (23.4 KiB/s): total in 8.00 MiB: out 6.52 KiB: 279 ms: srcpos 53.2 MiB
xdelta3: 1: in 8.00 MiB (0.98 GiB/s): out 2.70 KiB (337 KiB/s): total in 16.0 MiB: out 9.22 KiB: 8 ms: srcpos 53.2 MiB
xdelta3: 2: in 8.00 MiB (0.98 GiB/s): out 2.40 KiB (300 KiB/s): total in 24.0 MiB: out 11.6 KiB: 8 ms: srcpos 53.2 MiB
xdelta3: 3: in 8.00 MiB (727 MiB/s): out 3.40 KiB (308 KiB/s): total in 32.0 MiB: out 15.0 KiB: 11 ms: srcpos 53.2 MiB
xdelta3: 4: in 8.00 MiB (533 MiB/s): out 5.11 KiB (340 KiB/s): total in 40.0 MiB: out 20.1 KiB: 15 ms: srcpos 53.2 MiB
xdelta3: 5: in 8.00 MiB (470 MiB/s): out 5.25 KiB (308 KiB/s): total in 48.0 MiB: out 25.4 KiB: 17 ms: srcpos 53.2 MiB
xdelta3: 6: in 5.21 MiB (372 MiB/s): out 9.53 KiB (680 KiB/s): total in 53.2 MiB: out 34.9 KiB: 14 ms: srcpos 53.2 MiB
xdelta3: scanner configuration: slow
xdelta3: target hash table size: 8388608
xdelta3: source hash table size: 33554432
xdelta3: finished in 407 ms; input 55797760 output 35734 bytes (0.06%)

```


Should probably setup some more standard tests like all zeros, then all random, then some text, then some binary. I suspect different encoders will do better given different content.



Target only (compression) stats
```
xdelta3 -e -9 -f -n -R -D -v -v  -S none gcc-2.95.2.tar patch-comp.xdelta3
xdelta3: input gcc-2.95.2.tar window size 8.00 MiB
xdelta3: output patch-comp.xdelta3
xdelta3: 0: in 8.00 MiB (8.62 MiB/s): out 2.38 MiB (2.57 MiB/s): total in 8.00 MiB: out 2.38 MiB: 928 ms: srcpos 0 B
xdelta3: 1: in 8.00 MiB (8.46 MiB/s): out 2.38 MiB (2.51 MiB/s): total in 16.0 MiB: out 4.76 MiB: 946 ms: srcpos 0 B
xdelta3: 2: in 8.00 MiB (9.86 MiB/s): out 1.95 MiB (2.40 MiB/s): total in 24.0 MiB: out 6.71 MiB: 811 ms: srcpos 0 B
xdelta3: 3: in 8.00 MiB (10.5 MiB/s): out 1.94 MiB (2.55 MiB/s): total in 32.0 MiB: out 8.65 MiB: 761 ms: srcpos 0 B
xdelta3: 4: in 8.00 MiB (11.8 MiB/s): out 1.72 MiB (2.55 MiB/s): total in 40.0 MiB: out 10.4 MiB: 676 ms: srcpos 0 B
xdelta3: 5: in 8.00 MiB (12.2 MiB/s): out 1.81 MiB (2.77 MiB/s): total in 48.0 MiB: out 12.2 MiB: 654 ms: srcpos 0 B
xdelta3: 6: in 5.21 MiB (13.0 MiB/s): out 1.09 MiB (2.71 MiB/s): total in 53.2 MiB: out 13.3 MiB: 401 ms: srcpos 0 B
xdelta3: scanner configuration: slow
xdelta3: target hash table size: 8388608
xdelta3: finished in 5.2 sec; input 55797760 output 13913641 bytes (24.94%)
```