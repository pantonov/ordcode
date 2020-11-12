#![cfg(feature="std")]
#![allow(dead_code)]

use crate::{Result, WriteBytes};
#[cfg(feature="smallvec")] extern crate smallvec;

#[cfg(not(any(feature="smallvec", feature="sled")))]
/// Byte buffer type. May be `Vec<u8>` or `smallvec::SmallVec<[u8;36]>` depending
/// on `smallvec` feature setting.
pub type BytesBuf = Vec<u8>;

#[cfg(feature="smallvec")]
/// Byte buffer type. May be `Vec<u8>` or `smallvec::SmallVec<[u8;36]>` depending
/// on `smallvec` feature setting.
pub type BytesBuf = smallvec::SmallVec<[u8; 36]>;

// experimental, do not use yet
#[cfg(feature="sled")]
pub type BytesBuf = sled::IVec;

/// Helper trait for internally used methods of `BytesBuf`, which may be implemented differently for
/// externally provided `BytesBuf` types
pub trait BytesBufExt: WriteBytes {
    /// Reserve `BytesBuf` with capacity
    fn with_reserve(n: usize) -> Self;

    /// Convert `BytesBuf` into `Vec<u8>`
    fn into_vec8(self) -> Vec<u8>;
}

impl WriteBytes for Vec<u8> {
    fn write(&mut self, buf: &[u8]) -> Result {
        self.extend_from_slice(buf);
        Ok(())
    }
}

#[cfg(not(any(feature="smallvec", feature="sled")))]
impl BytesBufExt for Vec<u8> {
    fn with_reserve(n: usize) -> Self { Self::with_capacity(n) }
    fn into_vec8(self) -> Vec<u8> { self }
}

#[cfg(feature="smallvec")]
const _: () = {
    impl BytesBufExt for BytesBuf {
        fn with_reserve(n: usize) -> Self { Self::new() }
        fn into_vec8(self) -> Vec<u8> { self.into_vec() }
    }
    impl WriteBytes for BytesBuf {
        fn write(&mut self, buf: &[u8]) -> Result {
            self.extend_from_slice(buf);
            Ok(())
        }
    }
};

#[cfg(feature="sled")]
const _: () = {
    impl BytesBufExt for BytesBuf {
        fn with_reserve(n: usize) -> BytesBuf { Self::with_capacity(n) }
        fn into_vec8(self) -> Vec<u8> { self.into() }
    }
    impl WriteBytes for BytesBuf {
        fn write(&mut self, buf: &[u8]) -> Result {
            self.extend_from_slice(buf);
            Ok(())
        }
    }
};