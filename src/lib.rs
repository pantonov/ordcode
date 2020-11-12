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

#[macro_use] mod errors;
pub use errors::{ Error, ErrorKind, ResultExt };

/// A convenient Result type
pub type Result<T = (), E = errors::Error> = core::result::Result<T, E>;

#[macro_use] pub mod primitives;
pub mod varint;
pub mod bytes_esc;

mod hint_ser;
mod params;
mod bytesbuf;
mod readwrite;

#[doc(inline)]
pub use hint_ser::SizeCalc;

#[doc(inline)]
pub use params::{ Order, Endianness, LenEncoder, EncodingParams, SerializerParams,
                  AscendingOrder, DescendingOrder };

#[doc(inline)]
pub use readwrite::{ReadBytes, WriteBytes, ReadFromTail, WriteToTail, BytesReader, BiBuffer };

#[doc(inline)]
#[cfg(features="std")]
pub use bytesbuf::{BytesBuf, BytesBufExt };

//#[cfg(feature="serde")] mod ord_ser;
//#[cfg(feature="serde")] mod ord_de;
//#[cfg(feature="serde")] mod bin_ser;
//#[cfg(feature="serde")] mod bin_de;


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
