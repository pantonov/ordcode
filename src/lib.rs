//! A set of primitives and [Serde](https://serde.rs) serializers for
//! fast, prefix-free encoding which preserves lexicographical ordering of values.
//!
//! It is intended for encoding keys and values in key-value databases.
//!
//! *Features:*
//!
//! * encoding in both ascending and descending lexicographical orderings are supported
//! * encoding puts lengths of variable-size sequences to the end of serialized data,
//!   so resulting encoding is prefix-free and friendly to lexicographical ordering
//! * zero allocations, supports `#[no_std]` environments
//! * method to cheaply get exact size of serialized data without doing actual serialization,
//!   for effective buffer management
//! * space-efficient varint encoding for sequence lengths and discriminants
//! * easily customizable (endianness, encoding of primitive types etc.), with useful pre-sets
//! * reader/writer traits for double-ended buffers, so you can implement your own or use
//!   implementations provided by the crate
//!
//! ### Cargo.toml features and dependencies
//!
//! * `serde` (on by default): include `serde` serializer and deserializer.
//!    If you need only primitives, you can opt out.
//! * `std` (on by default): opt out for `#[no-std]` use, you will lose some utility methods
//!   which use `Vec`
//!
//! ### Stability guarantees
//! The underlying encoding format is simple and unlikely to change.
//! As a safeguard, `Serializer` implements `FormatVersion` trait for all cparameter pre-sets.
//!
//! Note: serializing with descending lexicographical order is particularly useful for key-value
//! databases like _rocksdb_, where reverse iteration is slower than forward iteration.

//#![doc(html_root_url = "https://docs.rs/ordcode")]
#![crate_name = "biord"]

#![deny(clippy::all, clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]

#[cfg(feature="serde")] #[macro_use] extern crate serde;

#[macro_use] mod errors;
#[doc(inline)]
pub use errors::Error;

/// A convenient Result type
pub type Result<T = (), E = errors::Error> = core::result::Result<T, E>;

#[macro_use] pub mod primitives;
pub mod varint;
pub mod bytes_esc;

mod size_calc;
pub mod params;
pub mod buf;

#[doc(inline)]
pub use params::Order;

#[doc(inline)]
pub use crate::ord_ser::Serializer;
pub use crate::ord_de::Deserializer;

pub use buf::{DeBytesReader, DeBytesWriter, ReadFromTail, WriteToTail };

#[cfg(feature="serde")] mod ord_ser;
#[cfg(feature="serde")] mod ord_de;
//#[cfg(feature="serde")] mod bin_ser;
//#[cfg(feature="serde")] mod bin_de;

/// Current version of data encoding format for `Serializer` parametrized with some of `SerializerParams`.
pub trait FormatVersion<P: params::SerializerParams> {
    const VERSION: u32;
}

/// Calculate exact size of serialized data for a `serde::Serialize` value.
///
/// Useful for calculating exact size of serialized objects for buffer allocations.
/// Calculation process is inexpensive, for fixed-size objects it evaluates to compile-time
/// constant, or a few `len()` method calls for variable-size objects.
///
/// ```
/// # use biord::*;
/// # use serde::ser::Serialize;
///
/// #[derive(serde_derive::Serialize)]
/// struct Foo(u32, String);
/// let foo = Foo(1, "abc".to_string());
///
/// let data_size = calc_size(&foo, params::AscendingOrder).unwrap();
/// assert_eq!(data_size, 8);
/// ```
pub fn calc_size<T, P>(value: &T, _params: P) -> Result<usize>
    where T: ?Sized + serde::ser::Serialize,
          P: params::SerializerParams,
{
    let mut sc = size_calc::SizeCalc::<P>::new();
    value.serialize(&mut sc)?;
    Ok(sc.size())
}

pub fn ser_to_vec_ordered<T>(value: &T, order: Order) -> Result<Vec<u8>>
    where T: ?Sized + serde::ser::Serialize,
{
    let mut byte_buf = vec![0u8; calc_size(value, params::AscendingOrder)?];
    let mut bi_buf = DeBytesWriter::new(byte_buf.as_mut_slice());
    let mut ser = Serializer::new(&mut bi_buf, params::AscendingOrder);
    value.serialize(&mut ser)?;
    bi_buf.is_complete()?;
    if matches!(order, Order::Descending) {
        primitives::invert_buffer(&mut byte_buf);
    }
    Ok(byte_buf)
}

pub fn de_from_bytes_ordered_asc<T>(input: &[u8]) -> Result<T>
    where T: serde::de::DeserializeOwned,
{
    let mut reader = DeBytesReader::new(input);
    let mut deser = Deserializer::new(&mut reader, params::AscendingOrder);
    T::deserialize(&mut deser)
}

pub fn de_from_bytes_ordered<T>(mut input: &mut [u8], order: Order) -> Result<T>
    where T: serde::de::DeserializeOwned,
{
    if matches!(order, Order::Descending) {
        primitives::invert_buffer(&mut input);
    }
    let mut reader = DeBytesReader::new(input);
    let mut deser = Deserializer::new(&mut reader, params::AscendingOrder);
    T::deserialize(&mut deser)
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

    use crate::{Result, Order, ReadBytes, WriteBytes, BytesBuf, DeBytesReader};

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
        from_bytes_reader(&mut DeBytesReader::new(input), order)
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

    use crate::{Result, WriteBytes, BytesBuf, DeBytesReader, ReadBytes};

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
        from_bytes_reader(&mut DeBytesReader::new(input))
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
