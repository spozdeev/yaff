# YaFF for Rust

The `yaff` crate reads and writes the existing C++ YaFF wire format. It has no
third-party runtime dependencies. Its default configuration forbids unsafe Rust;
an explicit `trusted` feature enables a separate low-level reader. Message views borrow
their input; strings, arrays and nested messages do not copy payloads. Scalars
are decoded by value with explicit little-endian reads, including unaligned data.

Rust 1.85 or later is required. Supported interoperability targets are
little-endian C++ producers/consumers such as x86-64 and AArch64. See the
[wire format specification](../docs/wire-format.md).

## Components

* `yaff`: checked readers, writers, typed offsets and array views.
* `yaff-build`: a dependency-free Cargo wrapper around `protoc` and
  `protoc-gen-yaff-rust`.
* `interop`: development-only generated API tests, mutation fuzzing and read
  benchmarks. Excluded from default workspace members so ordinary `cargo test`
  does not need a C++ build.

The generator is a backend of the existing C++ compiler. Both backends use the
same protobuf frontend, slice selection, defaults and layout metadata. Generated
Rust programs do not link C++, Protobuf or the generator.

## Generate from .proto

Build/install YaFF using the existing [CMake or Conan integration](../docs/integration.md).
Protobuf and its dependencies must be discoverable by CMake, as for C++ YaFF:

```sh
cmake -S . -B build/local -DCMAKE_BUILD_TYPE=Release
cmake --build build/local --target yaff_rust_protoc_plugin --parallel
```

For a local consumer, add path dependencies pointing at this checkout:

```toml
[dependencies]
yaff = { path = "/path/to/yaff/rust/yaff" }

[build-dependencies]
yaff-build = { path = "/path/to/yaff/rust/yaff-build" }
```

Example `build.rs`:

```rust,ignore
fn main() {
    yaff_build::Config::new()
        .plugin("/path/to/yaff/build/local/src/protoc-plugin/protoc-gen-yaff-rust")
        .compile(&["proto/feed.proto"], &["proto", "/path/to/yaff/include"])
        .unwrap();
}
```

Include the result **at crate root**, since generated imported-type references
are absolute paths within the crate:

```rust,ignore
include!(concat!(env!("OUT_DIR"), "/yaff.rs"));
```

All specified schemas and their referenced imports are emitted into one module
tree. Files in the same protobuf package share a module. By default package
`feed` becomes `crate::protoyaff::feed`. Nested message names use the same
flattening as the C++ IR. Rust keywords are escaped, and conflicting helper names
receive an underscore suffix. Accessors have the form `get_<proto_field_name>`;
the original field spelling is preserved.

`Config` also accepts `protoc`, `out_dir`, `namespace`, `slice` and `output`.
Environment alternatives are `PROTOC` and `YAFF_PROTOC_GEN_RUST`. An installed
plugin on `PATH` needs no explicit configuration. Generate multiple slices with
distinct `namespace` and `output` values, then include each file at crate root.

Direct protoc use is equivalent:

```sh
protoc -I proto -I /path/to/yaff/include \
  --plugin=protoc-gen-yaff-rust=/path/to/protoc-gen-yaff-rust \
  --yaff-rust_out=generated proto/feed.proto
```

Plugin options are `namespace`, `tag` (slice), `output` and `trusted`, passed through
`--yaff-rust_opt`. Generation supports proto2 and proto3, including optional
presence, oneof, enums/aliases, maps, imports, reserved/deprecated fields and YaFF
options. As with the existing C++ plugin, protobuf editions support is not
advertised. Well-known types are regular generated messages.

## Read and write

For `message Update { uint64 id = 1; optional string title = 2; }`:

```rust,ignore
use protoyaff::feed::{Update, UpdateBuilder};

let mut writer = yaff::Writer::new();
let title = writer.string("hello")?;
let root = UpdateBuilder { id: 42, title: Some(title) }.write(&mut writer)?;
let bytes = writer.finish(root)?;
let view = Update::read(&bytes)?;
assert_eq!(view.get_id()?, 42);
assert!(view.has_title()?);
assert_eq!(view.get_title()?.as_str()?, "hello");
```

