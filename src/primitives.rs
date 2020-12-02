//! Ordered serialization/deserialization for primitive types and byte arrays.
//!
//! If you need to serialize or deserialize a primitive type (e.g. for use as a key), it is better
//! to use [SerializableValue] trait methods on primitive types directly, without using [`serde`].
//!
//! Serialize method `to_write()` writes results to [WriteBytes] trait impl,
//!  deserialize method `from_reader()` reads from [ReadBytes].
//!
//! **Deserializing a value which was serialized for different [`EncodingParams`](crate::params::EncodingParams)
//! is unchecked and is undefined behaviour!**
//!
//! Note that `u128` and `i128` may not be supported on some platforms.
//!
//! ### Encoding details
//! - unsigned integers are encoded in big-endian layout
//! - integers are encoded min-value-complemented, big-endian layout
//!
//! ### Parameters
//! Encoding parameters are passed via impl of `EncodingParams` (usually ZST struct).

use crate::{Result, Error, buf::{ReadBytes, WriteBytes}, params::{EncodingParams, Order, Endianness}};
use core::convert::TryInto;

/// Serializable value
///
/// This crate implements this trait for all primitive types. For complex types, use
/// provided _serde_ serializer and deserializer.
pub trait SerializableValue: Sized {
    fn to_writer<P: EncodingParams>(&self, writer: impl WriteBytes, params: P) -> Result;
    fn from_reader<P: EncodingParams>(reader: impl ReadBytes, params: P) -> Result<Self>;
}

/// Serialization data format version
pub const VERSION: u8 = 1;

macro_rules! ord_cond {
    ($param:ident, $desc:expr, $asc:expr) => {
        match <$param>::ORDER {
            Order::Ascending|Order::Unordered => { $asc },
            Order::Descending =>  { $desc },
        }
    }
}

macro_rules! to_bytes {
    ($param:ident, $v:expr) => {
        &match <$param>::ENDIANNESS {
            Endianness::Little => $v.to_le_bytes(),
            Endianness::Big    => $v.to_be_bytes(),
            Endianness::Native => $v.to_ne_bytes(),
        }
    }
}

macro_rules! from_bytes {
    ($param:ident, $ut:ty, $v:expr) => {
        match <$param>::ENDIANNESS {
            Endianness::Little => <$ut>::from_le_bytes($v.try_into().unwrap()),
            Endianness::Big    => <$ut>::from_be_bytes($v.try_into().unwrap()),
            Endianness::Native => <$ut>::from_ne_bytes($v.try_into().unwrap()),
        }
    }
}

// Ordered serialization of integers
macro_rules! serialize_int {
    ($ufn:ident, $ut:ty, $ifn:ident, $it:ty, $dufn:ident, $difn:ident) => {
        impl SerializableValue for $ut {
            #[inline]
            fn to_writer<P: EncodingParams>(&self, mut writer: impl WriteBytes, _params: P) -> Result
            {
                writer.write(to_bytes!(P, &{ord_cond!(P, !*self, *self)}))
            }
            #[inline]
            fn from_reader<P: EncodingParams>(mut reader: impl ReadBytes, _params: P) -> Result<Self>
            {
                const N: usize = core::mem::size_of::<$ut>();
                reader.read(N, |buf| {
                    let rv = from_bytes!(P, $ut, buf);
                    Ok(ord_cond!(P, !rv, rv))
                })
            }
        }
        impl SerializableValue for $it {
            #[inline]
            fn to_writer<P: EncodingParams>(&self, writer: impl WriteBytes, params: P) -> Result
            {
                ((self ^ <$it>::min_value()) as $ut).to_writer(writer, params)
            }
            #[inline]
            fn from_reader<P: EncodingParams>(reader: impl ReadBytes, params: P) -> Result<Self>
            {
                <$ut>::from_reader(reader, params).map(|u| { (u as $it) ^ <$it>::min_value() })
            }
        }
    }
}

serialize_int!(serialize_u8,  u8,  serialize_i8,  i8,  deserialize_u8,  deserialize_i8);
serialize_int!(serialize_u16, u16, serialize_i16, i16, deserialize_u16, deserialize_i16);
serialize_int!(serialize_u32, u32, serialize_i32, i32, deserialize_u32, deserialize_i32);
serialize_int!(serialize_u64, u64, serialize_i64, i64, deserialize_u64, deserialize_i64);

#[cfg(not(no_i128))]
serialize_int!(serialize_u128, u128, serialize_i128, i128, deserialize_u128, deserialize_i128);

impl SerializableValue for bool {
    fn to_writer<P: EncodingParams>(&self, writer: impl WriteBytes, params: P) -> Result {
        let v: u8 = if *self { 1 } else { 0 };
        v.to_writer(writer, params)
    }

    fn from_reader<P: EncodingParams>(reader: impl ReadBytes, params: P) -> Result<Self> {
        <u8>::from_reader(reader, params).map(|v| v != 0)
    }
}

impl SerializableValue for char {
    fn to_writer<P: EncodingParams>(&self, writer: impl WriteBytes, params: P) -> Result {
        (*self as u32).to_writer(writer, params)
    }

    fn from_reader<P: EncodingParams>(reader: impl ReadBytes, params: P) -> Result<Self> {
        let ch = u32::from_reader(reader, params)?;
        core::char::from_u32(ch).ok_or_else(|| Error::InvalidUtf8Encoding)
    }
}

// Ordered serialization of floats
macro_rules! serialize_float {
    ($ft:ty, $ift:ty, $uft:ty, $sfn:ident, $dfn:ident, $difn:ident) => {
        impl SerializableValue for $ft {
            #[inline]
            fn to_writer<P: EncodingParams>(&self, mut writer: impl WriteBytes, _params: P) -> Result {
                let t = self.to_bits() as $ift;
                let ov = if matches!(P::ENDIANNESS, Endianness::Big) {
                    const MSBOFFS: usize = core::mem::size_of::<$ift>() * 8 - 1; // # of bits - 1
                    t ^ ((t >> MSBOFFS) | <$ift>::min_value())
                } else {
                    t
                };
                writer.write(to_bytes!(P, &ord_cond!(P, !ov, ov)))
            }
            #[inline]
            fn from_reader<P: EncodingParams>(reader: impl ReadBytes, params: P) -> Result<Self> {
                const MSBOFFS: usize = core::mem::size_of::<$ift>() * 8 - 1; // # of bits - 1
                let val = <$uft>::from_reader(reader, params)? as $ift;
                if matches!(P::ENDIANNESS, Endianness::Big) {
                    let t = ((val ^ <$ift>::min_value()) >> MSBOFFS) | <$ift>::min_value();
                    Ok(<$ft>::from_bits((val ^ t) as $uft))
                } else {
                    Ok(<$ft>::from_bits(val as $uft))
                }
            }
        }
    }
}

serialize_float!(f32, i32, u32, serialize_f32, deserialize_f32, deserialize_u32);
serialize_float!(f64, i64, u64, serialize_f64, deserialize_f64, deserialize_u64);

/// Bitwise invert contents of a buffer
pub fn invert_buffer(buf: &mut [u8])
{
    for b in buf {
        *b = !*b;
    }
}