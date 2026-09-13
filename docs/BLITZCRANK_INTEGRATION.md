# Blitzcrank integration design

Status: design for the next implementation milestone, not an implemented runtime
migration. Reference: Blitzcrank `0ed9c97908c51440b30a2eef3c1b90325dd2c87c`.
The integration worktree is isolated from the owner's modified research checkout.

## Why linking the block API is insufficient

The current CMake target and fixed-model C ABI already work as a downstream
dependency. However, Blitzcrank chooses conditional models while decoding fields,
and its semantic models use both alias mappings and simple contiguous intervals.
These operations cannot all be replaced with one fixed-model block call.

| Existing operation | Required standalone primitive |
| --- | --- |
| `categorical_model.cpp` primary branch | Immutable alias model and stateful model-selected symbol read |
| Categorical rare branch (`word / weight`) | Contiguous interval / equal-width partition, preserving legacy mapping |
| `numerical_model.cpp` histogram head | Alias model, then context-dependent following operations |
| Numerical exponential tail (high bit) | Two equal contiguous intervals, not a reconstructed alias ordering |
| Histogram residual and raw words | Equal-width intervals; frequency-one raw-word fast path |
| `timeseries_model.cpp` parameter words | Raw-word operations without a 65,536-symbol lookup allocation |
| Record/block boundaries and `ByteReader` | Bounded payload slices and externally owned offset/index metadata |

The original decoder separates `Read16Bits` and `Update`. Frequency-one raw reads
omit `Update` because multiplying by one and adding zero changes no state. A new
adapter must preserve this behavior, especially when a pending virtual word is
consumed. Rebuilding every interval as an alias model can change word ordering;
it is not a bit-exact migration strategy.

## Implementation sequence

1. Add a safe contiguous-interval encoding event alongside existing model events.
   Validate nonzero frequency and `start + frequency <= 65536`; avoid constructing
   huge identity tables for raw words. Keep existing Rust APIs source-compatible.
2. Add bounded stateful decoding for model symbols and equal-width partitions.
   Every operation performs normalization, consumes one logical word, validates
   its contribution and updates the state. Do not expose an unchecked caller-
   writable numerator/denominator structure. Validate unused partition tails.
3. Expose opaque C decoder handles and mixed-event batch encoding. Specify input
   buffer lifetime, output/error behavior, finalization, handle ownership and
   model lifetime. Keep all pointer handling inside the FFI crate.
4. Add mixed alias/interval/raw-word differential tests against the unmodified
   original encoder. Include transitions after virtual words, rare branches,
   conditional decisions, truncation and independent record reset.
5. Implement RAII C++ wrappers on `improve-delayed-coding`, selected by an opt-in
   CMake backend flag and a pinned standalone revision. Keep the research backend
   as the default until end-to-end acceptance. No silent file-format replacement.
6. First migrate categorical-only records, then numerical/rare paths, then JSON
   and time series. Check full reconstruction and random seeks, not only kernel
   roundtrips. Preserve model learning, serialization and record indexes in Blitzcrank.

## Performance acceptance

Measure native Rust, block C ABI and per-symbol C ABI separately with the same
conditional sequence. Small-record latency includes dispatch, finalization and
index lookup. Model setup, memory and many-model cache behavior also matter.
A per-symbol FFI adapter may be correct but too slow; if so, retain it as a
reference and measure record-level batches before selecting a production path.

Do not select four-state coding just because a fixed-model loop is faster. Field
dependencies, deterministic lane assignment, record size and random access must
be tested together. Any new interleaved container convention stays versioned and
experimental; historical scalar delay-24 compatibility remains a separate gate.
