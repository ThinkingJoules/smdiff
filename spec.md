# SimpleMicroDiff (SMDIFF)
A VCDIFF inspired format for representing file deltas.

## 1. Executive Summary

**SMDIFF** is a binary delta encoding format designed for efficient representation and transmission of differences between two related data sets (source and target). It is inspired by and translatable between the VCDIFF (RFC 3284) [1] format, aiming to provide a simpler and potentially more compact alternative.


## 2. Conventions

> The basic data unit is a byte.  For portability, SMDIFF shall limit a
byte to its lower eight bits even on machines with larger bytes. The
bits in a byte are ordered from right to left so that the least
significant bit (LSB) has value 1, and the most significant bit
(MSB), has value 128.

[1]

> For purposes of exposition in this document, we adopt the convention
that the LSB is numbered 0, and the MSB is numbered 7.  Bit numbers
never appear in the encoded format itself.

[1]

SMDIFF employs two variable-length integer encoding types, both are specified from their use in Protocol Buffers. We will use the nomenclature of u-varint [2], and i-varint [3].

The i-varint format utilizes the concept of zig-zag encoding to map signed integers to unsigned values. This transformation allows negative numbers to be encoded efficiently alongside positive ones.

The resulting unsigned value is then encoded using the u-varint variable-length scheme where the most significant bit (MSB) of each byte acts as a continuation bit. If the MSB is set, it indicates that another byte follows; otherwise, it's the final byte of the encoded integer. The remaining seven bits of each byte store the integer's binary representation.

This approach offers two key advantages:

Portability: The encoding is compatible across systems using 8-bit bytes.

Compactness: Smaller integer values, both positive and negative, are encoded using fewer bytes.

Example:
Let's consider the signed integer -123456789. Using zig-zag encoding, it is mapped to the unsigned integer 246913577 and then encoded per a normal u-varint. The u and i-varint encoding of this number is as follows:
```
i-varint = -123456789 && u-varint = 246913577 ==
+-------------------------------------------+
| 10101001 | 10110100 | 11011110 | 01110101 |
+-------------------------------------------+
  0xA9       0xB4       0xDE       0x75
```

Henceforth, the terms "byte" and "u-varint" and "i-varint" will refer to a byte, an unsigned integer, and a signed integer as described.

## 3. Delta Operations
In SMDIFF, we do not divide up the source or target in to windows. Instead, any Copy operations encountered considers the entirety of the Dictionary (initial or source) file, and the entirety of the Target (output) file generated so far.

There are three types of delta operation in SMDIFF:

*   **ADD:** This operation has two arguments, a size x and a sequence of x bytes to be copied.
*   **RUN:** This operation has two arguments, a size x and a byte b, that will be repeated x times.
*   **COPY:** This operation has two arguments, a size x and an address p from either the Dictionary or the Target.

The maximum size for ADD and COPY operations is limited to `u16::MAX` (65,535 bytes). RUN operations are limited to a maximum length of 62. To encode longer lengths, multiple operation should be used.

Below are example source and target *files* and the delta operation that encode the target file in terms of the source file.

  ```
        a b c d e f g h i j k l m n o p
        a b c d w x y z e f g h e f g h e f g h e f g h z z z z

        COPY_D  4, 0
        ADD     4, w x y z
        COPY_D  4, 4
        COPY_O  4, 8
        COPY_O  4, 8
        COPY_O  4, 8
        RUN     4, z
```
COPY_D is used to indicate a copy from the source or dictionary file. The first operation copies "abcd" from the dictionary and places it in the output. COPY_O is used to copy from earlier within the output. Unfortunately the original RFC3284 does not illustrate this well as we could have used COPY_D(4,4) and repeated that operation. However, this example does illustrate our lack of periodic sequence encoding in SMDIFF.

To reconstruct the target window, one simply processes one delta operation at a time and copies the data, either from the dictionary file or the target file being reconstructed, based on the type of the operation and the associated address, if any.

## 4. Delta File Organization

An SMDIFF delta file is laid out in sections. At a high level we have:
```
Section
    Section Header
        Control Byte - byte
        Number of operations   - u-varint
        [Number of Add Bytes]  - u-varint
        Output size            - u-varint*
    Operations - array of operations
        OpByte - Byte
        [Size Indicator]
        [Additional Field] (Always present in Micro Format)
            [Copy Address] - i-varint
            [Add Bytes]    - array of bytes for this one add op
            [Run Byte]     - byte
    [Add Bytes] - array of bytes for all add ops in section
```

The output generated by any one section of operations MUST NOT exceed `u24::MAX` (16,777,215 bytes).

