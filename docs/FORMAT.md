# Payload and compatibility policy (experimental 0.x)

The public API currently emits raw entropy payloads, not files. An application
must retain all of the following to decode a block:

| Metadata | Contract |
| --- | --- |
| Model | Identical symbol IDs and normalized integer frequencies; same alias layout |
| Delay | Same threshold, independent of fixed 16-bit probability precision |
| Lanes | Same round-robin state count |
| Symbol count | Externally supplied; output storage is bounded by the caller |
| Payload bytes | Exact length and location; big-endian 16-bit words |

Each independent block resets coder states to numerator=0, denominator=1.
An empty symbol sequence emits zero bytes. Model-table choices and reciprocal
versus division encoding do not alter bytes. Interleaving and delay changes do.

Scalar D=24 is tested byte-for-byte against the original Blitzcrank entropy
implementation at `0ed9c97` for valid normalized models. This does **not** claim
compatibility with an entire Blitzcrank file: model serialization, dictionaries,
indexes and block boundaries belong to Blitzcrank. The existing input/output
files and original checkout must not be rewritten during migration.

Until a release freezes the format, pin the full Git revision for both writer and
reader. Before persistent storage is supported as a stable contract, define a
versioned container with codec/configuration IDs, byte order, lengths, model
identity/serialization and corruption detection. A future format change must be
explicitly versioned and retain fixtures for readers of supported older versions.

`finish()` checks consumed length and zero terminal numerator state (including a
pending virtual word). These conditions cannot detect every wrong model, wrong
symbol count or corrupted payload. Add a checksum or authenticated container
appropriate to the application; set output limits before decoding. On an error,
output can contain a decoded prefix and should be discarded.

The C ABI has the same payload rules. Its pointers, handle ownership and
non-aliasing contracts are documented in the header. Null/nonzero-length arguments
are rejected, but no FFI can establish that an arbitrary non-null pointer actually
addresses live memory. The core crate forbids unsafe code; unsafe operations are
isolated in `ffi/`.