Explicit scalar fields use `Option<T>` in builders. `Some(default)` differs from
`None`, subject to the chosen layout's existing presence semantics. Reference
fields accept typed offsets produced by the same writer. Using an offset from
another writer, or one retained across `Writer::clear`, returns an error.

* Use `Writer::scalar_array` for numeric/bool arrays; enum arrays use raw `i32`s.
* Use `Writer::offset_array` for strings and non-fixed message offsets.
* Root strings and arrays are supported by `ByteString::read` and `Array::read`.
* Use generated `FooBuilder::write_array` for message arrays. It selects inline
  storage for fixed messages and offset storage otherwise.
* Maps expose entry builders such as `Update_LabelsEntryBuilder`; their
  `write_array` sorts keys and rejects duplicates. Views expose `get_labels`
  for iteration and `find_labels(key)` for binary search.
* Oneof members are optional builder fields. Setting multiple members is an
  error; views expose `which_<oneof_name>()` and individual field getters.
* Generated enums are open `i32` newtypes. Named constants and aliases preserve
  unknown numeric values.
* `bytes` returns `&[u8]`. A protobuf `string` returns `ByteString`, which offers
  `as_bytes()` and checked `as_str()`. Repeated string/bytes views offer `get`
  and `get_str`; arbitrary bytes are never implicitly treated as UTF-8.

Dynamic writes default to sized flat for dense schemas and sparse for schemas
with untyped gaps. `writer.set_dynamic_layout(yaff::Layout::Sparse)` selects sparse
for dense dynamic schemas. Static schemas retain their declared layout.

`view.copy_to(&mut writer)` reserializes known fields without a Protobuf object.
`view.to_builder(&mut writer, depth_limit)` also permits editing before writing.
The default copy nesting limit is 128. Original bytes can be forwarded to
preserve unknown fields; copying through a partial schema drops them, as does
materializing C++ YaFF into a partial Protobuf schema.

## Optional trusted access

For internal workloads with the same valid-buffer contract as C++ YaFF, enable
the runtime feature and explicitly request the generated trusted API:

```toml
yaff = { path = "/path/to/yaff/rust/yaff", features = ["trusted"] }
```

```rust,ignore
// build.rs, in addition to the regular plugin/include configuration:
config.trusted(true);

// The caller must guarantee the complete graph is valid for this schema and
// remains immutable for the borrow's lifetime. A lazy checked read is not a verifier.
let view = unsafe { Update::read_trusted(&bytes) };
let id = view.get_id()?;

// Resolve physical layout once when performing several accesses to one message.
if let Some(flat) = view.as_flat() {
    assert_eq!(flat.get_id()?, id);
} else if let Some(sparse) = view.as_sparse() {
    assert_eq!(sparse.get_id()?, id);
}
```

The `unsafe` entry point is the trust boundary: malformed data, wrong schemas,
out-of-range references or invalid scalar representations can cause undefined
behavior. The ordinary `read` API remains checked. There is no automatic verifier
or safe conversion from a partially accessed checked view to a trusted view.

Trusted views cache the immutable wire header. On 64-bit targets they occupy
16 bytes; checked views occupy 32 bytes. `as_flat` and `as_sparse` are safe layout
specializations of an already valid trusted view, not validators and not buffer
conversions. They return `None` for the other representation and for fixed
schemas. The const generic parameter in `UpdateTrusted<'a, MODE>` selects dispatch
(0 = schema/default, 1 = flat, 2 = sparse); normally let the compiler infer it.

Getter names and `Result` signatures match the checked API. Trusted field access
on a valid buffer is infallible; array `get(index)` still checks the index and
`get_str` still validates UTF-8. Scalar/message array `get_unchecked(index)` is a
separate unsafe operation requiring `index < len()`. Iterators enforce that bound
internally. Nested messages can have a different layout, so specialize each view
independently when useful. All pointer operations are isolated in `trusted.rs`;
there is no runtime assembly or platform-specific pointer tagging.

