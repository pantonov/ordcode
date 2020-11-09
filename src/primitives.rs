//! Ordered serialization/deserialization methods for primitive types and byte arrays.
//!
//! Serialize methods write results to `WriteBytes` trait impl, deserialize methods read from
//! `ReadBytes`. Both are defined on top of this crate.
//!
//! All methods in this module have `desc` parameter, which means serialization for ascending
//! order if set to `false`, for descending order if set to `true`.
//!
//! **Deserializing a value which was serialized for different order is undefined
//!   behaviour!**
//!
//! Note that `u128` and `i128` may not be supported on some platforms.
//!
//! ### Encoding details
//! - unsigned integers are encoded in big-endian layout
//! - integers are encoded min-value-complemented, big-endian layout
//! - byte sequences are escaped. Byte value `0xF8` is escaped as `{ 0xF8, 0xFF }` for
//!   ascending order, `{ 0x07, 0x00 }` for descending order. Sequence is terminated by
//!   `{ 0xF8, 0x01 }` for ascending order, `{ 0x07, 0xFE }` for descending order. Escaped byte
//!   value `0xF8` is chosen because it does not appear in valid UTF-8, and escaping zero
//!   is impractical (it is too common)
//! - varint encoding DOES NOT HOLD lexicographical ordering invariant. varint64 can use
//!   from 1 to 9 bytes.

use crate::{ ReadBytes, WriteBytes, Result, ResultExt, ErrorKind,
             Order, BytesBuf, BytesBufExt };
use std::convert::TryInto;

/// Serialization data format version
pub const VERSION: u8 = 1;

macro_rules! ord_cond {
    ($order:ident, $desc:expr, $asc:expr) => {
        match $order {
            Order::Ascending =>   { $asc  },
            Order::Descending =>  { $desc },
            Order::Unspecified => { $asc },
        }
    }
}

