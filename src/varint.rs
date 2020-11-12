//!
//! Variable length serialization of integers
//!
use crate::{ReadBytes, WriteBytes, TailReadBytes, TailWriteBytes, Result, Error, LengthEncoder, SerializerParams, WriteToTail, ReadFromTail};

// Varint code adaped and modified from the source below:
// VInt implementation: github.com/iqlusioninc/veriform
/// Get the length of an encoded u64 for the given value in bytes
#[must_use] #[inline]
pub fn varu64_encoded_len(value: u64) -> u8 {
    // indexing const array is twice as fast as 'match' in release mode
    const LENGTHS: [u8; 65] = [ 9,9,9,9,9,9,9,9,8,8,8,8,8,8,8,7,7,7,7,7,7,7,6,6,6,6,6,6,6,
        5,5,5,5,5,5,5,4,4,4,4,4,4,4,3,3,3,3,3,3,3,2,2,2,2,2,2,2,1,1,1,1,1,1,1,1 ];
    LENGTHS[value.leading_zeros() as usize]
}

#[must_use] #[inline]
pub fn varu32_encoded_len(value: u32) -> u8 {
    const LENGTHS: [u8; 33] = [ 5,5,5,5,4,4,4,4,4,4,4,3,3,3,3,3,3,3,2,2,2,2,2,2,2,1,1,1,1,1,1,1,1 ];
    LENGTHS[value.leading_zeros() as usize]
}


/// Get the byte length of encoded `u64`  or `u32` value from the first byte
#[must_use] #[inline]
pub fn varu_decoded_len(first_byte: u8) -> u8 {
    // truncation can't happen, max value is 8
    #![allow(clippy::cast_possible_truncation)]
    (first_byte.trailing_zeros() + 1) as u8
}

/// Encode `u64` as variable length bytes
pub fn varu64_encode_to_writer(mut writer: impl WriteBytes, value: u64) -> Result {
    let mut bytes = [0_u8; 9];
    let length = varu64_encode_to_slice(&mut bytes, value);
    writer.write(&[bytes[0]])?;
    writer.write(&bytes[1..length as usize])
}

/// Encode `u32` as variable length bytes
pub fn varu32_encode_to_writer(mut writer: impl WriteBytes, value: u32) -> Result {
    let mut bytes = [0_u8; 5];
    let length = varu32_encode_to_slice(&mut bytes, value);
    writer.write(&[bytes[0]])?;
    writer.write(&bytes[1..length as usize])
}

/// Encode `u64` as variable length bytes into fixed size buffer, returns encoded bytes length
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

/// Encode `u64` as variable length bytes into fixed size buffer, returns encoded bytes length
pub fn varu32_encode_to_slice(bytes: &mut[u8; 5], value: u32) -> u8 {
    let length = varu32_encoded_len(value);
    // 5-byte special case, length byte is zero in this case
    if length == 5 {
        bytes[0] = 0xf0;
        bytes[1..].copy_from_slice(&value.to_le_bytes());
    } else {
        let encoded = (value << 1 | 1) << (u32::from(length) - 1);
        bytes[..4].copy_from_slice(&encoded.to_le_bytes());
    }
    length
}

// Decode variable length bytes into `u64`, when decoded length is known
// from previous call of `varu64_decoded_len()`
fn varu64_decode(encoded_length: u8, first_byte: u8, bytes: &[u8]) -> Result<u64> {
    if bytes.len() + 1 < encoded_length as usize {
        return Err(Error::PrematureEndOfInput);
    }
    let mut encoded = [0_u8; 8];
    let result = if encoded_length == 9 {
        // 9-byte special case
        encoded.copy_from_slice(&bytes[0..8]);
        u64::from_le_bytes(encoded)
    } else {
        encoded[0] = first_byte;
        let len = encoded_length as usize;
        encoded[1..len as usize].copy_from_slice(&bytes[..len-1 as usize]);
        u64::from_le_bytes(encoded) >> encoded_length
    };
    #[cfg(debug_assertions)]
    if !(encoded_length == 1 || result >= (1 << (7 * (encoded_length - 1)))) {
        return Err(Error::InvalidVarintEncoding);
    }
    Ok(result)
}

// Decode variable length bytes into `u32`, when decoded length is known
// from previous call of `varu_decoded_len()`. `encoded_length` must be less or equal to 5.
fn varu32_decode(encoded_length: u8, first_byte: u8, bytes: &[u8]) -> Result<u32> {
    if bytes.len() + 1 < encoded_length as usize {
        return Err(Error::PrematureEndOfInput);
    }
    let mut encoded = [0_u8; 4];
    let result = if encoded_length == 5 {
        // 5-byte special case
        encoded.copy_from_slice(&bytes[0..4]);
        u32::from_le_bytes(encoded)
    } else {
        encoded[0] = first_byte;
        let len = encoded_length as usize;
        encoded[1..len as usize].copy_from_slice(&bytes[..len-1 as usize]);
        u32::from_le_bytes(encoded) >> encoded_length
    };
    #[cfg(debug_assertions)]
    if !(encoded_length == 1 || result >= (1 << (7 * (encoded_length - 1)))) {
        return Err(Error::InvalidVarintEncoding);
    }
    Ok(result)
}