### 4.1 Section Header Layout
```
Section Header
    Control Byte - byte
    Number of operations   - u-varint
    [Number of Add Bytes]  - u-varint
    Output size            - u-varint*
```
#### 4.1.1 Control Byte
The first byte in the header is a byte with info about the format of the section, if it is compressed, and by what compression algorithm. The high bit when set indicates there is another section that follows.

```
     7 6 5 4 3 2 1 0
    +-+-+-+-+-+-+-+-+
    | | |     |!used|
    +-+-+-+-+-+-+-+-+
     ^ ^   ^- Compression Algo (u3)
     | +------ Format (bool)
     +-------- Continuation Bit (another section follows)
```

| Bit(s) | Field               | Description        |
| ----- | ------------------- | -------------------- |
| 0-2   | Unused    |  N/A |
| 3-5   | Compression Algo    | 0 = None, 1 = Smdiff Compress, 2 = zstd, 3 = lzma2_xz, 4-7 = unspecified
| 6     | Format              | 0 = Interleaved, 1 = Segregated Adds  |
| 7     | Continuation Bit    | 0 = Terminal Section, 1 = More Sections Follow |

**Note:** The Compression Algo fields are finalized. Values 4-7 allow for extensions for future compression algorithms that will be determined by users of the spec, much like how the VCDIFF spec works for secondary compression.

If the format bit is set to 1, the `Number of Add Bytes`, as well as the `Add Bytes` section will be present in the section where they are specified.

#### 4.1.2 Other Section Header Fields

Following the section Control Byte will be a series of u-varints.

| Element           | Type   | Description             |
| ----------------- | ------ | ----------------------- |
| Number of operations | u-varint | Number of delta operations in the window   |
| Number of Add Bytes | u-varint | Present if Format bit = 1. Total number of bytes in the ADD operations that follow the encoded operations  |
| Output size        | u-varint | Difference-encoded representation of the actual output size. The true output size is this value plus the Number of Add Bytes. If format bit = 0 then this value is the actual output size. |


### 4.2 Delta Operation Layout

The basic layout for operations in either format is:
```
Operation
    OpByte - Byte
    [Size Indicator]
    [Additional Field] (Always present in Micro Format)
```

#### 4.2.1 Operation Byte (OpByte)

Each delta operation is represented by an OpByte followed by optional size indicators and additional fields.

```
     7 6 5 4 3 2 1 0
    +-+-+-+-+-+-+-+-+
    |           |   |
    +-+-+-+-+-+-+-+-+
           ^      ^ Operation Type (u2)
           |
           +------ Size Value (u6)
```

| Bit(s) | Description                                                                 |
| ------ | --------------------------------------------------------------------------- |
| 0-1    | **Operation Type**<br>0 = CopyDict<br>1 = CopyOutput<br>2 = Add<br>3 = Run |
| 2-7    | **Size Value** See next section on interpretation of value |

#### 4.2.2 Size Indicator

If the size is *not* in the range of 1..=62 then there will be additional data to read.

This implies that we cannot allow 0 length operations, which seems logical. A Size Indicator is never present on Run Operations.

| Condition           | Type    | Description                     |
| ------------------- | ----    | ------------------------------- |
| If Size Value == 63 | u8      | Read u8 and add 62 to its value |
| If Size Value == 0  | u16_le  | Read u16, this is the size      |
| If Size Value 1..=62| null    | Size Value is the Size, no Size Indicator|

#### 4.2.3 Additional Field

Additional Field depend on the operation type and the section format.

```
Additional Field
    [Copy Address] - i-varint
    [Add Bytes]    - array of bytes
    [Run Byte]     - byte
```
| Field        | Condition            | Type        | Description        |
| ------------ | -------------------- | ----------- | ------------------ |
| Copy Address | If operation is Copy | i-varint     | CopyD = i-var-int from the last CopyDict address seen<br>CopyO = i-var-int from the last CopyOutput address seen |
| Add Bytes    | If operation is Add && Format == Interleaved  | byte[] | Bytes to be added, with length as specified |
| Run Bytes    | If operation is Run  | byte        | A single byte representing the repeated byte   |

In either format we will have exactly one of the fields and it must match the Operation Type listed in the OpByte. The exception is when the format bit in the Control Byte = 1. Then the Add Bytes field is never populated at the op level.

## 5. Delta Operation Encoding
Some differences between the SMDIFF and the VCDIFF spec is that we do not have two operations per byte (complicated instruction table), and we also do not have any special 'modes' for address encoding.

