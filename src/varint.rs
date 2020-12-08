//!
//! Fast variable length serialization of unsigned integers with [`VarUInt`] trait.
//!
//! This trait is implemented by this crate for [`u32`], [`u64`] integer types.
use crate::{buf::{ReadBytes, WriteBytes, TailReadBytes, TailWriteBytes, WriteToTail, ReadFromTail},
            params::LengthEncoder, Result, Error};

/// Methods for variable length serializaiton of unsigned integers
pub trait VarUInt: Sized {
    /// Get the length of an varint-encoded value in bytes
    fn varu_encoded_len(&self) -> u8;

    /// Get the byte length of varint-encoded value from the first byte
    fn varu_decoded_len(first_byte: u8) -> u8;

    /// Encode as variable length integer to `writer`
    fn varu_to_writer(&self, writer: impl WriteBytes) -> Result;

    /// Read variable length integer from `reader`
    fn varu_from_reader(reader: impl ReadBytes) -> Result<Self>;

    /// Decode variable length integer from slice
    fn varu_from_slice(bytes: &[u8]) -> Result<(Self, u8)>;
    
    /// Encode variable length integer into slice
    ///
    /// Slice must be of enough length, can be calculated with `varu_encoded_len()`.
    /// Returns actual length of encoded varint.
    fn varu_to_slice(&self, bytes: &mut [u8]) -> u8;
}

impl VarUInt for u64 {
    #[inline]
    fn varu_encoded_len(&self) -> u8 {
        // indexing const array is twice as fast as 'match' in release mode
        const LENGTHS: [u8; 65] = [ 9,9,9,9,9,9,9,9,8,8,8,8,8,8,8,7,7,7,7,7,7,7,6,6,6,6,6,6,6,
            5,5,5,5,5,5,5,4,4,4,4,4,4,4,3,3,3,3,3,3,3,2,2,2,2,2,2,2,1,1,1,1,1,1,1,1 ];
        LENGTHS[self.leading_zeros() as usize]
    }
    #[inline]
    fn varu_decoded_len(first_byte: u8) -> u8 {
        #![allow(clippy::cast_possible_truncation)]
        (first_byte.trailing_zeros() + 1) as u8    
    }
    #[inline]
    fn varu_to_writer(&self, mut writer: impl WriteBytes) -> Result {
        let mut bytes = [0_u8; 9];
        let length = self.varu_to_slice(&mut bytes);
        writer.write(&[bytes[0]])?;
        writer.write(&bytes[1..length as usize])    
    }
    #[inline]
    fn varu_from_reader(mut reader: impl ReadBytes) -> Result<Self> {
        let (first_byte, varu_decoded_len) = reader.read(1, |buf| {
            Ok((buf[0], Self::varu_decoded_len(buf[0])))
        })?;
        reader.read((varu_decoded_len - 1) as usize, |buf| {
            varu64_decode(varu_decoded_len, first_byte,buf)
        })   
    }
    #[inline]
    fn varu_from_slice(bytes: &[u8]) -> Result<(Self, u8)> {
        if bytes.is_empty() {
            return Err(Error::PrematureEndOfInput);
        }
        let varu_decoded_len = Self::varu_decoded_len(bytes[0]);
        Ok((varu64_decode(varu_decoded_len, bytes[0], &bytes[1..])?, varu_decoded_len))   
    }
    #[inline]
    fn varu_to_slice(&self, bytes: &mut [u8]) -> u8 {
        let length = self.varu_encoded_len();
        // 9-byte special case, length byte is zero in this case
        if length == 9 {
            bytes[1..].copy_from_slice(&self.to_le_bytes());
        } else {
            let encoded = (*self << 1 | 1) << (u64::from(length) - 1);
            bytes[..8].copy_from_slice(&encoded.to_le_bytes());
        }
        length     
    }
}

