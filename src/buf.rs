//! Types and traits for working with serialization and deserialization buffers
//!
use crate::{Result, Error};

/// Simple byte reader from buffer
///
/// If you need to read from `&[u8]`, you may use `BytesReader` provided by this crate.
pub trait ReadBytes {
    /// Peek `n` bytes from head
    fn peek<F, R>(&mut self, n: usize, f: F) -> Result<R> where F: FnOnce(&[u8]) -> Result<R>;

    /// Advance buffer head by `n` bytes. `n` should be smaller than remaining buffer size.
    fn advance(&mut self, n: usize);

    /// Get `n` bytes from the beginning of buffer, advance by `n` bytes
    fn read<F, R>(&mut self, n: usize, f: F) -> Result<R> where F: FnOnce(&[u8]) -> Result<R> {
        let r = self.peek(n, f)?;
        self.advance(n);
        Ok(r)
    }
    /// Returns view into remaining buffer
    fn remaining_buffer(&mut self) -> &'_[u8];

    /// Check if buffer is fully consumed (empty)
    fn is_complete(&mut self) -> Result {
        if self.remaining_buffer().is_empty() {
            Ok(())
        } else {
            Err(Error::BufferUnderflow)
        }
    }
}

pub trait TailReadBytes: ReadBytes {
    fn peek_tail<F, R>(&mut self, n: usize, f: F) -> Result<R> where F: FnOnce(&[u8]) -> Result<R>;

    /// Advance buffer head by `n` bytes. `n` should be smaller than remaining buffer size.
    fn advance_tail(&mut self, n: usize);

    /// Get `n` bytes from the beginning of buffer, advance by `n` bytes
    fn read_tail<F, R>(&mut self, n: usize, f: F) -> Result<R> where F: FnOnce(&[u8]) -> Result<R> {
        let r = self.peek_tail(n, f)?;
        self.advance_tail(n);
        Ok(r)
    }
}

// forwarding for being able to use `&mut ReadBytes` in place of `ReadBytes`
impl<'a, T> ReadBytes for &'a mut T where T: ReadBytes  {
    fn peek<F, R>(&mut self, n: usize, f: F) -> Result<R> where F: FnOnce(&[u8]) -> Result<R> {
        (*self).peek(n, f)
    }
    fn advance(&mut self, n: usize) {
        (*self).advance(n)
    }
    fn remaining_buffer(&mut self) -> &'_[u8] { (*self).remaining_buffer() }
}

// forwarding for being able to use `&mut ReadBytes` in place of `ReadBytes`
impl<'a, T> TailReadBytes for &'a mut T where T: TailReadBytes  {
    fn peek_tail<F, R>(&mut self, n: usize, f: F) -> Result<R> where F: FnOnce(&[u8]) -> Result<R> {
        (*self).peek_tail(n, f)
    }
    fn advance_tail(&mut self, n: usize) {
        (*self).advance_tail(n)
    }
}

/// Adapter type which implements double-ended read buffer over byte slice
///
/// Implements `ReadBytes`, `TailReadBytes` traits and intended to be used as input to `Deserializer`.
pub struct DeBytesReader<'a> {
    buf: &'a [u8],
}

impl<'a> DeBytesReader<'a> {
    /// Constructs reader from provided byte slice
    #[must_use] pub fn new(buf: &'a [u8]) -> Self { Self { buf } }
}

impl <'a> ReadBytes for DeBytesReader<'a> {
    fn peek<F, R>(&mut self, n: usize, f: F) -> Result<R>
        where F: FnOnce(&[u8]) -> Result<R>,
    {
        if n <= self.buf.len() {
            f(&self.buf[..n])
        } else {
            Err(Error::PrematureEndOfInput)
        }
    }
    fn advance(&mut self, n: usize) {
        self.buf = &self.buf[n..];
        //println!("after advance {} len={}", n, self.buf.len());

    }
    fn remaining_buffer(&mut self) -> &'_[u8] { self.buf }
}

impl<'a> TailReadBytes for DeBytesReader<'a> {
    fn peek_tail<F, R>(&mut self, n: usize, f: F) -> Result<R>
        where F: FnOnce(&[u8]) -> Result<R>,
    {
        if n <= self.buf.len() {
            f(&self.buf[(self.buf.len() - n)..])
        } else {
            Err(Error::PrematureEndOfInput)
        }
    }
    fn advance_tail(&mut self, n: usize) {
        self.buf = &self.buf[..self.buf.len() - n];
        //println!("after advance_tail {} len={}", n, self.buf.len());
    }
}

/// Adapter which implements `ReadBytes` for reading from the end of the buffer.
/// ```
/// # use biord::{ DeBytesReader, ReadFromTail, params, primitives::deserialize_u16 };
/// let buf = vec![11, 22, 33, 44, 55, 0, 1];
/// let mut reader = DeBytesReader::new(&buf);
/// assert_eq!(deserialize_u16(ReadFromTail(&mut reader), params::AscendingOrder).unwrap(), 1);
/// ```
/// (which in turn should implement `TailReadBytes` trait (such as `BytesReader`).
pub struct ReadFromTail<'a, R>(pub &'a mut R) where R: TailReadBytes;

impl <'a, R> ReadBytes for ReadFromTail<'a, R>
    where R: TailReadBytes,
{
    fn peek<F, RV>(&mut self, n: usize, f: F) -> Result<RV>
        where F: FnOnce(&[u8]) -> Result<RV>,
    {
        self.0.peek_tail(n, f)
    }
    fn advance(&mut self, n: usize) {
        self.0.advance_tail(n)
    }
    fn remaining_buffer(&mut self) -> &'_[u8] { self.0.remaining_buffer() }
}