While this may make the resulting delta file larger, it greatly simplifies the spec.

### 5.1 Address Encoding
We simply use an i-varint to denote the *difference* from the last copy address used from that source. Each new section we set `last_dict_addr` and `last_out_addr` to zero. Then each time we read a Copy from either the dict or the out, we simply use the signed integer value and add it to the appropriate `last_addr`. This is the absolute offset from the start of which ever file we need to start copying from. This absolute value is then set to the appropriate `last_addr` variable for reading on the next Copy operation encountered.

## 6. Performance
My spec encoder is a (nearly) 'perfect' encoder. That is, the smdiff encoder considers everything all the time. For windowed format our minimum match might leave some matches missed (hence the 'nearly' perfect). Using micro format (not applicable to the below test), we consider the shortest profitable match possible, always.

Using the table from the original RFC. I have added something similar using the exact same data.
Using the knowledge that the encoder methods differ, we cannot really compare the outputs directly to know if our format has excessive overhead (more on that below).

What the table below does illustrate, is that window selection is very important if you want to build a more memory efficient encoder. This is evident when comparing Vcdiff-dcw against Smdiff-dcw.

Since computers have advanced greatly and RAM is cheap and still getting cheaper, I elected to build the encoder for modern computers, and not really large file sizes. This allows for better matches and a simpler encoder (no window selection logic) at the cost of not being able to do massive files.

The beauty of the VCDIFF/SMDIFF formats is that the encoder and decoder are independent from each other. So if someone wanted to do massive files they would need to write just an encoder with some sort of window selection to limit memory consumption. This is also easier to do since SMDIFF is so much simpler to comply with the spec.

### 6.1 Example Delta File Sizes
Below is the explanation from the original RFC:
```
Below are the different Vcdiff runs:

    Vcdiff: vcdiff is used as a compressor only.

    Vcdiff-d: vcdiff is used as a differencer only.  That is, it only
        compares target data against source data.  Since the files
        involved are large, they are broken into windows.  In this
        case, each target window, starting at some file offset in the
        target file, is compared against a source window with the same
        file offset (in the source file).  The source window is also
        slightly larger than the target window to increase matching
        opportunities.

    Vcdiff-dc: This is similar to Vcdiff-d, but vcdiff can also
        compare target data against target data as applicable.  Thus,
        vcdiff both computes differences and compresses data.  The
        windowing algorithm is the same as above.  However, the above
        hint is recinded in this case.

    Vcdiff-dcw: This is similar to Vcdiff-dc but the windowing
        algorithm uses a content-based heuristic to select a source
        window that is more likely to match with a given target window.
        Thus, the source data segment selected for a target window
        often will not be aligned with the file offsets of this target
        window.
```
Original Table with Smdiff comparisons
```
                gcc-2.95.1     gcc-2.95.2     gcc-2.95.3
---------------------------------------------------------
1. raw size      55,746,560     55,797,760     55,787,520
2. compress         -           19,939,390     19,939,453
3. gzip             -           12,973,443     12,998,097
4. Vcdiff           -           15,358,786     15,371,737
5. Vcdiff-d         -              100,971     26,383,849
6. Vcdiff-dc        -               97,246     14,461,203
7. Vcdiff-dcw       -              256,445      1,248,543
8. Smdiff           -           15,827,696     15,815,017
9. Smdiff-d         -               96,652        451,818
10.Smdiff-dc        -                  N/A            N/A
11.Smdiff-dcw       -               86,309        251,370
```
It is probably really only fair to compare the 'compress only' (Vcdiff & Smdiff). As noted, my test encoder is a (nearly) 'perfect' encoder.

If we want to compare the *format* we need a different approach. Since I do not have the original source code for VCDIFF enocoder, or the exact delta files used in the tables, I cannot do an exact comparison to the data in the table. From testing against other extant encoders, I converted the .vcdiff delta file directly in to .smdiff format. The resulting files were actually a little less than 1% *smaller*. They did not have any periodic sequences in the original .vcdiff format. The Smdiff output is ~3% larger than the table values for Vcdiff, so I would assume that they must had a few periodic sequences in their data.

## 7. Conclusion
My conclusion is that this new format is probably about a wash unless your data (and encoder) can leverage the periodic sequence that does not exist in SMDIFF. The spec is massively simplified, and having fixed and known secondary compressors defined in the spec will aide in interoperability.

## 8. References
[1] https://www.rfc-editor.org/rfc/rfc3284

[2] https://protobuf.dev/programming-guides/encoding/#varints

[3] https://protobuf.dev/programming-guides/encoding/#signed-ints