impl VarUInt for u32 {
    #[inline]
    fn varu_encoded_len(&self) -> u8 {
        const LENGTHS: [u8; 33] = [ 5,5,5,5,4,4,4,4,4,4,4,3,3,3,3,3,3,3,2,2,2,2,2,2,2,1,1,1,1,1,1,1,1 ];
        LENGTHS[self.leading_zeros() as usize]
    }
    #[inline]
    fn varu_decoded_len(first_byte: u8) -> u8 {
        <u64>::varu_decoded_len(first_byte)
    }
    #[inline]
    fn varu_to_writer(&self, mut writer: impl WriteBytes) -> Result {
        let mut bytes = [0_u8; 5];
        let length = self.varu_to_slice(&mut bytes);
        writer.write(&[bytes[0]])?;
        writer.write(&bytes[1..length as usize])   
    }
    #[inline]
    fn varu_from_reader(mut reader: impl ReadBytes) -> Result<Self> {
        let (first_byte, varu_decoded_len) = reader.read(1, |buf| {
            Ok((buf[0], Self::varu_decoded_len(buf[0])))
        })?;
        if varu_decoded_len <= 5 {
            reader.read((varu_decoded_len - 1) as usize, |buf| {
                varu32_decode(varu_decoded_len, first_byte, buf)
            })
        } else {
            Err(Error::InvalidVarintEncoding)
        }
    }
    #[inline]
    fn varu_from_slice(bytes: &[u8]) -> Result<(Self, u8)> {
        if bytes.is_empty() {
            return Err(Error::PrematureEndOfInput);
        }
        let varu_decoded_len = Self::varu_decoded_len(bytes[0]);
        if varu_decoded_len <= 5 {
            Ok((varu32_decode(varu_decoded_len, bytes[0], &bytes[1..])?, varu_decoded_len))
        } else {
            Err(Error::InvalidVarintEncoding)
        }
    }
    #[inline]
    fn varu_to_slice(&self, bytes: &mut [u8]) -> u8 {
        let length = self.varu_encoded_len();
        // 5-byte special case, length byte is zero in this case
        if length == 5 {
            bytes[0] = 0xf0;
            bytes[1..].copy_from_slice(&self.to_le_bytes());
        } else {
            let encoded = (*self << 1 | 1) << (u32::from(length) - 1);
            bytes[..4].copy_from_slice(&encoded.to_le_bytes());
        }
        length
    }
}

// Decode variable length bytes into `u64`, when decoded length is known
// from previous call of `varu64_varu_decoded_len()`
#[inline]
fn varu64_decode(varu_encoded_length: u8, first_byte: u8, bytes: &[u8]) -> Result<u64> {
    if bytes.len() + 1 < varu_encoded_length as usize {
        return Err(Error::PrematureEndOfInput);
    }
    let mut encoded = [0_u8; 8];
    let result = if varu_encoded_length == 9 {
        // 9-byte special case
        encoded.copy_from_slice(&bytes[0..8]);
        u64::from_le_bytes(encoded)
    } else {
        encoded[0] = first_byte;
        let len = varu_encoded_length as usize;
        encoded[1..len as usize].copy_from_slice(&bytes[..len-1 as usize]);
        u64::from_le_bytes(encoded) >> varu_encoded_length
    };
    #[cfg(debug_assertions)]
    if !(varu_encoded_length == 1 || result >= (1 << (7 * (varu_encoded_length - 1)))) {
        return Err(Error::InvalidVarintEncoding);
    }
    Ok(result)
}

// Decode variable length bytes into `u32`, when decoded length is known
// from previous call of `varu_varu_decoded_len()`. `varu_encoded_length` must be less or equal to 5.
#[inline]
fn varu32_decode(varu_encoded_length: u8, first_byte: u8, bytes: &[u8]) -> Result<u32> {
    if bytes.len() + 1 < varu_encoded_length as usize {
        return Err(Error::PrematureEndOfInput);
    }
    let mut encoded = [0_u8; 4];
    let result = if varu_encoded_length == 5 {
        // 5-byte special case
        encoded.copy_from_slice(&bytes[0..4]);
        u32::from_le_bytes(encoded)
    } else {
        encoded[0] = first_byte;
        let len = varu_encoded_length as usize;
        encoded[1..len as usize].copy_from_slice(&bytes[..len-1 as usize]);
        u32::from_le_bytes(encoded) >> varu_encoded_length
    };
    #[cfg(debug_assertions)]
    if !(varu_encoded_length == 1 || result >= (1 << (7 * (varu_encoded_length - 1)))) {
        return Err(Error::InvalidVarintEncoding);
    }
    Ok(result)
}