/// Decode variable length bytes into `u64`, returns value and encoded length
pub fn varu64_decode_from_slice(bytes: &[u8]) -> Result<(u64, u8)> {
    if bytes.is_empty() {
        return Err(Error::PrematureEndOfInput);
    }
    let decoded_len = varu_decoded_len(bytes[0]);
    Ok((varu64_decode(decoded_len, bytes[0], &bytes[1..])?, decoded_len))
}

/// Decode variable length bytes into `u32`, returns value and encoded length
pub fn varu32_decode_from_slice(bytes: &[u8]) -> Result<(u32, u8)> {
    if bytes.is_empty() {
        return Err(Error::PrematureEndOfInput);
    }
    let decoded_len = varu_decoded_len(bytes[0]);
    if decoded_len <= 5 {
        Ok((varu32_decode(decoded_len, bytes[0], &bytes[1..])?, decoded_len))
    } else {
        Err(Error::InvalidVarintEncoding)
    }
}

/// Decode variable length bytes into `u64`
pub fn varu64_decode_from_reader(mut reader: impl ReadBytes) -> Result<u64> {
    let (first_byte, decoded_len) = reader.read(1, |buf| {
        Ok((buf[0], varu_decoded_len(buf[0])))
    })?;
    reader.read(decoded_len as usize, |buf| {
        varu64_decode(decoded_len, first_byte,buf)
    })
}

/// Decode variable length bytes into `u64`
pub fn varu32_decode_from_reader(mut reader: impl ReadBytes) -> Result<u32> {
    let (first_byte, decoded_len) = reader.read(1, |buf| {
        Ok((buf[0], varu_decoded_len(buf[0])))
    })?;
    if decoded_len <= 5 {
        reader.read(decoded_len as usize, |buf| {
            varu32_decode(decoded_len, first_byte, buf)
        })
    } else {
        Err(Error::InvalidVarintEncoding)
    }
}

/// Variable-length encoding for array lengths, enum discriminants etc.
pub struct VarIntLenEncoder<P> where P: SerializerParams {
    _marker: std::marker::PhantomData<P>,
}

#[cfg(target_pointer_width = "64")]
impl<P> LengthEncoder for VarIntLenEncoder<P> where P: SerializerParams {
    type Value = usize;

    #[inline]
    fn calc_size(value: Self::Value) -> usize {
        varu64_encoded_len(value as u64) as usize
    }
    #[inline]
    #[allow(clippy::cast_possible_truncation)] // can't happen because of cfg
    fn read(mut reader: impl TailReadBytes) -> Result<usize> {
        if P::USE_TAIL {
            varu64_decode_from_reader(ReadFromTail(&mut reader)).map(|v| v as usize)
        } else {
            varu64_decode_from_reader(reader).map(|v| v as usize)
        }
    }
    #[inline]
    fn write(mut writer: impl TailWriteBytes, value: usize) -> Result {
        if P::USE_TAIL {
            varu64_encode_to_writer(WriteToTail(&mut writer), value as u64)
        } else {
            varu64_encode_to_writer(writer, value as u64)
        }
    }
}

#[cfg(not(target_pointer_width = "64"))]
#[allow(clippy::cast_possible_truncation)] // can't happen because of cfg
impl LengthEncoder for VarIntLenEncoder {
    #[inline]
    fn calc_size(value: usize) -> usize {
        varu32_encoded_len(value as u32) as usize
    }
    #[inline]
    fn read(reader: impl ReadBytes, _params: impl EncodingParams) -> Result<usize> {
        varu32_decode_from_reader(reader).map(|v| v as usize)
    }
    #[inline]
    fn write(writer: impl WriteBytes, _params: impl EncodingParams, value: usize) -> Result {
        varu32_encode_to_writer(writer, value as u32)
    }
}

/// Variable-length encoding for enum discriminants
pub struct VarIntDiscrEncoder;

#[cfg(target_pointer_width = "64")]
impl LengthEncoder for VarIntDiscrEncoder {
    type Value = u32;

    #[inline]
    fn calc_size(value: Self::Value) -> usize {
        varu32_encoded_len(value) as usize
    }
    #[inline]
    fn read(reader: impl TailReadBytes) -> Result<Self::Value> {
        varu32_decode_from_reader(reader)
    }
    #[inline]
    fn write(writer: impl TailWriteBytes, value: Self::Value) -> Result {
        varu32_encode_to_writer(writer, value)
    }
}