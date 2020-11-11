//!
//! Variable length serialization of integers
//!
use crate::{ ReadBytes, WriteBytes, Result };

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
#[must_use]
pub fn varu64_decoded_len(first_byte: u8) -> u8 {
    // truncation can't happen, max value is 8
    #![allow(clippy::cast_possible_truncation)]
    (first_byte.trailing_zeros() + 1) as u8
}

/// Encode `u64` as variable length bytes, write to tail of the buffer
pub fn varu64_encode_to_writer(mut writer: impl WriteBytes, value: u64) -> Result {
    let mut bytes = [0_u8; 9];
    let length = varu64_encode_to_slice(&mut bytes, value);
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

/// Decode variable length bytes into `u64`, when decoded length is known
/// from previous call of `varu64_decoded_len()`
pub fn varu64_decode(encoded_length: u8, first_byte: u8, bytes: &[u8]) -> Result<u64> {
    if bytes.len() + 1 < encoded_length as usize {
        return err!(PrematureEndOfInput);
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
        return err!(InvalidVarintEncoding);
    }
    Ok(result)
}

/// Decode variable length bytes into `u64`, returns value and encoded length
pub fn varu64_decode_from_slice(bytes: &[u8]) -> Result<(u64, u8)> {
    if bytes.is_empty() {
        return err!(PrematureEndOfInput);
    }
    let decoded_len = varu64_decoded_len(bytes[0]);
    Ok((varu64_decode(decoded_len, bytes[0], &bytes[1..])?, decoded_len))
}

/// Decode variable length bytes into `u64`
pub fn varu64_decode_from_reader(mut reader: impl ReadBytes) -> Result<u64> {
    let (first_byte, decoded_len) = reader.read(1, |buf| {
        Ok((buf[0], varu64_decoded_len(buf[0])))
    })?;
    reader.read(decoded_len as usize, |buf| {
        varu64_decode(decoded_len, first_byte,buf)
    })
}