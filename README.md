# Ordcode

[![Build Status](https://travis-ci.org/pantonov/ordcode.svg?branch=master)](https://travis-ci.org
/pantonov/ordcode)
[![Crates.io](https://img.shields.io/crates/v/ordcode.svg)](https://crates.io/crates/ordcode)
[![Documentation](https://docs.rs/ordcode/badge.svg)](https://docs.rs/ordcode)

This Rust crate implements a set of primitives and [Serde](https://serde.rs) data format for
fast, prefix-free encoding which preserves lexicographical ordering of values.

It is intended for encoding keys and values in key-value databases.

Serialized data format has the following properties:
* encodings in both ascending and descending lexicographical orders are supported
* concatenation of encoded values preserves ordering. Therefore, serializing `struct` yields
  composite key.
* encoded data format is NOT self-descriptive and relies on correct sequence
* encoding of the primitive types (ints, floats) has the same size as original type
* byte arrays and strings encoded with prefix-free escaping, strings use UTF-8
* non-byte variable-length sequences use double encoding: first, they are encoded into the
 temporary byte buffer, then this buffer is encoded again with prefix-free encoding
* encoding is cross-platform, does not depend on endianness.

This crate also provides `bin` module, which contains fast serializer and deserializer similar
 to [bincode](https://github.com/servo/bincode), but binary portable between platforms with
 different endianness. It also provides more compact encoding (indexes and lengths are
 encoded as varints).

## Usage
Add this to your `Cargo.toml`:
```toml
[dependencies]
ordcode = "0.1.*"
```

### Cargo.toml features
Feature `serde` is on by default. If you need only primitives, and do not want `serde`
 dependency, you can opt out.

### Stability guarantees
The underlying encoding format is simple and unlikely to change. However, this crate
 provides versioned re-exports to guarantee data format compatibility, such as `ordcode::asc_v1`.

### Example
For more examples, see documentation of methods in `asc`, `desc` and `bin` modules.
```rust
use ordcode::asc;

let buf = asc::to_bytes(&258u16).unwrap();
assert!(buf[0] == 1 && buf[1] == 2);   
let v: u16 = asc::from_bytes(&buf).unwrap();
assert_eq!(v, 258);
```

### Other
Encoding and decoding speed is supposed to be of the same order as
 [bincode](https://github.com/servo/bincode), but a bit slower because of varints and
 prefix-free encoding for sequences.


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