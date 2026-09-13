# Initial measurements — 2026-09-12

Machine: Intel Xeon Platinum 8474C, Linux x86-64. CPU affinity: logical CPU 2.
Rust 1.92.0; GCC 12.2.0, CMake Release. Rust release uses thin LTO and one codegen
unit. No native-CPU flags. rANS upstream:
`c9d162d996fd600315af9ae8eb89d832576cb32d`.

These are cache-warm kernel measurements with fixed models and 4096 u32 symbols
per block. The process has a Rust/C block boundary for Delayed Coding. Seven
samples per metric; report the median. See [methodology](README.md) before using
these numbers. This is not an application-level or SIMD comparison.

## Findings

The first safe Rust extraction showed a clear encoding bottleneck: linear scans
over the many disjoint alias segments of a high-frequency symbol. The early
development snapshot took about 40 ns/symbol on `skewed` and 66 ns/symbol on
`near_constant` at delay 24. Binary search lowered those to approximately 15 and
31 ns/symbol. A direct encode table lowered them further to about 8 and 9 ns,
at an additional 128 KiB/model. Uniform inputs did not benefit from the large table.

Four independent coding states reduce the decoder's serial dependence, and share
one stream without lane-length headers. The initial interleaved implementation
still trails four-state rANS. Direct decode tables have a 512 KiB/model cost and
can regress scalar decoding, so they remain opt-in.

From `results/2026-09-12-xeon-8474c-4096-interleaved.csv`:

| Distribution | Delayed24 scalar decode ns/symbol | Delayed24 4-state, direct tables | rANS64 4-state | Delayed/rANS64 payload bytes (4-state) |
| --- | ---: | ---: | ---: | ---: |
| uniform256 | 4.5402 | 2.9175 | 2.1883 | 4104 / 4128 |
| uniform16 | 6.1541 | 2.8879 | 2.1235 | 2064 / 2080 |
| skewed | 5.7652 | 3.0307 | 2.1653 | 2550 / 2556 |
| near_constant | 7.8079 | 2.9232 | 2.1101 | 40 / 52 |

These synthetic cases suggest useful short-block overhead and scalar-decoding
properties; they do not establish a general advantage over rANS. In particular,
rANS encoding remains considerably faster in this initial suite. Next work is
to shorten the virtual-word/state path, measure table footprint and cache effects,
and test alias/SIMD rANS, real data and whole-block costs.

The `initial`, `binary`, `tables` and `interleaved` CSVs were collected during
development before the first repository commit. They document experiment history,
not four separately versioned releases. Subsequent results should identify an
exact source revision and any dirty patch in a run manifest.

## Deferred normalization (2026-09-13)

Starting from `e9634dd`, normalization moved to the next read of a state. The
denominator encodes whether a virtual word exists, and the numerator holds its
bits; a separate Option and the second branch were unnecessary. All property
and original-C++ differential checks still pass.

`results/2026-09-13-xeon-8474c-4096-deferred.csv` records the same harness, CPU and
flags. Four-state compact decoding fell from about 3.1–3.3 to 2.58–2.64 ns/symbol
across these four distributions. This is approximately 17–19% less time, without
the 512 KiB decode table. Four-state rANS64 remained around 2.11–2.22 ns/symbol.
Scalar throughput was largely unchanged. This is a measured implementation
improvement, not a new compressed format.

A subsequent chunked-loop experiment attempted to expose fixed lane indices to
the optimizer. It regressed four-state decoding to roughly 3.4–3.9 ns/symbol in
this build and was reverted. The `2026-09-13-...-batched.csv` file retains those
negative results. The simpler per-symbol loop remains the default.
