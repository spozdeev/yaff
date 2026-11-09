<img width="64" src="docs/_assets/icon.svg"/><br/>

[![CI](https://github.com/yandex/yaff/actions/workflows/ci.yaml/badge.svg)](https://github.com/yandex/yaff/actions/workflows/ci.yaml)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://github.com/yandex/yaff/blob/main/LICENSE)
[![Telegram](https://img.shields.io/badge/Telegram-en-2ba2d9.svg)](https://t.me/yaff_en)
[![Telegram](https://img.shields.io/badge/Telegram-ru-2ba2d9.svg)](https://t.me/yaff_ru)

# YaFF

[Documentation](https://yaff.tech/docs/)

YaFF (Yet another Flat Format) is a high-performance serialization library with C++ and Rust runtimes that provides a zero-copy wire format for the [Protobuf](https://protobuf.dev/) ecosystem. While the `.proto` schema remains your single source of truth, YaFF changes the physical representation of data to eliminate runtime overhead. This provides performance-critical applications with direct, zero-parsing access through a proto-like interface, while allowing less performance-sensitive components to parse the wire format back into Protobuf messages.

YaFF shares the zero-copy foundations of [FlatBuffers](https://flatbuffers.dev/), but concentrates on server-side runtimes and adaptive layout strategies that bring read performance close to native C++ structs, preserving interoperability with Protobuf.

## Key Features

* **Zero-Copy Speed** — Eliminates parsing entirely with mmap-compatible layouts.
* **Adaptive Layouts** — Allows explicit configuration or runtime adjustment to manage tradeoffs between speed, memory overhead, and schema flexibility.
* **Protobuf Native** — Uses existing `.proto` files and respects Protobuf semantics.
* **Incremental Adoption** — Emulates standard proto interfaces and provides two-way message conversion for easy module-by-module integration.

## Quick Start

Working with YaFF feels close to Protobuf: you serialize a message and read fields back through a familiar interface, with no parsing step in between.

```cpp
// Turn a Protobuf message into a zero-copy YaFF representation...
const auto buffer = yaff::Serialize<protoyaff::feed::FeedResponse>(proto);

// ...then access fields instantly with zero parsing overhead
const auto& response = yaff::ReadMessage<protoyaff::feed::FeedResponse>(buffer.Data());
std::string_view authorName = response.items(0).author().name();
```

See the full [Quick Start](docs/quick_start.md) guide for the complete walkthrough.

## Roadmap

YaFF’s core design decouples schema definitions from the underlying physical memory layout. This separation allows the exact same data structures to support multiple runtime representations, enabling optimization for specific access patterns without changing the source schema.

YaFF is under active development. Planned and in-progress work includes:

* **Columnar Layout** — Compact representation for large repeated fields, optimized for analytics and ML pipelines.
* **Automated Adaptivity** — Layout selection driven automatically by data analysis or via profile-guided modes.
* **Multi-Language Bindings** — Rust readers, writers and code generation are available; further language support is planned.

## Rust

The [Rust workspace](rust/README.md) provides checked, borrowed message views and
native serialization compatible with C++ YaFF. `protoc-gen-yaff-rust` generates
typed views and builders from the same `.proto` schemas using the shared compiler
frontend. The Rust runtime has no third-party dependencies and does not link C++.
See the [wire format specification](docs/wire-format.md) and the bidirectional
checks under `tests/interop` for the compatibility contract.

### Rust read performance

The same C++-generated buffers were measured on AArch64 macOS with Apple Clang
21 (`-O3 -DNDEBUG`) and Rust 1.85.0 (Cargo Release). Values are nanoseconds per
operation; lower is better. Each process uses one million warmup operations and
seven samples of five million operations. The table shows the median of five
process medians, with process order randomized.

| Layout | Access | C++ unchecked | Rust checked | Rust trusted | Rust trusted, specialized |
| --- | --- | ---: | ---: | ---: | ---: |
| Flat | Scalar | 0.268 | 0.796 | 0.498 | 0.249 |
| Flat | Nested field | 0.442 | 2.593 | 0.796 | 0.533 |
| Flat | Array element¹ | 0.406 | 1.469 | 0.531 | 0.443 |
| Sparse | Scalar | 0.792 | 1.736 | 0.534 | 0.400 |
| Sparse | Nested field | 1.323 | 4.013 | 0.932 | 0.932 |
| Sparse | Array element¹ | 0.762 | 2.548 | 0.608 | 0.531 |

**The columns have different safety contracts.** Checked Rust validates the
accessed data. C++ and Rust `trusted` require a valid, immutable buffer matching
the schema. Rust trusted access is opt-in and starts at an `unsafe` entry point;
it does not validate the complete graph. Specialized views select the root's
physical layout once with `as_flat()` or `as_sparse()` before timing. Nested
messages can have a different layout and are selected during the nested access.
See [trusted API usage](rust/README.md#optional-trusted-access).

¹ C++ and the trusted columns use an unchecked array index after checking the
fixture length before timing; checked Rust uses `get(2)`. The bounds-checked
trusted `get(2)` costs **0.768 / 0.533 ns** for flat dynamic/specialized views and
**0.801 / 0.664 ns** for sparse dynamic/specialized views. UTF-8 conversion remains
checked in the trusted API.

The table above uses already-open views. Constructing a root view and reading one
scalar on every iteration, including Rust layout specialization where selected:

| Layout | C++ open + read | Rust checked | Rust trusted | Rust trusted, specialized |
| --- | ---: | ---: | ---: | ---: |
| Flat | 0.461 | 0.918 | 0.516 | 0.532 |
| Sparse | 0.799 | 1.874 | 0.532 | 0.533 |

On this workload, specialized trusted Rust matches flat scalar performance and
is about **1.4–2x faster** on sparse reads. Flat nested access remains about **20%
slower**, and the unchecked-index array path about **9% slower**. These are
hot-buffer throughput measurements, not isolated-load latency or a guarantee
about cold data, serialization or application-level performance.

[Recorded process medians and source hashes](benchmarks/rust/reader-aarch64.json)
are included. See [build and reproduction commands](rust/README.md#read-benchmarks)
to run the generated APIs on your hardware; the default API remains checked.

## Integration Guide

YaFF can be added to a project in two ways:

- **CMake** — build and install YaFF, then use `find_package`
- **Conan** — build YaFF into the local Conan cache, then declare it as a dependency

Both methods expose the same set of CMake targets and the `yaff_generate()` helper function.

### Available CMake Targets

| Target | Type | When to use |
|---|---|---|
| `yaff::core` | `INTERFACE` library | Always — provides all runtime headers (`include/yaff/*.h`) |
| `yaff::proto` | `STATIC` library | When reading/writing Protobuf-backed YaFF messages |
| `yaff::protoc_plugin` | Executable | Used internally by `yaff_generate()`; not linked by consumers |
| `yaff::rust_protoc_plugin` | Executable | Generates Rust views and builders; not linked by Rust consumers |

Most applications only need `yaff::core` and `yaff::proto`.

For step-by-step integration instructions see [docs/integration.md](docs/integration.md).

## Contributing

Contributions are welcome! See the [contributor's guide](CONTRIBUTING.md) for how to report issues and submit pull requests.

## License

This project is licensed under the Apache 2.0 License (Apache-2.0). [Apache License 2.0](LICENSE).
