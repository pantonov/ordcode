//! A set of primitives and [Serde](https://serde.rs) serializers for
//! fast, prefix-free encoding which preserves lexicographical ordering of values.
//!
//! It is intended for encoding keys and values in key-value databases.
//!
//! Serialized data format has the following properties:
//! * encodings in both ascending and descending lexicographical orders are supported
//! * concatenation of encoded values preserves ordering. Therefore, serializing `struct` yields
//!   composite key
//! * encoded data format is NOT self-descriptive and relies on correct sequence
//! * encoding of the primitive types (ints, floats) has the same size as original type
//! * byte arrays and strings encoded with prefix-free escaping, strings use UTF-8
//! * non-byte variable-length sequences use double encoding: first, they are encoded into the
//!   temporary byte buffer, then this buffer is encoded again with prefix-free encoding
//! * encoding is always big-endian, serialized data is safe to move between platforms with different
//!   endianness.
//!
//! This crate also provides `bin` module, which contains fast serializer and deserializer similar
//! to [bincode](https://github.com/servo/bincode), but portable between platforms with
//! different endianness. It also uses a more compact encoding: indexes and lengths are
//! encoded as varints instead of u32/u64.
//!
//! ### Cargo.toml features
//! Feature `serde` is on by default. If you need only primitives, and do not want `serde`
//! dependency, you can opt out.
//!
//! Optional feature `smallvec` replaces `Vec<u8>` with `SmallVec<[u8;36]>` as default byte buffer.
//!
//! ### Stability guarantees
//! The underlying encoding format is simple and unlikely to change. As a safeguard, modules
//! `primitives`, `ord` and `bin` provide `VERSION` constant.
//!
//! ### Other
//! Encoding and decoding speed is supposed to be in the same league as
//! [bincode](https://github.com/servo/bincode), but a bit slower because of fixed endianness,
//! varints and prefix-free encoding for sequences.
//!
//! Serializing with descending lexicographical order is particularly useful for key-value storages like
//! _rocksdb_, where iteration in reverse key order is expensive.

#![doc(html_root_url = "https://docs.rs/ordcode")]
#![crate_name = "ordcode"]

#![deny(clippy::all, clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]

#[cfg(feature="serde")] #[macro_use] extern crate serde;
#[macro_use] extern crate error_chain;
#[cfg(feature="smallvec")] extern crate smallvec;

#[macro_use] mod errors;
pub use errors::{ Error, ErrorKind, ResultExt };

/// A convenient Result type
pub type Result<T = (), E = errors::Error> = core::result::Result<T, E>;

pub mod primitives;

#[cfg(feature="serde")] mod ord_ser;
#[cfg(feature="serde")] mod ord_de;
#[cfg(feature="serde")] mod bin_ser;
#[cfg(feature="serde")] mod bin_de;

/// Specifies lexicographical ordering for serialization. There are no ordering marks in the
/// serialized data; specification of different ordering for serialization and deserialization
/// of the same data is UB.
#[derive(Copy, Clone)]
pub enum Order {
    Ascending,
    Descending,
    /// For use by other crates. For the purposes of `ordcode`, same as `Ascending`.
    Unspecified
}

#[cfg(not(any(feature="smallvec", feature="sled")))]
/// Byte buffer type. May be `Vec<u8>` or `smallvec::SmallVec<[u8;36]>` depending
/// on `smallvec` feature setting.
pub type BytesBuf = Vec<u8>;

#[cfg(feature="smallvec")]
/// Byte buffer type. May be `Vec<u8>` or `smallvec::SmallVec<[u8;36]>` depending
/// on `smallvec` feature setting.
pub type BytesBuf = smallvec::SmallVec<[u8; 36]>;

// experimental, do not use yet
#[cfg(feature="sled")]
pub type BytesBuf = sled::IVec;

/// Helper trait for internally used methods of `BytesBuf`, which may be implemented differently for
/// externally provided `BytesBuf` types
pub trait BytesBufExt {
    /// Reserve `BytesBuf` with capacity
    fn with_reserve(n: usize) -> BytesBuf;

    /// Convert `BytesBuf` into `Vec<u8>`
    fn into_vec8(self) -> Vec<u8>;
}

