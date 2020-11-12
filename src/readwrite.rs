use crate::Result;

/// Simple byte reader from buffer
///
/// If you need to read from `&[u8]`, you may use `BytesReader` provided by this crate.
pub trait ReadBytes {
    /// Peek `n` bytes from head
    fn peek(&mut self, n: usize) -> Result<&'_[u8]>;

    /// Advance buffer head by `n` bytes. `n` should be smaller than remaining buffer size.
    fn advance(&mut self, n: usize);

    /// Get `n` bytes from the beginning of buffer, advance by `n` bytes
    fn read<F, R>(&mut self, n: usize, f: F) -> Result<R> where F: Fn(&[u8]) -> Result<R> {
        let r = f(self.peek(n)?)?;
        self.advance(n);
        Ok(r)
    }
    /// Returns view into remaining buffer
    fn remaining_buffer(&mut self) -> &'_[u8];

    /// Check if buffer is fully consumed (empty)
    fn is_empty(&mut self) -> bool { self.remaining_buffer().is_empty() }
}

// forwarding for being able to use `&mut ReadBytes` in place of `ReadBytes`
impl<'a, T> ReadBytes for &'a mut T where T: ReadBytes  {
    fn peek(&mut self, n: usize) -> Result<&'_[u8]> {
        (*self).peek(n)
    }
    fn advance(&mut self, n: usize) {
        (*self).advance(n)
    }
    fn remaining_buffer(&mut self) -> &'_[u8] { (*self).remaining_buffer() }
}

/// Implementation of `BiReadBytes` from byte slice
pub struct BytesReader<'a> {
    buf: &'a [u8],
}

impl<'a> BytesReader<'a> {
    /// Constructs reader from provided byte slice
    #[must_use] pub fn new(buf: &'a [u8]) -> Self { Self { buf } }

    fn peek_head(&mut self, n: usize) -> Result<&'_[u8]> {
        if n <= self.buf.len() {
            Ok(&self.buf[..n])
        } else {
            err!(PrematureEndOfInput)
        }
    }
    fn advance_head(&mut self, n: usize) {
        self.buf = &self.buf[n..];
    }
    fn peek_tail(&mut self, n: usize) -> Result<&'_[u8]> {
        if n <= self.buf.len() {
            Ok(&self.buf[(self.buf.len() - n)..])
        } else {
            err!(PrematureEndOfInput)
        }
    }
    fn advance_tail(&mut self, n: usize) {
        self.buf = &self.buf[..self.buf.len() - n];
    }
}

impl <'a> ReadBytes for BytesReader<'a> {
    fn peek(&mut self, n: usize) -> Result<&'_[u8]> {
        self.peek_head(n)
    }
    fn advance(&mut self, n: usize) {
        self.advance_head(n)
    }
    fn remaining_buffer(&mut self) -> &'_[u8] { self.buf }
}

/// Adapter for `BytesReader` for reading from tail if the buffer
pub struct ReadFromTail<'a, 'b>(pub &'a mut BytesReader<'b>);

impl <'a, 'b> ReadBytes for ReadFromTail<'a, 'b> {
    fn peek(&mut self, n: usize) -> Result<&'_[u8]> {
        self.0.peek_tail(n)
    }
    fn advance(&mut self, n: usize) {
        self.0.advance_tail(n)
    }
    fn remaining_buffer(&mut self) -> &'_[u8] { self.0.remaining_buffer() }
}

#[cfg(feature="std")]
impl std::io::Read for BytesReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.buf.read(buf)
    }
}

/// Trait for writer to the byte buffer
pub trait WriteBytes {
    /// Write to the byte buffer
    fn write(&mut self, value: &[u8]) -> Result;
}

/// Bipartite byte buffer
pub struct BiBuffer<'a> {
    buf: &'a mut [u8],
    head: usize,
    tail: usize,
}

impl<'a> BiBuffer<'a> {
    /// Use provided byte slice as buffer
    pub fn new(buf: &'a mut [u8]) -> Self {
        let tail = buf.len();
        Self { buf, head: 0, tail }
    }
    /// Finalize by collapsing extra space in internal buffer, returns data length
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
    /// Checks if buffer completely filled (collapsed)
    #[must_use]
    pub fn is_complete(&self) -> bool { self.head == self.tail }

    fn write_head(&mut self, value: &[u8]) -> Result {
        if (self.tail - self.head) < value.len() {
            err!(BufferOverflow)
        } else {
            self.buf[self.head..(self.head + value.len())].copy_from_slice(value);
            self.head += value.len();
            Ok(())
        }
    }
    fn write_tail(&mut self, value: &[u8]) -> Result {
        if (self.tail - self.head) < value.len() {
            err!(BufferOverflow)
        } else {
            let end_offs = self.tail - value.len();
            self.buf[end_offs..].copy_from_slice(value);
            self.tail -= value.len();
            Ok(())
        }
    }
}

impl<'a> WriteBytes for BiBuffer<'a> {
    fn write(&mut self, value: &[u8]) -> Result {
        self.write_head(value)
    }
}

/// Adapter for writing to the tail of the buffer
pub struct WriteToTail<'a, 'b>(pub &'a mut BiBuffer<'b>);

impl<'a, 'b> WriteBytes for WriteToTail<'a, 'b> {
    fn write(&mut self, value: &[u8]) -> Result {
        self.0.write_tail(value)
    }
}

// forwarding for being able to use `&mut WriteBytes` in place of `WriteBytes`
impl<T> WriteBytes for &mut T where T: WriteBytes {
    fn write(&mut self, buf: &[u8]) -> Result { (*self).write(buf) }
}