## Validation and limits

Constructing a view validates its root/header, not the whole object graph.
Each accessor checks the bounds and representation it accesses and returns
`yaff::Result`. Generated array getters validate the complete inline array span
before iteration. Reading one field does not certify unrelated fields or the
business meaning of values. Manual low-level metadata must describe the intended
schema. The writer enforces field IDs up to 8190 and buffers below 2 GiB.
Nonempty arrays of zero-sized fixed messages are rejected, matching C++'s producer.

The current Rust writer does not automatically deduplicate objects or sparse
tables. Its output may be larger than C++ output while remaining readable by C++.
Explicitly reusing a typed offset shares an object. Readers support C++
deduplication, including signed references to shared sparse tables.
Protobuf object adapters, Serde integration and the C++ reflection visitor API
are separate additions; they are not needed for binary interoperability.

## Tests and fuzzing

Runtime checks need only Rust and the checked-in C++ fixtures:

```sh
cd rust
cargo test --offline
cargo fmt --all --check
```

For full cross-language tests, build C++ with GoogleTest available:

```sh
cmake -S . -B build/local -DYAFF_BUILD_TESTS=ON -DCMAKE_BUILD_TYPE=Release
cmake --build build/local --parallel
ctest --test-dir build/local --output-on-failure
python3 tests/interop/check.py --build-dir build/local
cd rust
cargo clippy --offline --workspace --all-targets -- -D warnings
```

`check.py` verifies the golden corpus, both native writers/readers, generated
round trips, old/new schemas, map order, defaults, unknown enums, oneof and slices.
It also runs 20,000 deterministic mutation cases against the safe Rust reader.
Increase `--fuzz-cases` for longer runs. `yaff_interop::fuzz::exercise` is reusable
from external coverage-guided fuzzing harnesses. Corrupt buffers are never fed to
the unchecked C++ reader.

The generated test crate supports `YAFF_CPP_INTEROP` and `YAFF_PROTOC_GEN_RUST`
to locate executables in a different CMake build directory.

For pointer/provenance checks, install nightly with the `miri` and `rust-src`
components, then run from `rust/` (the generated test also needs the C++ generator):

```sh
cargo +nightly miri setup
MIRIFLAGS='-Zmiri-disable-isolation -Zmiri-strict-provenance' \
  cargo +nightly miri test --offline -p yaff --features trusted --test trusted
MIRIFLAGS='-Zmiri-disable-isolation -Zmiri-strict-provenance' \
  cargo +nightly miri test --offline -p yaff-interop trusted_native_roundtrip_and_defaults
```

## Read benchmarks

The [main README](../README.md#rust-read-performance) reports checked, trusted and
layout-specialized trusted performance, including construction and array-index
costs. The recorded [process medians](../benchmarks/rust/reader-aarch64.json) include
source-file hashes so the measurements remain identifiable after rebasing.

Build the C++ fixture producer, both generators, and the benchmark in Release
(with Protobuf and GoogleTest available as described above):

```sh
cmake -S . -B build/local -DYAFF_BUILD_TESTS=ON -DCMAKE_BUILD_TYPE=Release
cmake --build build/local --parallel
cmake --build build/local --target yaff_interop_bench --parallel
export PROTOC="$(cat build/local/tests/interop/protoc-path.txt)"
cargo +1.85.0 build --offline --release --manifest-path rust/Cargo.toml \
  -p yaff-interop --example bench_parity
RUSTUP_TOOLCHAIN=1.85.0 RUSTC=rustc \
  python3 tests/interop/parity.py --build-dir build/local --cpp-build build/local \
    --repetitions 5
```

The runner creates valid C++ fixtures, uses the same bytes in both languages,
randomizes process order, and writes all process medians to
`build/parity-results/results.json`. `bench_parity` measures the actual generated
APIs. Its unchecked-index case uses a length established before timing, matching
the C++ array accessor's contract. These hot-loop measurements are not isolated
load latency, serialization speed or end-to-end application throughput.