#[cfg(not(any(feature="smallvec", feature="sled")))]
impl BytesBufExt for Vec<u8> {
    fn with_reserve(n: usize) -> BytesBuf { Self::with_capacity(n) }
    fn into_vec8(self) -> Vec<u8> { self }
}

/// Trait for low-level reader, optimized for byte buffers.
///
/// If you need to read from `&[u8]`, you may use `BytesReader` provided by this crate.
pub trait ReadBytes {
    /// Call a closure with to slice of the buffer containing exactly `n` bytes.
    /// If `advance` is true, advance buffer by `n` bytes after calling closure.
    fn apply_bytes<R, F>(&mut self, n: usize, advance: bool, f: F) -> Result<R>
        where F: FnOnce(&[u8]) -> Result<R>;

    /// Iterate over buffer, splitting on escape byte `esc`, calling closure with
    /// reference to slice which includes `esc` byte in last position, and
    /// with value of next byte after `esc`. If `advance` is true, advance buffer
    /// by slice.len()+1. Iteration continues while closure returns `Ok(true)`.
    fn apply_over_esc<F>(&mut self, esc: u8, advance: bool, f: &mut F) -> Result
        where F: FnMut(&[u8], u8) -> Result<bool>;

    /// Read all data until the end of input and apply closure. Implementation of this method
    /// is not required, as it is used only by primitive `deserialize_bytes_noesc`,
    /// which is not used in Serde serializers provided by this crate.
    fn apply_all<R, F>(&mut self, f: F) -> Result<R>
        where F: FnOnce(&[u8]) -> Result<R> { let _ = f; err!(BytesReadAllNotImplemented) }

    /// Returns `true` if at the end of input
    fn at_end(&mut self) -> bool;
}

// forwarding for being able to use `&mut ReadBytes` in place of `ReadBytes`
impl<'a, T> ReadBytes for &'a mut T where T: ReadBytes  {
    #[inline]
    fn apply_bytes<R, F>(&mut self, n: usize, advance: bool, f: F) -> Result<R>
        where F: FnOnce(&[u8]) -> Result<R> {
        (*self).apply_bytes(n, advance, f)
    }
    #[inline]
    fn apply_over_esc<F>(&mut self, esc: u8, advance: bool, f: &mut F) -> Result
        where F: FnMut(&[u8], u8) -> Result<bool> { (*self).apply_over_esc(esc, advance, f) }
    #[inline]
    fn apply_all<R, F>(&mut self, f: F) -> Result<R>
        where F: FnOnce(&[u8]) -> Result<R> { (*self).apply_all( f) }
    #[inline]
    fn at_end(&mut self) -> bool { (*self).at_end() }
}

/// Implementation of `ReadBytes` from byte slice
pub struct BytesReader<'a> {
    buf: &'a [u8],
}

impl<'a> BytesReader<'a> {
    /// Constructs reader from provided byte slice
    #[must_use] pub fn new(buf: &'a [u8]) -> Self { Self { buf } }
}

impl <'a> ReadBytes for BytesReader<'a> {
    #[inline]
    fn apply_bytes<R, F>(&mut self, n: usize, advance: bool, f: F) -> Result<R>
        where F: FnOnce(&[u8]) -> Result<R>
    {
        if self.buf.len() >= n {
            let r = f(&self.buf[..n]);
            if advance {
                self.buf = &self.buf[n..];
            }
            r
        } else {
            err!(PrematureEndOfInput)
        }
    }
    #[inline]
    fn apply_over_esc<F>(&mut self, esc: u8, advance: bool, f: &mut F) -> Result
        where F: FnMut(&[u8], u8) -> Result<bool>
    {
        let mut b = &self.buf[..];
        let r = loop {
            if let Some(pos) = b.iter().position(|v| *v == esc) {
                if pos + 1 >= b.len() {
                    break err!(PrematureEndOfInput)
                }
                if !f(&b[..=pos], b[pos+1])? {
                    b = &b[pos+2..];
                    break Ok(())
                }
                b = &b[pos+2..];
            } else {
                break err!(PrematureEndOfInput)
            }
        };
        if advance {
            self.buf = b
        }
        r
    }
    #[inline]
    fn apply_all<R, F>(&mut self, f: F) -> Result<R>
        where F: FnOnce(&[u8]) -> Result<R>
    {
        f(self.buf)
    }
    #[inline]
    fn at_end(&mut self) -> bool { self.buf.is_empty() }
}