// Note the 32 and 64 bit versions below are binary compatible: 64-bit version can read
// data written by 32-bit encoder, but not vice versa

/// Variable-length encoding for sequence lengths which writes to the end of the double-ended buffer
pub struct VarIntTailLenEncoder;

#[cfg(target_pointer_width = "64")]
impl LengthEncoder for VarIntTailLenEncoder {
    type Value = usize;

    #[inline]
    fn calc_size(value: Self::Value) -> usize {
        (value as u64).varu_encoded_len() as usize
    }
    #[inline]
    #[allow(clippy::cast_possible_truncation)] // can't happen because of cfg
    fn read(mut reader: impl TailReadBytes) -> Result<usize> {
        <u64>::varu_from_reader(ReadFromTail(&mut reader)).map(|v| v as usize)
    }
    #[inline]
    fn write(mut writer: impl TailWriteBytes, value: usize) -> Result {
        (value as u64).varu_to_writer(WriteToTail(&mut writer))
    }
}

#[cfg(target_pointer_width = "32")]
#[allow(clippy::cast_possible_truncation)] // can't happen because of cfg
impl LengthEncoder for VarIntTailLenEncoder {
    type Value = usize;

    #[inline]
    fn calc_size(value: Self::Value) -> usize {
        (value as u32).varu_encoded_len() as usize
    }
    #[inline]
    #[allow(clippy::cast_possible_truncation)] // can't happen because of cfg
    fn read(mut reader: impl TailReadBytes) -> Result<usize> {
        <u32>::varu_from_reader(ReadFromTail(&mut reader)).map(|v| v as usize)
    }
    #[inline]
    fn write(mut writer: impl TailWriteBytes, value: usize) -> Result {
        (value as u32).varu_to_writer(WriteToTail(&mut writer))
    }
}

/// Variable-length encoding for sequence lengths which writes
/// to the head of the double-ended buffer
pub struct VarIntLenEncoder;

#[cfg(target_pointer_width = "64")]
impl LengthEncoder for VarIntLenEncoder {
    type Value = usize;

    #[inline]
    fn calc_size(value: Self::Value) -> usize {
        (value as u64).varu_encoded_len() as usize
    }
    #[inline]
    #[allow(clippy::cast_possible_truncation)] // can't happen because of cfg
    fn read(mut reader: impl TailReadBytes) -> Result<usize> {
        <u64>::varu_from_reader(&mut reader).map(|v| v as usize)
    }
    #[inline]
    fn write(mut writer: impl TailWriteBytes, value: usize) -> Result {
        (value as u64).varu_to_writer(&mut writer)
    }
}

#[cfg(target_pointer_width = "32")]
#[allow(clippy::cast_possible_truncation)] // can't happen because of cfg
impl LengthEncoder for VarIntLenEncoder {
    type Value = usize;

    #[inline]
    fn calc_size(value: Self::Value) -> usize {
        (value as u32).varu_encoded_len() as usize
    }
    #[inline]
    #[allow(clippy::cast_possible_truncation)] // can't happen because of cfg
    fn read(mut reader: impl TailReadBytes) -> Result<usize> {
        <u32>::varu_from_reader(&mut reader).map(|v| v as usize)
    }
    #[inline]
    fn write(mut writer: impl TailWriteBytes, value: usize) -> Result {
        (value as u32).varu_to_writer(&mut writer)
    }
}

/// Variable-length encoding for enum discriminants
pub struct VarIntDiscrEncoder;

impl LengthEncoder for VarIntDiscrEncoder {
    type Value = u32;

    #[inline]
    fn calc_size(value: Self::Value) -> usize {
        value.varu_encoded_len() as usize
    }
    #[inline]
    fn read(reader: impl TailReadBytes) -> Result<Self::Value> {
        <u32>::varu_from_reader(reader)
    }
    #[inline]
    fn write(writer: impl TailWriteBytes, value: Self::Value) -> Result {
        value.varu_to_writer(writer)
    }
}