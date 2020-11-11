//! Ordered serialization/deserialization methods for primitive types and byte arrays.
//!
//! Serialize methods write results to `WriteBytes` trait impl, deserialize methods read from
//! `ReadBytes`. Both are defined on top of this crate.
//!
//! **Deserializing a value which was serialized for different order is undefined
//!   behaviour!**
//!
//! Note that `u128` and `i128` may not be supported on some platforms.
//!
//! ### Encoding details
//! - unsigned integers are encoded in big-endian layout
//! - integers are encoded min-value-complemented, big-endian layout
//!
//! ### Parameters
//! Encoding parameters are passed to methods via impl of `EncodingParams` (usually ZST struct).

use crate::{ReadBytes, WriteBytes, Result, Order, EncodingParams, Endianness};
use std::convert::TryInto;

/// Serialization data format version
pub const VERSION: u8 = 1;

#[macro_export]
macro_rules! ord_cond {
    ($param:ident, $desc:expr, $asc:expr) => {
        match $param.order() {
            Order::Ascending|Order::Unordered => { $asc },
            Order::Descending =>  { $desc },
        }
    }
}

macro_rules! to_bytes {
    ($param:ident, $v:expr) => {
        &match $param.endianness() {
            Endianness::Little => $v.to_le_bytes(),
            Endianness::Big    => $v.to_be_bytes(),
            Endianness::Native => $v.to_ne_bytes(),
        }
    }
}

macro_rules! from_bytes {
    ($param:ident, $ut:ty, $v:expr) => {
        match $param.endianness() {
            Endianness::Little => <$ut>::from_le_bytes($v.try_into().unwrap()),
            Endianness::Big    => <$ut>::from_be_bytes($v.try_into().unwrap()),
            Endianness::Native => <$ut>::from_ne_bytes($v.try_into().unwrap()),
        }
    }
}

// Ordered serialization of integers
macro_rules! serialize_int {
    ($ufn:ident, $ut:ty, $ifn:ident, $it:ty, $dufn:ident, $difn:ident) => {
        #[inline]
        pub fn $ufn(mut writer: impl WriteBytes, value: $ut, param: impl EncodingParams) -> Result
        {
            writer.write(to_bytes!(param, &{ord_cond!(param, !value, value)}))
        }
        #[inline]
        pub fn $ifn(writer: impl WriteBytes, value: $it, param: impl EncodingParams) -> Result
        {
            $ufn(writer, (value ^ <$it>::min_value()) as $ut, param)
        }
        #[inline]
        pub fn $dufn(mut reader: impl ReadBytes, param: impl EncodingParams) -> Result<$ut> {
            const N: usize = std::mem::size_of::<$ut>();
            reader.read(N, |buf| {
                let rv = from_bytes!(param, $ut, buf);
                Ok(ord_cond!(param, !rv, rv))
            })
        }
        #[inline]
        pub fn $difn(reader: impl ReadBytes, param: impl EncodingParams) -> Result<$it> {
            $dufn(reader, param).map(|u| { (u as $it) ^ <$it>::min_value() })
        }
    }
}

serialize_int!(serialize_u8,  u8,  serialize_i8,  i8,  deserialize_u8,  deserialize_i8);
serialize_int!(serialize_u16, u16, serialize_i16, i16, deserialize_u16, deserialize_i16);
serialize_int!(serialize_u32, u32, serialize_i32, i32, deserialize_u32, deserialize_i32);
serialize_int!(serialize_u64, u64, serialize_i64, i64, deserialize_u64, deserialize_i64);

#[cfg(not(no_i128))]
serialize_int!(serialize_u128, u128, serialize_i128, i128, deserialize_u128, deserialize_i128);

#[inline]
pub fn serialize_bool(writer: impl WriteBytes, v: bool, param: impl EncodingParams) -> Result
{
    serialize_u8(writer,if v { 1 } else { 0 }, param)
}

#[inline]
pub fn deserialize_bool(reader: impl ReadBytes, param: impl EncodingParams) -> Result<bool>
{
    deserialize_u8(reader, param).map(|v| v != 0)
}

#[inline]
pub fn serialize_char(writer: impl WriteBytes, v: char, param: impl EncodingParams) -> Result
{
    serialize_u32(writer, v as u32, param)
}

#[inline]
pub fn deserialize_char(reader: impl ReadBytes, param: impl EncodingParams) -> Result<char>
{
    let ch = deserialize_u32(reader, param)?;
    std::char::from_u32(ch).ok_or_else(|| errobj!(InvalidUtf8Encoding))
}

// Ordered serialization of floats
macro_rules! serialize_float {
    ($ft:ty, $ift:ty, $uft:ty, $sfn:ident, $dfn:ident, $difn:ident) => {
        #[inline]
        pub fn $sfn(mut writer: impl WriteBytes, value: $ft, param: impl EncodingParams) -> Result {
            let t = value.to_bits() as $ift;
            let ov = if matches!(param.endianness(), Endianness::Native) {
                t
            } else {
                const MSBOFFS: usize = std::mem::size_of::<$ift>() * 8 - 1; // # of bits - 1
                t ^ ((t >> MSBOFFS) | <$ift>::min_value())
            };
            writer.write(to_bytes!(param, &ord_cond!(param, !ov, ov)))
        }
        #[inline]
        pub fn $dfn(reader: impl ReadBytes, param: impl EncodingParams) -> Result<$ft> {
            const MSBOFFS: usize = std::mem::size_of::<$ift>() * 8 - 1; // # of bits - 1
            let val = $difn(reader, param)? as $ift;
            if matches!(param.endianness(), Endianness::Native) {
                Ok(<$ft>::from_bits(val as $uft))
            } else {
                let t = ((val ^ <$ift>::min_value()) >> MSBOFFS) | <$ift>::min_value();
                Ok(<$ft>::from_bits((val ^ t) as $uft))
            }
        }
    }
}

serialize_float!(f32, i32, u32, serialize_f32, deserialize_f32, deserialize_u32);
serialize_float!(f64, i64, u64, serialize_f64, deserialize_f64, deserialize_u64);