impl std::io::Read for BytesReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.buf.read(buf)
    }
}

/// Trait for writer to the byte buffer
///
/// This crate provides implementation of `WriteBytes` for `BytesBuf`, which can be `Vec<u8>` or
/// `smallvec::SmallVec<[u8;36]>` depending on `smallvec` feature setting.
pub trait WriteBytes {
    /// Write to the byte buffer
    fn write(&mut self, value: &[u8]) -> Result;
    /// Write single byte to the byte buffer
    fn write_byte(&mut self, value: u8) -> Result {
        self.write(&[value])
    }
}

impl WriteBytes for Vec<u8> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> Result {
        self.extend_from_slice(buf);
        Ok(())
    }
    #[inline]
    fn write_byte(&mut self, b: u8) -> Result {
        self.push(b);
        Ok(())
    }
}

// forwarding for being able to use `&mut WriteBytes` in place of `WriteBytes`
impl<T> WriteBytes for &mut T where T: WriteBytes {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> Result { (*self).write(buf) }
    #[inline]
    fn write_byte(&mut self, b: u8) -> Result { (*self).write_byte(b) }
}

#[cfg(feature="smallvec")]
impl BytesBufExt for BytesBuf {
    fn with_reserve(n: usize) -> BytesBuf { Self::with_capacity(n) }
    fn into_vec8(self) -> Vec<u8> { self.into_vec() }
}

#[cfg(feature="sled")]
impl BytesBufExt for BytesBuf {
    fn with_reserve(n: usize) -> BytesBuf { Self::with_capacity(n) }
    fn into_vec8(self) -> Vec<u8> { self.into() }
}


#[cfg(any(feature="smallvec", feature="sled"))]
impl WriteBytes for BytesBuf {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> Result {
        self.extend_from_slice(buf);
        Ok(())
    }
    #[inline]
    fn write_byte(&mut self, b: u8) -> Result {
        self.push(b);
        Ok(())
    }
}

#[cfg(feature="serde")]
pub mod ord {
    //! Methods for lexicographically ordered serialization and deserialization.

    // workaround for absence (yet) of const generics
    #[doc(hidden)]
    #[allow(clippy::module_name_repetitions)]
    pub trait OrdTrait {
        const ORDER: Order;
    }
    #[doc(hidden)]
    pub struct AscendingOrder;
    impl OrdTrait for AscendingOrder {
        const ORDER: Order = Order::Ascending;
    }
    #[doc(hidden)]
    pub struct DescendingOrder;
    impl OrdTrait for DescendingOrder {
        const ORDER: Order = Order::Descending;
    }

    use crate::{Result, Order, ReadBytes, WriteBytes, BytesBuf, BytesReader };

    /// Serialize `value` into `writer`
    ///
    /// *Example*
    /// ```
    /// # use ordcode::{ ord, Order };
    /// let mut buf = Vec::<u8>::new();
    /// ord::to_bytes_writer(&mut buf, &258u16, Order::Ascending).unwrap();
    /// assert!(buf[0] == 1 && buf[1] == 2); // 258 serialized as big endian
    /// ```
    pub fn to_bytes_writer<T>(writer: &mut impl WriteBytes, value: &T, order: Order) -> Result
        where T: ?Sized + serde::ser::Serialize,
    {
        match order {
            Order::Descending =>
                value.serialize(&mut new_serializer_descending(writer)),
            Order::Ascending|Order::Unspecified =>
                value.serialize(&mut new_serializer_ascending(writer)),
        }
    }
    /// Serialize `value` into byte vector
    ///
    /// *Example*
    /// ```
    /// # use ordcode::{ ord, Order };
    /// let buf = ord::to_bytes(&258u16, Order::Ascending).unwrap();
    /// assert!(buf[0] == 1 && buf[1] == 2); // 258 serialized as big endian
    /// ```
    pub fn to_bytes<T>(value: &T, order: crate::Order) -> Result<BytesBuf>
        where T: ?Sized + serde::ser::Serialize
    {
        let mut out = BytesBuf::new();
        to_bytes_writer(&mut out, value, order)?;
        Ok(out)
    }
    /// Deserialize value from `reader`
    ///
    /// *Example*
    /// ```
    /// # use ordcode::{ BytesReader, ord, Order };
    /// let reader = BytesReader::new(&[1u8, 2]); // 258, big endian
    /// let v: u16 = ord::from_bytes_reader(reader, Order::Ascending).unwrap();
    /// assert_eq!(v, 258);
    /// ```
    pub fn from_bytes_reader<T>(reader: impl ReadBytes, order: Order) -> Result<T>
        where T: serde::de::DeserializeOwned,
    {
        match order {
            Order::Descending =>
                T::deserialize(&mut new_deserializer_descending(reader)),
            Order::Ascending|Order::Unspecified =>
                T::deserialize(&mut new_deserializer_ascending(reader)),
        }
    }
    /// Deserialize value from byte slice
    ///
    /// *Example*
    /// ```
    /// # use ordcode::{ ord, Order };
    /// let v: u16 = ord::from_bytes(&[1u8, 2], Order::Ascending).unwrap();
    /// assert_eq!(v, 258);
    /// ```
    pub fn from_bytes<T>(input: &[u8], order: Order) -> Result<T>
        where T: serde::de::DeserializeOwned,
    {
        from_bytes_reader(&mut BytesReader::new(input), order)
    }
    pub use crate::ord_ser::VERSION;

