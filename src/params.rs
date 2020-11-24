//! Serialization parameters traits and types
//!
#![allow(clippy::module_name_repetitions)]

use crate::{varint, Result, buf::{TailReadBytes, TailWriteBytes}};

/// Lexicographical ordering for serialization
///
/// Note that there are no ordering marks in the serialized data; specification of different ordering
/// for serialization and deserialization of the same data is UB.
#[derive(Copy, Clone)]
pub enum Order {
    Ascending,
    Descending,
    /// For use by other crates. For the purposes of `ordcode`, same as [`Ascending`](Order::Ascending).
    Unordered
}

/// Endianness representation for serialized integers.
#[derive(Copy, Clone)]
pub enum Endianness {
    Little,
    Big,
    Native,
}

/// Encoding parameters for primitive types serialization: lexicographical order and endianness.
pub trait EncodingParams: Copy {
    /// Serialization ordering of primitive types
    ///
    /// Note that you should not specify [`Order::Descending`] when parameterizing [`Serializer`](crate::Serializer):
    /// descending ordering for composite types is achieved differently, by negating resulting
    /// byte buffer (this is also faster).
    const ORDER: Order;

    /// Endianness for encoding integer and float values; for encodings which preserve
    /// lexicographical ordering, should be [`Endianness::Big`]
    const ENDIANNESS: Endianness;
}

/// Parameters for implementations of `serde` serializer and deserializer
pub trait SerializerParams: EncodingParams {
    /// Encoder for sequence lengths
    type SeqLenEncoder: LengthEncoder<Value=usize>;

    /// Encoder for discriminant values
    type DiscriminantEncoder: LengthEncoder<Value=u32>;
}

/// Encoder for array lengths, enum discriminants etc.
pub trait LengthEncoder {
    /// Value type, may be `u32`, `u64` or usize
    type Value;

    /// Calculate serialized size for value
    fn calc_size(value: Self::Value) -> usize;
    fn read(reader: impl TailReadBytes) -> Result<Self::Value>;
    fn write(writer: impl TailWriteBytes, value: Self::Value) -> Result;
}

impl<T> EncodingParams for &T where T: EncodingParams {
    const ORDER: Order = T::ORDER;
    const ENDIANNESS: Endianness = T::ENDIANNESS;
}

impl <T> SerializerParams for &T where T: SerializerParams {
    type SeqLenEncoder = T::SeqLenEncoder;
    type DiscriminantEncoder = T::DiscriminantEncoder;
}

/// Serializer parameters for lexicographical order-preserving serialization in ascending order
#[derive(Copy, Clone, Default)]
pub struct AscendingOrder;

impl EncodingParams for AscendingOrder {
    const ORDER: Order = Order::Ascending;
    const ENDIANNESS: Endianness = Endianness::Big;
}

impl SerializerParams for AscendingOrder {
    type SeqLenEncoder = varint::VarIntTailLenEncoder;
    type DiscriminantEncoder = varint::VarIntDiscrEncoder;
}

/// Encoding paramerers for lexicographical order-preserving serialization in descending order
///
/// Note: deliberately implements only [`EncodingParams`] trait, not [`SerializerParams`], so it can
/// be used with serialization primitives, but not with [`Serializer`](crate::Serializer)
/// or [`Deserializer`](crate::Deserializer).
#[derive(Copy, Clone, Default)]
pub struct DescendingOrder;

impl EncodingParams for DescendingOrder {
    const ORDER: Order = Order::Descending;
    const ENDIANNESS: Endianness = Endianness::Big;
}

/// Serializer parameters for portable binary format, which does not need double-ended buffer.
/// However, it still requires implementation of [`TailReadBytes`](crate::buf::TailReadBytes),
/// [`TailWriteBytes`](crate::buf::TailWriteBytes) traits
/// for reader and writer, which should behave same as [`ReadBytes`](crate::buf::ReadBytes),
/// [`WriteBytes`](crate::buf::WriteBytes).
#[derive(Copy, Clone, Default)]
pub struct PortableBinary;

impl EncodingParams for PortableBinary {
    const ORDER: Order = Order::Ascending;
    const ENDIANNESS: Endianness = Endianness::Big;
}

impl SerializerParams for PortableBinary {
    type SeqLenEncoder = varint::VarIntLenEncoder;
    type DiscriminantEncoder = varint::VarIntDiscrEncoder;
}

/// Serializer parameters for platform-specific binary format, which does not need double-ended buffer.
/// This is probably the fastest option, but serialized data will not be portable.
///
/// It still requires implementation of [`TailReadBytes`](crate::buf::TailReadBytes),
/// [`TailWriteBytes`](crate::buf::TailWriteBytes) traits for reader
/// and writer, which should behave same as [`ReadBytes`](crate::buf::ReadBytes),
/// [`WriteBytes`](crate::buf::WriteBytes).
#[derive(Copy, Clone, Default)]
pub struct NativeBinary;

impl EncodingParams for NativeBinary {
    const ORDER: Order = Order::Unordered;
    const ENDIANNESS: Endianness = Endianness::Native;
}

impl SerializerParams for NativeBinary {
    type SeqLenEncoder = varint::VarIntLenEncoder;
    type DiscriminantEncoder = varint::VarIntDiscrEncoder;
}