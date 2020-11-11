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

//#![doc(html_root_url = "https://docs.rs/ordcode")]
#![crate_name = "biord"]

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
pub mod varint;
pub mod bytes_esc;
pub mod hint_ser;
pub mod params;

pub use hint_ser::SizeCalc;
pub use params::{ EncodingParams, Endianness, AscendingOrder, DescendingOrder,
                  LenEncoder, VarIntLenEncoder, Order };

//#[cfg(feature="serde")] mod ord_ser;
//#[cfg(feature="serde")] mod ord_de;
//#[cfg(feature="serde")] mod bin_ser;
//#[cfg(feature="serde")] mod bin_de;

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

/// Simple byte reader from buffer
///
/// If you need to read from `&[u8]`, you may use `BytesReader` provided by this crate.
pub trait ReadBytes {
    /// Peek `n` bytes from head
    fn peek(&mut self, n: usize) -> Result<&'_[u8]>;

    /// Advance buffer head by `n` bytes. `n` should be smaller than remaining buffer size.
    fn advance(&mut self, n: usize);

    /// Get `n` bytes from the beginning of buffer, advance by `n` bytes
    fn read<F, R>(&mut self, n: usize, f: F) -> Result<R> where F: Fn(&[u8]) -> Result<R> {
        let r = f(self.peek(n)?)?;
        self.advance(n);
        Ok(r)
    }
    /// Returns view into remaining buffer
    fn remaining_buffer(&mut self) -> &'_[u8];

    /// Check if buffer is fully consumed (empty)
    fn is_empty(&mut self) -> bool { self.remaining_buffer().is_empty() }
}

// forwarding for being able to use `&mut ReadBytes` in place of `ReadBytes`
impl<'a, T> ReadBytes for &'a mut T where T: ReadBytes  {
    fn peek(&mut self, n: usize) -> Result<&'_[u8]> {
        (*self).peek(n)
    }
    fn advance(&mut self, n: usize) {
        (*self).advance(n)
    }
    fn remaining_buffer(&mut self) -> &'_[u8] { (*self).remaining_buffer() }
}

/// Implementation of `BiReadBytes` from byte slice
pub struct BytesReader<'a> {
    buf: &'a [u8],
}

impl<'a> BytesReader<'a> {
    /// Constructs reader from provided byte slice
    #[must_use] pub fn new(buf: &'a [u8]) -> Self { Self { buf } }

    fn peek_head(&mut self, n: usize) -> Result<&'_[u8]> {
        if n <= self.buf.len() {
            Ok(&self.buf[..n])
        } else {
            err!(PrematureEndOfInput)
        }
    }
    fn advance_head(&mut self, n: usize) {
        self.buf = &self.buf[n..];
    }
    fn peek_tail(&mut self, n: usize) -> Result<&'_[u8]> {
        if n <= self.buf.len() {
            Ok(&self.buf[(self.buf.len() - n)..])
        } else {
            err!(PrematureEndOfInput)
        }
    }
    fn advance_tail(&mut self, n: usize) {
        self.buf = &self.buf[..self.buf.len() - n];
    }
}

impl <'a> ReadBytes for BytesReader<'a> {
    fn peek(&mut self, n: usize) -> Result<&'_[u8]> {
        self.peek_head(n)
    }
    fn advance(&mut self, n: usize) {
        self.advance_head(n)
    }
    fn remaining_buffer(&mut self) -> &'_[u8] { self.buf }
}

/// Adapter for `BytesReader` for reading from tail if the buffer
pub struct ReadFromTail<'a, 'b>(pub &'a mut BytesReader<'b>);

impl <'a, 'b> ReadBytes for ReadFromTail<'a, 'b> {
    fn peek(&mut self, n: usize) -> Result<&'_[u8]> {
        self.0.peek_tail(n)
    }
    fn advance(&mut self, n: usize) {
        self.0.advance_tail(n)
    }
    fn remaining_buffer(&mut self) -> &'_[u8] { self.0.remaining_buffer() }
}

impl std::io::Read for BytesReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.buf.read(buf)
    }
}

/// Trait for writer to the byte buffer
pub trait WriteBytes {
    /// Write to the byte buffer
    fn write(&mut self, value: &[u8]) -> Result;
}

/// Bipartite byte buffer
pub struct BiBuffer<'a> {
    buf: &'a mut [u8],
    head: usize,
    tail: usize,
}

impl<'a> BiBuffer<'a> {
    /// Use provided byte slice as buffer
    pub fn new(buf: &'a mut [u8]) -> Self {
        let tail = buf.len();
        Self { buf, head: 0, tail }
    }
    /// Finalize by collapsing extra space in internal buffer, returns data length
    pub fn finalize(&mut self) -> Result<usize> {
        if self.head == self.tail {
            Ok(self.buf.len())
        } else {
            self.buf.copy_within(self.tail.., self.head);
            let len = self.buf.len() - (self.tail - self.head);
            self.head = self.tail;
            Ok(len)
        }
    }
    /// Checks if buffer completely filled (collapsed)
    #[must_use]
    pub fn is_complete(&self) -> bool { self.head == self.tail }

    fn write_head(&mut self, value: &[u8]) -> Result {
        if (self.tail - self.head) < value.len() {
            err!(BufferOverflow)
        } else {
            self.buf[self.head..(self.head + value.len())].copy_from_slice(value);
            self.head += value.len();
            Ok(())
        }
    }
    fn write_tail(&mut self, value: &[u8]) -> Result {
        if (self.tail - self.head) < value.len() {
            err!(BufferOverflow)
        } else {
            let end_offs = self.tail - value.len();
            self.buf[end_offs..].copy_from_slice(value);
            self.tail -= value.len();
            Ok(())
        }
    }
}

impl<'a> WriteBytes for BiBuffer<'a> {
    fn write(&mut self, value: &[u8]) -> Result {
        self.write_head(value)
    }
}

/// Adapter for writing to the tail of the buffer
pub struct WriteToTail<'a, 'b>(pub &'a mut BiBuffer<'b>);

impl<'a, 'b> WriteBytes for WriteToTail<'a, 'b> {
    fn write(&mut self, value: &[u8]) -> Result {
        self.0.write_tail(value)
    }
}

// forwarding for being able to use `&mut WriteBytes` in place of `WriteBytes`
impl<T> WriteBytes for &mut T where T: WriteBytes {
    fn write(&mut self, buf: &[u8]) -> Result { (*self).write(buf) }
}

impl WriteBytes for Vec<u8> {
    fn write(&mut self, buf: &[u8]) -> Result {
        self.extend_from_slice(buf);
        Ok(())
    }
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


#[cfg(feature="xserde")]
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
            Order::Ascending|Order::Unordered =>
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
            Order::Ascending|Order::Unordered =>
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

#[cfg(feature="xserde")]
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
