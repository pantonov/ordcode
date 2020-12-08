# Ordcode


![Build](https://github.com/pantonov/ordcode/workflows/Build/badge.svg)

 A set of primitives and [Serde](https://serde.rs) serializers for
 fast, prefix-free encoding which preserves lexicographical ordering of values.

 It is intended for encoding keys and values in key-value databases.

## OMG! Yet another serialization format?

 In most existing designs, prefix-free encoding of byte sequences is performed by escaping
 "end-of-sequence" bytes. This takes extra space, and makes it difficult to know sequence length
 without processing the whole input buffer; this also complicates memory allocation for
 deserialized data. Instead, we take advantage of the fact that exact record size is always
 known in key-value databases, so this implementation relies on "two-sided" buffer design:
 sequence lengths are varint-encoded and pushed to the tail end of the buffer, so
 it is possible to get original length of serialized byte sequence(s) by deserializing of
 a few bytes only.
 For serialization, this implementation provides (very fast) calculation of exact size
 of serialized data length prior to serialization. These features
 enable effective and predictable buffer management for repetitive scans and no-heap
 (`#[no-std]`) targets.

## Features

 * encodings in both ascending and descending lexicographical orderings are supported
 * encoding puts lengths of variable-size sequences to the end of serialized data,
   so resulting encoding is prefix-free and friendly to lexicographical ordering
 * zero allocations, supports `#[no_std]` environments
 * method to cheaply get exact size of serialized data without doing actual serialization,
   for effective buffer management
 * space-efficient varint encoding for sequence lengths and discriminants
 * easily customizable (endianness, encoding of primitive types etc.), with useful pre-sets
 * reader/writer traits for double-ended buffers, so you can implement your own or use
   implementations provided by the crate
 * no unsafe code

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE.txt](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT.txt](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.