// Ordered serialization of integers
macro_rules! serialize_int {
    ($ufn:ident, $ut:ty, $ifn:ident, $it:ty, $dufn:ident, $difn:ident) => {
        #[inline]
        pub fn $ufn(writer: &mut impl WriteBytes, value: $ut, order: Order) -> Result {
            writer.write(&{ord_cond!(order, !value, value)}.to_be_bytes())
        }
        #[inline]
        pub fn $ifn(writer: &mut impl WriteBytes, value: $it, order: Order) -> Result {
            $ufn(writer, (value ^ <$it>::min_value()) as $ut, order)
        }
        #[inline]
        pub fn $dufn(reader: &mut impl ReadBytes, order: Order) -> Result<$ut> {
            const N: usize = std::mem::size_of::<$ut>();
            reader.apply_bytes(N, true, |buf| {
                let rv = <$ut>::from_be_bytes(buf.try_into().unwrap());
                Ok(ord_cond!(order, !rv, rv))
            })
        }
        #[inline]
        pub fn $difn(reader: &mut impl ReadBytes, order: Order) -> Result<$it> {
            $dufn(reader, order).map(|u| { (u as $it) ^ <$it>::min_value() })
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
pub fn serialize_bool(writer: &mut impl WriteBytes, v: bool, order: Order) -> Result
{
    serialize_u8(writer,if v { 1 } else { 0 }, order)
}

#[inline]
pub fn deserialize_bool(reader: &mut impl ReadBytes, order: Order) -> Result<bool>
{
    deserialize_u8(reader, order).map(|v| v != 0)
}

#[inline]
pub fn serialize_char(writer: &mut impl WriteBytes, v: char, order: Order) -> Result
{
    serialize_u32(writer, v as u32, order)
}

#[inline]
pub fn deserialize_char(reader: &mut impl ReadBytes, order: Order) -> Result<char>
{
    let ch = deserialize_u32(reader, order)?;
    std::char::from_u32(ch).ok_or_else(|| errobj!(InvalidUtf8Encoding))
}

// Ordered serialization of floats
macro_rules! serialize_float {
    ($ft:ty, $ift:ty, $uft:ty, $sfn:ident, $dfn:ident, $difn:ident) => {
        #[inline]
        pub fn $sfn(writer: &mut impl WriteBytes, value: $ft, order: Order) -> Result {
            let t = value.to_bits() as $ift;
            const MSBOFFS: usize = std::mem::size_of::<$ift>() * 8 - 1; // # of bits - 1
            let ov = t ^ ((t >> MSBOFFS) | <$ift>::min_value());
            writer.write(&ord_cond!(order, !ov, ov).to_be_bytes())
        }
        #[inline]
        pub fn $dfn(reader: &mut impl ReadBytes, order: Order) -> Result<$ft> {
            const MSBOFFS: usize = std::mem::size_of::<$ift>() * 8 - 1; // # of bits - 1
            let val = $difn(reader, order)? as $ift;
            let t = ((val ^ <$ift>::min_value()) >> MSBOFFS) | <$ift>::min_value();
            Ok(<$ft>::from_bits((val ^ t) as $uft))
        }
    }
}

serialize_float!(f32, i32, u32, serialize_f32, deserialize_f32, deserialize_u32);
serialize_float!(f64, i64, u64, serialize_f64, deserialize_f64, deserialize_u64);

// Escape and terminator sequences for prefix-free byte array encoding.
struct ByteStrEscapes { start: u8, esc: u8, term: u8 }
const BSTR_ESCAPE_ASC: ByteStrEscapes  = ByteStrEscapes { start: 0xF8, esc: 0xFF, term: 0x01 };
const BSTR_ESCAPE_DESC: ByteStrEscapes = ByteStrEscapes {
    start: !BSTR_ESCAPE_ASC.start, esc: !BSTR_ESCAPE_ASC.esc, term: !BSTR_ESCAPE_ASC.term
};

// Calculates unescaped length of escaped sequence, does not advance reader
#[inline]
fn unescaped_length(rb: &mut impl ReadBytes, esc: &ByteStrEscapes) -> Result<usize> {
    let mut len = 0_usize;
    rb.apply_over_esc(esc.start, false, &mut |buf, c| {
        if c == esc.esc {
            len += buf.len();
            Ok(true)
        } else if c == esc.term {
            len += buf.len() - 1;
            Ok(false)
        } else {
            err!(InvalidByteSequenceEscape)
        }
    }).and(Ok(len))
}

/// Calculate length of pending byte sequence from reader
#[inline]
pub fn bytes_length(reader: &mut impl ReadBytes, order: Order) -> Result<usize> {
    ord_cond!(order, unescaped_length(reader, &BSTR_ESCAPE_DESC),
              unescaped_length(reader, &BSTR_ESCAPE_ASC))
}

/// Serialize byte sequence to escaped representation
pub fn serialize_bytes(writer: &mut impl WriteBytes, value: &[u8], order: Order) -> Result {
    ord_cond!(order, {
        for b in value {
            if BSTR_ESCAPE_ASC.start == *b {
                writer.write_byte(BSTR_ESCAPE_DESC.start)?;
                writer.write_byte(BSTR_ESCAPE_DESC.esc)?;
            } else {
                writer.write_byte(!*b)?;
            }
        }
        writer.write_byte(BSTR_ESCAPE_DESC.start)?;
        writer.write_byte(BSTR_ESCAPE_DESC.term)
    }, {
        for b in value {
            if BSTR_ESCAPE_ASC.start == *b {
                writer.write_byte(BSTR_ESCAPE_ASC.start)?;
                writer.write_byte(BSTR_ESCAPE_ASC.esc)?;
            } else {
                writer.write_byte(*b)?;
            }
        }
        writer.write_byte(BSTR_ESCAPE_ASC.start)?;
        writer.write_byte(BSTR_ESCAPE_ASC.term)
    })
}

fn read_escaped_bytes_asc(rb: &mut impl ReadBytes, out: &mut impl WriteBytes) -> Result
{
    rb.apply_over_esc(BSTR_ESCAPE_ASC.start, true, &mut |buf, c| {
        if c == BSTR_ESCAPE_ASC.esc {
            out.write(&buf[..buf.len()])?;
            Ok(true)
        } else if c == BSTR_ESCAPE_ASC.term {
            out.write(&buf[..buf.len() - 1])?;
            Ok(false)
        } else {
            err!(InvalidByteSequenceEscape)
        }
    })
}

fn read_escaped_bytes_desc(rb: &mut impl ReadBytes, out: &mut impl WriteBytes) -> Result
{
    rb.apply_over_esc(BSTR_ESCAPE_DESC.start, true, &mut |buf, c| {
        if c == BSTR_ESCAPE_DESC.esc {
            write_complement_bytes(out,&buf[..buf.len()])?;
            Ok(true)
        } else if c == BSTR_ESCAPE_DESC.term {
            write_complement_bytes(out,&buf[..buf.len() - 1])?;
            Ok(false)
        } else {
            err!(InvalidByteSequenceEscape)
        }
    })
}

/// Deserialize escaped byte sequence and write result to `WriteBytes`
#[inline]
pub fn deserialize_bytes_to_writer(reader: &mut impl ReadBytes, out: &mut impl WriteBytes, order: Order) -> Result
{
    ord_cond!(order, read_escaped_bytes_desc(reader, out),
              read_escaped_bytes_asc(reader, out))
}

/// Deserialize escaped byte sequence to `VecBuf`
pub fn deserialize_bytes(reader: &mut impl ReadBytes, order: Order) -> Result<BytesBuf> {
    let len = bytes_length(reader, order)?;
    let mut v = BytesBuf::with_reserve(len);
    deserialize_bytes_to_writer(reader, &mut v, order)?;
    Ok(v)
}

/// Write 0xFF bitwise complement of input
#[inline]
pub fn write_complement_bytes(writer: &mut impl WriteBytes, input: &[u8]) -> Result {
    for v in input {
        writer.write_byte(!*v)?;
    }
    Ok(())
}

/// Serialize whole input buffer as ordered byte string, no escaping and termination sequences.
/// This method copies source byte buffer for `Ascending` order, or bitwise complements
/// if ordering is `Descending`.
pub fn serialize_bytes_noesc(writer: &mut impl WriteBytes, v: &[u8], order: Order) -> Result
{
    ord_cond!(order, write_complement_bytes(writer, v), writer.write(v))
}

/// Deserialize input buffer as ordered byte string into writer, no escaping and termination sequences
pub fn deserialize_bytes_noesc_to_writer(reader: &mut impl ReadBytes, writer: &mut impl WriteBytes, order: Order) -> Result
{
    ord_cond!(order, reader.apply_all(|v| write_complement_bytes(writer, v)),
              reader.apply_all(|v| writer.write(v)))
}

/// Deserialize input buffer as ordered byte string to `VecBuf`, no escaping and termination sequences
pub fn deserialize_bytes_noesc(reader: &mut impl ReadBytes, order: Order) -> Result<BytesBuf>
{
    reader.apply_all(|v| {
        let mut res = BytesBuf::with_reserve(v.len());
        ord_cond!(order, { write_complement_bytes(&mut res, v)?; },
                  { res.extend_from_slice(v); });
        Ok(res)
    })
}

/// Deserialize input buffer as ordered bytes to `String`, no escaping and termination sequences
pub fn deserialize_bytes_noesc_to_string(reader: &mut impl ReadBytes, order: Order) -> Result<String>
{
    let bstr = deserialize_bytes_noesc(reader, order)?;
    let s = String::from_utf8(bstr.into_vec8()).chain_err(|| ErrorKind::InvalidUtf8Encoding)?;
    Ok(s)
}

// Varint code adaped and modified from the source below:
// VInt implementation: github.com/iqlusioninc/veriform

/// Get the length of an encoded u64 for the given value in bytes
#[must_use]
pub fn varu64_encoded_len(value: u64) -> u8 {
    // indexing const array is twice as fast as 'match' in release mode
    const LENGTHS: [u8; 65] = [ 9,9,9,9,9,9,9,9,8,8,8,8,8,8,8,7,7,7,7,7,7,7,6,6,6,6,6,6,6,
        5,5,5,5,5,5,5,4,4,4,4,4,4,4,3,3,3,3,3,3,3,2,2,2,2,2,2,2,1,1,1,1,1,1,1,1 ];
    LENGTHS[value.leading_zeros() as usize]
}

/// Get the byte length of encoded `u64` value from the first byte
#[inline]
#[must_use]
pub fn varu64_decoded_len(first_byte: u8) -> u8 {
    // truncation can't happen, max value is 8
    #![allow(clippy::cast_possible_truncation)]
    (first_byte.trailing_zeros() + 1) as u8
}

/// Encode `u64` as variable length bytes
#[inline]
pub fn varu64_encode_to_writer<W: WriteBytes>(writer: &mut W, value: u64) -> Result {
    let mut bytes = [0_u8; 9];
    let length = varu64_encode_to_slice(&mut bytes, value);
    writer.write(&bytes[0..length as usize])
}

/// Encode `u64` as variable length bytes into fixed size buffer, returns encoded bytes length
#[inline]
pub fn varu64_encode_to_slice(bytes: &mut[u8; 9], value: u64) -> u8 {
    let length = varu64_encoded_len(value);
    // 9-byte special case, length byte is zero in this case
    if length == 9 {
        bytes[1..].copy_from_slice(&value.to_le_bytes());
    } else {
        let encoded = (value << 1 | 1) << (u64::from(length) - 1);
        bytes[..8].copy_from_slice(&encoded.to_le_bytes());
    }
    length
}

/// Decode variable length bytes into `u64`, when decoded length is known
/// from previous call of `varu64_decoded_len()`
#[inline]
pub fn varu64_decode(encoded_length: u8, bytes: &[u8]) -> Result<u64> {
    if bytes.len() < encoded_length as usize {
        return err!(PrematureEndOfInput);
    }
    let mut encoded = [0_u8; 8];
    let result = if encoded_length == 9 {
        // 9-byte special case
        encoded.copy_from_slice(&bytes[1..9]);
        u64::from_le_bytes(encoded)
    } else {
        encoded[..encoded_length as usize].copy_from_slice(&bytes[..encoded_length as usize]);
        u64::from_le_bytes(encoded) >> encoded_length
    };
    #[cfg(debug_assertions)]
    if !(encoded_length == 1 || result >= (1 << (7 * (encoded_length - 1)))) {
        return err!(InvalidVarintEncoding);
    }
    Ok(result)
}

/// Decode variable length bytes into `u64`, returns value and encoded length
#[inline]
pub fn varu64_decode_from_bytes(bytes: &[u8]) -> Result<(u64, u8)> {
    if bytes.is_empty() {
        return err!(PrematureEndOfInput);
    }
    let decoded_len = varu64_decoded_len(bytes[0]);
    Ok((varu64_decode(decoded_len, bytes)?, decoded_len))
}

/// Decode variable length bytes into `u64`
#[inline]
pub fn varu64_decode_from_reader<R: ReadBytes>(reader: &mut R) -> Result<u64> {
    let decoded_len = reader.apply_bytes(1, false, |buf| {
        Ok(varu64_decoded_len(buf[0]))
    })?;
    reader.apply_bytes(decoded_len as usize, true, |buf| {
        varu64_decode(decoded_len, buf)
    })
}