    /// Create new deserializer instance, with `Ascending` ordering.
    /// Mutable reference to returned value implements `serde::Deserializer`.
    ///
    /// *Example*
    /// ```
    /// # use { ordcode::{ BytesReader, ord }, serde::Deserialize };
    /// let reader = BytesReader::new(&[1u8, 2]); // 258, big endian
    /// let mut deser = ord::new_deserializer_ascending(reader);
    /// let foo = u16::deserialize(&mut deser).unwrap();
    /// let mut _reader = deser.into_reader();             // can get reader back, if needed
    ///
    /// assert_eq!(foo, 258);
    /// ```
    pub fn new_deserializer_ascending<R>(reader: R) -> crate::ord_de::Deserializer<R, AscendingOrder>
        where R: ReadBytes,
    {
        crate::ord_de::Deserializer::new(reader)
    }

    /// Create new deserializer instance, with `Descending` ordering.
    /// Mutable reference to returned value implements `serde::Deserializer`.
    ///
    /// *Example*
    /// ```
    /// # use { ordcode::{ BytesReader, ord }, serde::Deserialize };
    /// let reader = BytesReader::new(&[254u8, 253]); // 258, big endian, descending
    /// let mut deser = ord::new_deserializer_descending(reader);
    /// let foo = u16::deserialize(&mut deser).unwrap();
    /// let mut _reader = deser.into_reader();             // can get reader back, if needed
    ///
    /// assert_eq!(foo, 258);
    /// ```
    pub fn new_deserializer_descending<R>(reader: R) -> crate::ord_de::Deserializer<R, DescendingOrder>
        where R: ReadBytes,
    {
        crate::ord_de::Deserializer::new(reader)
    }

    /// Create new serializer instance with `Ascending` ordering. Mutable reference to returned value
    /// implements `serde::Serializer`.
    ///
    /// *Example*
    /// ```
    /// # use { ordcode::{ ord, BytesBuf }, serde::Serialize };
    /// let mut buf: BytesBuf = BytesBuf::new();
    /// let mut ser = ord::new_serializer_ascending(&mut buf);
    /// 258u16.serialize(&mut ser).unwrap();
    ///
    /// assert!(buf[0] == 1 && buf[1] == 2); // 258 serialized as big endian
    /// ```
    pub fn new_serializer_ascending<W>(writer: W) -> crate::ord_ser::Serializer<W, AscendingOrder>
        where W: WriteBytes,
    {
        crate::ord_ser::Serializer::new(writer)
    }

    /// Create new serializer instance with `Descending` ordering. Mutable reference to returned value
    /// implements `serde::Serializer`.
    ///
    /// *Example*
    /// ```
    /// # use { ordcode::{ ord, BytesBuf }, serde::Serialize };
    /// let mut buf: BytesBuf = BytesBuf::new();
    /// let mut ser = ord::new_serializer_descending(&mut buf);
    /// 258u16.serialize(&mut ser).unwrap();
    ///
    /// assert!(buf[0] == 254 && buf[1] == 253); // 258 serialized as descendng order, big endian
    /// ```
    pub fn new_serializer_descending<W>(writer: W) -> crate::ord_ser::Serializer<W, DescendingOrder>
        where W: WriteBytes,
    {
        crate::ord_ser::Serializer::new(writer)
    }
}

