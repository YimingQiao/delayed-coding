# Coverage-guided fuzzing

The fuzz harness is a separate package: runtime users do not depend on libFuzzer.
It exercises malformed bounded decoding and valid roundtrips across random models,
table modes, delay settings and lane counts. Input/output sizes are capped so
fuzzer effort is spent on behavior rather than arbitrary allocations.

```sh
rustup toolchain install nightly --profile minimal
cargo install cargo-fuzz --locked
cargo +nightly fuzz run codec -- -max_total_time=60 -max_len=4096
```

Keep and minimize any crashing inputs. Add deterministic regression tests before
fixing a bug. No crash in a bounded fuzz run is not a proof that all malformed
streams are rejected: payloads do not include a checksum or metadata.

The C ABI's valid-pointer ownership and buffer paths can also be checked with Miri:

```sh
rustup component add miri --toolchain nightly
cargo +nightly miri test -p delayed-coding-ffi --test api
```
