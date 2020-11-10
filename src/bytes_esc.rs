//! Byte sequences encoding and decoding with escaping
//!
//! Byte value `0xF8` is escaped as `{ 0xF8, 0xFF }` for ascending order,
//! `{ 0x07, 0x00 }` for descending order. Sequence is terminated by
//! `{ 0xF8, 0x01 }` for ascending order, `{ 0x07, 0xFE }` for descending order. Escaped byte
//!   value `0xF8` is chosen because it does not appear in valid UTF-8, and escaping zero
//!   is impractical (it is too common)
use crate::{ ReadBytes, WriteBytes, Result, ResultExt, ErrorKind,
             Order, BytesBuf, BytesBufExt, ord_cond };

fn apply_over_esc<R, F>(rb: &mut R, esc: u8, advance: bool, f: &mut F) -> Result
    where F: FnMut(&[u8], u8) -> Result<bool>,
    R: ReadBytes,
{
    let mut b = &rb.remaining_buffer()[..];
    let r = loop {
        if let Some(pos) = b.iter().position(|v| *v == esc) {
            if pos + 1 >= b.len() {
                break err!(PrematureEndOfInput)
            }
            if !f(&b[..=pos], b[pos+1])? {
                b = &b[pos+2..];
                break Ok(())
            }
            b = &b[pos+2..];
        } else {
            break err!(PrematureEndOfInput)
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

fn read_escaped_bytes_asc(rb: &mut impl ReadBytes, out: &mut impl WriteBytes) -> Result
{
    apply_over_esc(rb,BSTR_ESCAPE_ASC.start, true, &mut |buf, c| {
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
    apply_over_esc(rb,BSTR_ESCAPE_DESC.start, true, &mut |buf, c| {
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
        writer.write(&[!*v])?;
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
    let b = reader.remaining_buffer();
    let r = ord_cond!(order, write_complement_bytes(writer, b), writer.write(b))?;
    let len = b.len();
    reader.advance(len);
    Ok(r)
}

/// Deserialize input buffer as ordered byte string to `VecBuf`, no escaping and termination sequences
pub fn deserialize_bytes_noesc(reader: &mut impl ReadBytes, order: Order) -> Result<BytesBuf>
{
    let v = reader.remaining_buffer();
    let mut res = BytesBuf::with_reserve(v.len());
    ord_cond!(order, { write_complement_bytes(&mut res, v)?; }, { res.extend_from_slice(v); });
    let len = v.len();
    reader.advance(len);
    Ok(res)
}

/// Deserialize input buffer as ordered bytes to `String`, no escaping and termination sequences
pub fn deserialize_bytes_noesc_to_string(reader: &mut impl ReadBytes, order: Order) -> Result<String>
{
    let bstr = deserialize_bytes_noesc(reader, order)?;
    let s = String::from_utf8(bstr.into_vec8()).chain_err(|| ErrorKind::InvalidUtf8Encoding)?;
    Ok(s)
}
