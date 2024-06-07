# SMDIFF Encoder
A decent encoder for a second attempt. My first attempt was naive and I learned a lot while doing it. I tried to apply this in the second iteration of this encoder. This one is about twice as fast my first attempt and uses *way* less memory. However, it is not as good as xdelta3 for speed or matches, but it is in readable, fully safe Rust. From rough benchmarks it is about 2-3 times slower than xdelta3 and is on par for finding src matches with xdelta3 (with no secondary compressor). Using just pure compression, this encoder isn't great. It's speed is about the same as xd3, but somehow it isn't finding all the good matches, so it needs some work (about 20% larger delta file). So not great, but this is pretty young code. Secondary compressors bring it closer to being inline with xd3 with secondary compressors (in size, not speed).

## Improvements

The main encoders for both src and trgt use a similar design, so improvements to one will improve both. I tried to keep them efficient. They are broadly modelled on how xdelta3 approaches finding matches. However, I must not have something quite right as I don't find the same good matches that xdelta3 does. The match algorithm however sort of feeds on itself: better matches lead to better matches due to the nature of how the encoder advances and assess each position in the output stream. So finding somewhat better matches will up the likelihood of finding better matches. The speed of the encoding process also gets faster with finding better matches (we assess less positions). Thus getting good matches is doubly important. Apparently my current design has some issues with the hashing and/or table storage. I'm a little puzzled why my target matcher (compressor) is so bad compared to xd3, while the source matcher is nearly the same (but also not *quite* as good). This leads me to believe I have something slightly off.

I think the main speed improvements would come from having a more complex build process and lots more constants. That is basically what xd3 does, and it makes it really hard to read. Basically xd3 has a specific encoder for different presets, where mine is configured at runtime. I'm sure there is serious performance there, but for now this is easy to follow for the performance it does get.


## API
It would probably make sense to try to make a stream interface. There isn't really a point as the current encoder puts everything in memory. I think it would make the API clunky, adding a layer of indirection at the moment. The code would need to integrate reading the input in chunks and processing as it goes. I have already made the API take Readers/Writers in preparation for this. Ideally we would instantiate a writer wrapper that wraps a Src file (optional) as well as the patch output Writer. Then we feed in the trgt file. Not sure exactly the best ergonomics, hence just leaving things as is.



# Rough Performance
This is not done with any proper benchmarking, but aims to illustrate the various encoder/compression speeds and compression values.

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