#[cfg(feature="serde")]
pub mod bin {
    //! Methods for fast binary serialization and deserialization. Ordering parameter is ignored.

    use crate::{Result, WriteBytes, BytesBuf, BytesReader, ReadBytes};

    /// Serialize `value` into `writer`
    ///
    /// *Example*
    /// ```
    /// # use ordcode::{ bin };
    /// let mut buf = Vec::<u8>::new();
    /// bin::to_bytes_writer(&mut buf, &258u16).unwrap();
    /// assert!(buf[0] == 1 && buf[1] == 2); // 258 serialized as big endian
    /// ```
    pub fn to_bytes_writer<T>(writer: &mut impl WriteBytes, value: &T) -> Result
        where T: ?Sized + serde::ser::Serialize,
    {
        value.serialize(&mut new_serializer(writer))
    }

    /// Serialize `value` into byte vector
    ///
    /// *Example*
    /// ```
    /// # use ordcode::{ bin  };
    /// let buf = bin::to_bytes(&258u16).unwrap();
    /// assert!(buf[0] == 1 && buf[1] == 2); // 258 serialized as big endian
    /// ```
    pub fn to_bytes<T>(value: &T) -> Result<BytesBuf>
        where T: ?Sized + serde::ser::Serialize
    {
        let mut out = crate::BytesBuf::new();
        to_bytes_writer(&mut out, value)?;
        Ok(out)
    }

    /// Deserialize value from `reader`
    ///
    /// *Example*
    /// ```
    /// # use ordcode::{ bin, BytesReader };
    /// let reader = BytesReader::new(&[1u8, 2]); // 258, big endian
    /// let v: u16 = bin::from_bytes_reader(reader).unwrap();
    /// assert_eq!(v, 258);
    /// ```
    pub fn from_bytes_reader<T>(reader: impl ReadBytes) -> Result<T>
        where T: serde::de::DeserializeOwned,
    {
        T::deserialize(&mut new_deserializer(reader))
    }

    /// Deserialize value from byte slice
    ///
    /// *Example*
    /// ```
    /// # use ordcode::{ ord, Order };
    /// let v: u16 = ord::from_bytes(&[1u8, 2], Order::Ascending).unwrap();
    /// assert_eq!(v, 258);
    /// ```
    pub fn from_bytes<T>(input: &[u8]) -> Result<T>
        where T: serde::de::DeserializeOwned,
    {
        from_bytes_reader(&mut BytesReader::new(input))
    }

    /// Create new deserializer instance. Mutable reference to returned value
    /// implements `serde::Deserializer`.
    ///
    /// *Example*
    /// ```
    /// # use { ordcode::{ BytesReader, bin }, serde::Deserialize };
    /// let reader = BytesReader::new(&[1u8, 2]); // 258, big endian
    /// let mut deser = bin::new_deserializer(reader);
    /// let foo = u16::deserialize(&mut deser).unwrap();
    /// let mut _reader = deser.into_reader();             // can get reader back, if needed
    ///
    /// assert_eq!(foo, 258);
    /// ```
    pub fn new_deserializer<R>(reader: R) -> crate::bin_de::Deserializer<R>
        where R: crate::ReadBytes,
    {
        crate::bin_de::Deserializer::new(reader)
    }

    /// Create new serializer instance. Mutable reference to returned value
    /// implements `serde::Serializer`.
    ///
    /// *Example*
    /// ```
    /// # use { ordcode::{ bin, BytesBuf }, serde::Serialize };
    /// let mut buf: BytesBuf = BytesBuf::new();
    /// let mut ser = bin::new_serializer(&mut buf);
    /// 258u16.serialize(&mut ser).unwrap();
    ///
    /// assert!(buf[0] == 1 && buf[1] == 2); // 258 serialized as big endian
    /// ```
    pub fn new_serializer<W>(writer: W) -> crate::bin_ser::Serializer<W>
        where W: WriteBytes,
    {
        crate::bin_ser::Serializer::new(writer)
    }
    pub use crate::bin_ser::VERSION;

}
