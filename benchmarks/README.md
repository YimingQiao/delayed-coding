# Reproducing benchmarks

No benchmark dependency is downloaded automatically. For the upstream comparison:

```sh
git clone https://github.com/rygorous/ryg_rans.git /tmp/ryg_rans
git -C /tmp/ryg_rans checkout c9d162d996fd600315af9ae8eb89d832576cb32d
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release \
  -DDELAYED_CODING_RANS_DIR=/tmp/ryg_rans
cmake --build build -j
taskset -c 2 ./build/compare_rans 4096
```

Choose an available CPU on your machine; omit `taskset` on non-Linux systems.
Run several block sizes (e.g. 16, 64, 4096, 65536). Output is CSV. Each cell is
the median of seven samples, each repeating at least 262144 symbols unless a
single block exceeds that size. No universal speed claim follows from this small
suite. Baselines use their optimized reciprocal encoders, not division-only APIs.

## Contract and limitations

- Same generated u32 symbol arrays, same normalized 16-bit weights, same process.
- Both sides write u32 symbols, avoiding an output-width mismatch. `ns/symbol` is
  the primary metric; payload bits/symbol is reported beside it.
- rANS uses unmodified `rans_byte.h` / `rans64.h` and the direct symbol lookup
  style of the upstream examples. One and four states are separate rows.
- Delayed Coding uses the Rust C ABI once per block, bounded reads and final-state
  checks. rANS's inner input reads are unchecked; this is disclosed, not hidden.
- Models and buffers are built before timing; encoder workspace is warmed and
  reused. Timings include state reset and payload finalization. The volatile
  checksum makes results observable; roundtrips are checked outside timing.
- Repeated data and models are cache-warm. Synthetic data is generated from the
  benchmark's model. This does not cover mismatched models or changing distributions.
- The C++ harness currently covers synthetic fixed models. Alias/SIMD rANS,
  real-data same-process comparisons, randomized access, many-model comparisons
  against rANS, hardware counters and whole-block model/framing cost remain work.
- Do not mix CPU flags, compilers, probability precision or lane counts without
  stating it. The initial CSVs use generic x86-64 Rust and GCC Release defaults,
  no `target-cpu=native` or `-march=native`.

The Rust-only benchmark also reports table bytes, model build time and known
round-robin switching over 1/16/256 models. It uses a different deterministic
generator from the C++ comparison, so do not compare its rows to rANS CSVs:

```sh
cargo bench --bench throughput -- 4096
# Optional real byte file, <=64 MiB:
cargo bench --bench throughput -- 4096 /path/to/file
```

## Ablation

```sh
cmake -S . -B build-division -DCMAKE_BUILD_TYPE=Release \
  -DDELAYED_CODING_RANS_DIR=/tmp/ryg_rans \
  -DDELAYED_CODING_REFERENCE_DIVISION=ON
cmake --build build-division -j
taskset -c 2 ./build-division/compare_rans 4096
cargo test --features reference-division
```

Record `rustc -Vv`, C++ compiler version, `lscpu`, CPU affinity, dependency
commits, dirty diff (if any) and command line with every result. Measure setup and
container costs separately before using numbers to choose an application codec.
