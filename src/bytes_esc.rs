//! Byte sequences encoding and decoding with escaping
//!
//! Byte value `0xF8` is escaped as `{ 0xF8, 0xFF }` for ascending order,
//! `{ 0x07, 0x00 }` for descending order. Sequence is terminated by
//! `{ 0xF8, 0x01 }` for ascending order, `{ 0x07, 0xFE }` for descending order. Escaped byte
//!   value `0xF8` is chosen because it does not appear in valid UTF-8, and escaping zero
//!   is impractical (it is too common)
use crate::{Error,ReadBytes, WriteBytes, Result, Order, EncodingParams};

#[cfg(features="std")]
use crate::BytesBufExt;

fn apply_over_esc<R, F>(rb: &mut R, esc: u8, advance: bool, f: &mut F) -> Result
    where F: FnMut(&[u8], u8) -> Result<bool>,
    R: ReadBytes,
{
    let mut b = &rb.remaining_buffer()[..];
    let r = loop {
        if let Some(pos) = b.iter().position(|v| *v == esc) {
            if pos + 1 >= b.len() {
                break Err(Error::PrematureEndOfInput)
            }
            if !f(&b[..=pos], b[pos+1])? {
                b = &b[pos+2..];
                break Ok(())
            }
            b = &b[pos+2..];
        } else {
            break Err(Error::PrematureEndOfInput)
        }
    };
    let len = b.len();
    if advance {
        rb.advance(len);
    }
    r
}


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
    apply_over_esc(rb, esc.start, false, &mut |buf, c| {
        if c == esc.esc {
            len += buf.len();
            Ok(true)
        } else if c == esc.term {
            len += buf.len() - 1;
            Ok(false)
        } else {
            Err(Error::InvalidByteSequenceEscape)
        }
    }).and(Ok(len))
}

/// Calculate length of pending byte sequence from reader
#[inline]
pub fn bytes_length<P: EncodingParams>(mut reader: impl ReadBytes, _param: P) -> Result<usize> {
    ord_cond!(P, unescaped_length(&mut reader, &BSTR_ESCAPE_DESC),
              unescaped_length(&mut reader, &BSTR_ESCAPE_ASC))
}

/// Serialize byte sequence to escaped representation
pub fn serialize_bytes<P: EncodingParams>(mut writer: impl WriteBytes, value: &[u8], _param: P) -> Result {
    ord_cond!(P, {
        for b in value {
            if BSTR_ESCAPE_ASC.start == *b {
                writer.write(&[BSTR_ESCAPE_DESC.start, BSTR_ESCAPE_DESC.esc])?;
            } else {
                writer.write(&[!*b])?;
            }
        }
        writer.write(&[BSTR_ESCAPE_DESC.start, BSTR_ESCAPE_DESC.term])
    }, {
        for b in value {
            if BSTR_ESCAPE_ASC.start == *b {
                writer.write(&[BSTR_ESCAPE_ASC.start, BSTR_ESCAPE_ASC.esc])?;
            } else {
                writer.write(&[*b])?;
            }
        }
        writer.write(&[BSTR_ESCAPE_ASC.start, BSTR_ESCAPE_ASC.term])
    })
}

fn read_escaped_bytes_asc(mut rb: impl ReadBytes, mut out: impl WriteBytes) -> Result
{
    apply_over_esc(&mut rb,BSTR_ESCAPE_ASC.start, true, &mut |buf, c| {
        if c == BSTR_ESCAPE_ASC.esc {
            out.write(&buf[..buf.len()])?;
            Ok(true)
        } else if c == BSTR_ESCAPE_ASC.term {
            out.write(&buf[..buf.len() - 1])?;
            Ok(false)
        } else {
            Err(Error::InvalidByteSequenceEscape)
        }
    })
}

fn read_escaped_bytes_desc(mut rb: impl ReadBytes, mut out: impl WriteBytes) -> Result
{
    apply_over_esc(&mut rb,BSTR_ESCAPE_DESC.start, true, &mut |buf, c| {
        if c == BSTR_ESCAPE_DESC.esc {
            write_complement_bytes(&mut out,&buf[..buf.len()])?;
            Ok(true)
        } else if c == BSTR_ESCAPE_DESC.term {
            write_complement_bytes(&mut out,&buf[..buf.len() - 1])?;
            Ok(false)
        } else {
            Err(Error::InvalidByteSequenceEscape)
        }
    })
}

/// Deserialize escaped byte sequence and write result to `WriteBytes`
#[inline]
pub fn deserialize_bytes_to_writer<P: EncodingParams>(reader: impl ReadBytes, out: impl WriteBytes, _param: P) -> Result
{
    ord_cond!(P, read_escaped_bytes_desc(reader, out),
              read_escaped_bytes_asc(reader, out))
}

/// Deserialize escaped byte sequence
#[cfg(feature="std")]
pub fn deserialize_bytes_to_vec<P: EncodingParams>(mut reader: impl ReadBytes, param: P) -> Result<Vec<u8>>
{
    let len = bytes_length(&mut reader, &param)?;
    let mut v = Vec::with_capacity(len);
    deserialize_bytes_to_writer(&mut reader, &mut v, &param)?;
    Ok(v)
}

/// Write 0xFF bitwise complement of input
#[inline]
pub fn write_complement_bytes(mut writer: impl WriteBytes, input: &[u8]) -> Result {
    for v in input {
        writer.write(&[!*v])?;
    }
    Ok(())
}

/// Serialize whole input buffer as ordered byte string, no escaping and termination sequences.
/// This method copies source byte buffer for `Ascending` order, or bitwise complements
/// if ordering is `Descending`.
pub fn serialize_bytes_noesc<P: EncodingParams>(mut writer: impl WriteBytes, v: &[u8], _param: P) -> Result
{
    ord_cond!(P, write_complement_bytes(writer, v), writer.write(v))
}

/// Deserialize input buffer as ordered byte string into writer, no escaping and termination sequences
pub fn deserialize_bytes_noesc_to_writer<P: EncodingParams>(mut reader: impl ReadBytes,
                                                            mut writer: impl WriteBytes,
                                                            _param: P) -> Result
{
    let b = reader.remaining_buffer();
    ord_cond!(P, write_complement_bytes(writer, b), writer.write(b))?;
    let len = b.len();
    reader.advance(len);
    Ok(())
}

/// Deserialize input buffer as ordered byte string, no escaping and termination sequences
#[cfg(feature="std")]
pub fn deserialize_bytes_noesc_to_vec<P: EncodingParams>(mut reader: impl ReadBytes, _param: P) -> Result<Vec<u8>>
{
    let v = reader.remaining_buffer();
    let mut res = Vec::with_capacity(v.len());
    ord_cond!(P, { write_complement_bytes(&mut res, v)?; }, { res.write(v)?; });
    let len = v.len();
    reader.advance(len);
    Ok(res)
}

/// Deserialize input buffer as ordered bytes to `String`, no escaping and termination sequences
#[cfg(feature="std")]
pub fn deserialize_bytes_noesc_to_string<P: EncodingParams>(reader: impl ReadBytes, param: P) -> Result<String>
{
    let bstr = deserialize_bytes_noesc_to_vec(reader, param)?;
    let s = String::from_utf8(bstr).map_err(|_| Error::InvalidUtf8Encoding)?;
    Ok(s)
}