#[cfg(feature="std")]
impl std::io::Read for DeBytesReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.buf.read(buf)
    }
}

/// Trait for writer to the byte buffer
pub trait WriteBytes {
    /// Write to the byte buffer
    fn write(&mut self, value: &[u8]) -> Result;
}

/// Trait for writer to the tail of byte buffer
pub trait TailWriteBytes: WriteBytes {
    /// Write to the tail of byte buffer
    fn write_tail(&mut self, value: &[u8]) -> Result;
}

/// Adapter type which implements double-ended write byte buffer over mutable byte slice
///
/// `DeBytesWriter` implements `WriteBytes` and `TailWriteBytes`, and can be used with `Serializer`.
pub struct DeBytesWriter<'a> {
    buf: &'a mut [u8],
    head: usize,
    tail: usize,
}

impl<'a> DeBytesWriter<'a> {
    /// Use provided byte slice as buffer
    pub fn new(buf: &'a mut [u8]) -> Self {
        let tail = buf.len();
        Self { buf, head: 0, tail }
    }
    /// Finalize by collapsing extra space in internal buffer
    ///
    /// Returns data length, which is smaller or equal to the original buffer size.
    pub fn finalize(&mut self) -> Result<usize> {
        if self.head == self.tail {
            Ok(self.buf.len())
        } else {
            self.buf.copy_within(self.tail.., self.head);
            let len = self.buf.len() - (self.tail - self.head);
            self.head = self.tail;
            Ok(len)
        }
    }
    /// Checks if buffer completely filled
    #[must_use]
    pub fn is_complete(&self) -> Result {
        if self.head == self.tail {
            Ok(())
        } else {
            Err(Error::BufferUnderflow)
        }
    }
}

impl<'a> WriteBytes for DeBytesWriter<'a> {
    fn write(&mut self, value: &[u8]) -> Result {
        if (self.head + value.len()) > self.tail {
            Err(Error::BufferOverflow)
        } else {
            self.buf[self.head..(self.head + value.len())].copy_from_slice(value);
            self.head += value.len();
            Ok(())
        }
    }
}

impl<'a> TailWriteBytes for DeBytesWriter<'a> {
    fn write_tail(&mut self, value: &[u8]) -> Result {
        if (self.head + value.len()) > self.tail {
            Err(Error::BufferOverflow)
        } else {
            let end_offs = self.tail - value.len();
            self.buf[end_offs..self.tail].copy_from_slice(value);
            self.tail = end_offs;
            Ok(())
        }
    }
}

/// Adapter which implements `WriteBytes` for writing to the end of double-ended
/// write buffer which implements `TailWriteBytes` trait (such as `DeBytesWriter`).
/// ```
/// # use biord::{ DeBytesWriter, WriteToTail, params, primitives::serialize_u16 };
/// let mut buf = vec![0_u8; 100];
/// let mut writer = DeBytesWriter::new(&mut buf);
/// serialize_u16(WriteToTail(&mut writer), 1, params::AscendingOrder).unwrap();
/// assert_eq!(&buf[98..100], &[0, 1]);
/// ```
pub struct WriteToTail<'a, W>(pub &'a mut W) where W: TailWriteBytes;

impl<'a, W> WriteBytes for WriteToTail<'a, W>
    where W: TailWriteBytes
{
    fn write(&mut self, value: &[u8]) -> Result {
        self.0.write_tail(value)
    }
}

// forwarding for being able to use `&mut WriteBytes` in place of `WriteBytes`
impl<T> WriteBytes for &mut T where T: WriteBytes {
    fn write(&mut self, buf: &[u8]) -> Result { (*self).write(buf) }
}

impl<T> TailWriteBytes for &mut T where T: TailWriteBytes {
    fn write_tail(&mut self, buf: &[u8]) -> Result { (*self).write_tail(buf) }
}

/// Pushes data to the vector
#[cfg(feature="std")]
impl WriteBytes for Vec<u8> {
    fn write(&mut self, buf: &[u8]) -> Result {
        self.extend_from_slice(buf);
        Ok(())
    }
}

/// Pushes data to the vector, same as `write()`
///
/// This means that `Serializer` can write to `Vec<u8>` buffer directly and grow it as needed,
/// however in this case lexicographical ordering property will not be preserved.
#[cfg(feature="std")]
impl TailWriteBytes for Vec<u8> {
    fn write_tail(&mut self, buf: &[u8]) -> Result {
        self.extend_from_slice(buf);
        Ok(())
    }
}

#[cfg(feature="std")]
#[test]
fn test_debuffer() {
    let mut byte_buf = vec![0_u8; 7];
    let mut bib = DeBytesWriter::new(byte_buf.as_mut_slice());
    bib.write(b"aa").unwrap();
    bib.write_tail(b"1").unwrap();
    bib.write(b"bb").unwrap();
    bib.write_tail(b"2").unwrap();
    bib.write(b"d").unwrap();
    bib.is_complete().unwrap();
    assert_eq!(&byte_buf, b"aabbd21");
    let mut rb = DeBytesReader::new(byte_buf.as_slice());
    assert_eq!(rb.read(3, |b| Ok(b == b"aab")).unwrap(), true);
    assert_eq!(rb.read_tail(1, |b| Ok(b == b"1")).unwrap(), true);
    assert_eq!(rb.read_tail(1, |b| Ok(b == b"2")).unwrap(), true);
    assert_eq!(rb.read(2, |b| Ok(b == b"bd")).unwrap(), true);
    rb.is_complete().unwrap();
}