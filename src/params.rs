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
    /// For use by other crates. For the purposes of `ordcode`, same as `Ascending`.
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
    /// Note that you should not specify `Order::Descending` when parameterizing `OrderedSerializer`:
    /// descending ordering for composite types is achieved differently, by negating resulting
    /// byte buffer (this is also faster).
    const ORDER: Order;

    /// Endianness for encoding integer and float values; for encodings which preserve
    /// lexicographical ordering, should be `Endianness::Big`
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
    /// Value type, may be u32, u64 or usize
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

/// Lexicographical order-preserving serialization in ascending order
#[derive(Copy, Clone, Default)]
pub struct AscendingOrder;

impl EncodingParams for AscendingOrder {
    const ORDER: Order = Order::Ascending;
    const ENDIANNESS: Endianness = Endianness::Big;
}

impl SerializerParams for AscendingOrder {
    type SeqLenEncoder = varint::VarIntLenEncoder;
    type DiscriminantEncoder = varint::VarIntDiscrEncoder;
}

/// Lexicographical order-preserving serialization in descending order
///
/// Note that only `EncodingParams` trait is implemented, not `SerializerParams`: this is deliberate.
#[derive(Copy, Clone, Default)]
pub struct DescendingOrder;

impl EncodingParams for DescendingOrder {
    const ORDER: Order = Order::Descending;
    const ENDIANNESS: Endianness = Endianness::Big;
}
