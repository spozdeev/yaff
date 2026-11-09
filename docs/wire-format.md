# YaFF wire format

This describes the existing encodings in YaFF 0.1.0, checked against C++ commit
`67bf5c9f1219`. Schema information is required to read a buffer. There is no
embedded schema, type identifier, total length, checksum or version envelope.
Transport framing supplies the buffer length and root type.

The Rust implementation targets the bytes emitted by little-endian C++ hosts
(including x86-64 and AArch64). C++ currently copies native scalar representations
without endian conversion. Cross-endian C++ interoperability is not guaranteed.

## Common values and references

All records are packed without alignment padding. Booleans occupy one byte
(`0` or `1`); integers/enums use 4 or 8 bytes; enums are open signed `int32` values.
Floats use IEEE 754 binary32/binary64. Protobuf integer encoding variants with
the same signedness and width share the same representation; there are no varints.

A complete buffer starts with a little-endian `uint32` offset from byte zero to
the root object (a message, array or string). A message root may have metadata between this prefix and its header.
The C++ serializer limits offsets to `2^31 - 1`. Field IDs are in `1..=8190`.

An object field stores a `uint32` displacement **from the containing message's
start**, not from the field's address. Zero represents an absent reference in
fixed/flat messages. An absent sparse field has no table entry. Present object
references always point to objects written before their container by the
backwards-growing serializer, i.e. later in the final byte stream.

Strings and bytes share `[uint32 byte_length][bytes][zero byte]`. The trailing
zero is excluded from length; interior zero bytes are allowed. The wire format
does not validate UTF-8. An explicitly present empty string has a nonzero object
reference and a zero-length string record, distinct from absence.

Arrays start with a `uint32` element count. Scalars immediately follow at their
natural wire width. Non-fixed messages and strings use `uint32` offsets relative
to the **start of the array's elements**, after its count. Fixed messages are
embedded consecutively with the fixed schema size as stride. References inside
each embedded fixed message remain relative to that individual message.
An empty repeated field is normally represented by a zero field reference.

Maps are arrays of fixed key/value messages (`key = 1`, `value = 2`), sorted in
ascending key order. String keys compare their bytes lexicographically; numeric
keys use their signed/unsigned value. Maps have unique keys.

## Fixed

There is no header. Fields occupy the schema's cumulative inline offsets, in
field-ID order. Scalar slots have their scalar width; object slots occupy four
bytes. Every slot is allocated, including typed deprecated fields.

A scalar stores `value_bits XOR default_bits`. Missing slots are zero-filled.
Readers XOR again with the schema default. Floating-point XOR is bitwise.
Scalar/string presence is implicit, determined by nonzero encoded bytes;
message presence is determined by a nonzero reference. Fixed schemas cannot
evolve and do not support oneof. Singular fixed children are still referenced;
only fixed array elements are embedded inline.

## Flat

The message begins with a `uint16 typed_limit`, followed by packed field slots.
Scalar encoding and field offsets match fixed, but offsets start after this
two-byte header. Slots beyond the last populated field may be omitted.

For a nonempty message:

```
typed_limit = 0x8000 | ((highest_present_id + 1) << 2) | (sized << 1) | explicit
```

A field is addressable when its ID is smaller than
`(typed_limit & 0x7fff) >> 2`. Otherwise its schema default applies. Empty flat
messages are serialized as `0x8000`; the C++ static default view uses `0x8003`.

Metadata precedes the header. Treat it as a bit stream whose byte zero is the
byte immediately before the header, byte one is two bytes before it, etc.
Bits within each byte run least-significant first. For each field ID in order:

* `explicit` contributes one presence bit;
* `sized` contributes two size bits after the presence bit, if any;
* size codes `0, 1, 2, 3` mean inline widths `0, 1, 4, 8`.

Metadata occupies `ceil(highest_present_id * (explicit + 2*sized) / 8)` bytes.
When explicit is zero, presence comes from nonzero encoded field bytes. The
C++ writer drops the presence mask if every explicitly added scalar differs
from its default. It retains it when a default scalar is explicitly present.
Object references, including empty strings/messages, already encode presence.

Dynamic flat writers include size metadata. A newer reader whose schema has
removed untyped fields adds their recorded sizes to the known cumulative
offset. This is what lets a sparse-capable new schema read old dense buffers.
Statically selected flat layouts need all slot widths preserved in the schema.

## Sparse

A nonempty message starts with `uint16 ((highest_present_id + 1) << 2) | 3`,
followed by only the present fields in ascending field-ID order. Scalars store
their actual bits **without default XOR**. Presence means a nonzero table entry.
Explicit defaults are therefore preserved. Empty sparse messages contain only
`uint16 2`; the C++ static default view uses `uint16 3`.

At `message_start - 4` is an `int32` displacement from `message_start` to the
**end** of the field-offset table. Entries are indexed backwards from that end:

* IDs 1 through 31: one-byte offset at `table_end - id`;
* IDs 32 onward: two-byte offset at `table_end - 2*id + 31`.

Each nonzero entry is an offset from `message_start` to the field's inline slot;
zero means absent. An object slot then contains another offset from the same
message start to its object. A fresh table is before the signed displacement;
tables may be shared, so their displacement can be positive or negative.

## Dynamic dispatch, defaults and evolution

Dynamic is a schema/dispatch choice, not a fourth physical encoding. Bit 15 of
the header distinguishes flat from sparse. It cannot identify a fixed message;
fixed requires the matching schema. Static readers require their own layout.

The current C++ default for a dense dynamic schema is sized flat. Schemas with
untyped gaps use sparse. The caller can explicitly force sparse for a dense
dynamic schema. Data-driven selection is not implemented yet.

Presence follows the layout and schema, including oneof members. There is no
separate union tag: oneof case is determined by member presence. Unknown enum
numbers survive. Unknown fields survive forwarding the original buffer; an
object materialized with a partial schema does not preserve them automatically.

Defaults must agree between implementations. C++ currently represents float
defaults in IR using `std::to_string` and tests implicit floating-point equality
using `abs(a-b) <= min(abs(a),abs(b)) * epsilon`. These existing behaviors affect
the bytes and omission rules; changing them independently in one language would
break compatibility. NaN payloads and negative zero need bit-level tests where
the writer actually retains them.

## Reproducible corpus

`tests/interop/oracle.cpp` writes and checks the checked-in binary corpus using
the original C++ runtime with hand-written schema metadata. Build and run with:

```sh
python3 tests/interop/run.py check
# Regenerate deliberately after reviewing a format change:
python3 tests/interop/run.py write
```

This needs a C++20 compiler and Python, but no Protobuf or GoogleTest. Rust tests
read these files independently. Semantic compatibility is the primary contract:
deduplication and object placement can produce different valid byte sequences.
