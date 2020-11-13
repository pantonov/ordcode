//! Pre-set serialization configurations

#![allow(clippy::module_name_repetitions)]

use crate::{ varint, Result, TailReadBytes, TailWriteBytes };

/// Specifies lexicographical ordering for serialization. There are no ordering marks in the
/// serialized data; specification of different ordering for serialization and deserialization
/// of the same data is UB.
#[derive(Copy, Clone)]
pub enum Order {
    Ascending,
    Descending,
    /// For use by other crates. For the purposes of `ordcode`, same as `Ascending`.
    Unordered
}

/// Endianness representation for serialized integers
#[derive(Copy, Clone)]
pub enum Endianness {
    Little,
    Big,
    Native,
}

/// Trait which collects encoding params for serializers: lexicographical order,
/// endianness for integer values, encoding of sequence lengths and discriminants,
/// use of tail-buffer encoding
pub trait EncodingParams: Copy {
    /// Lexicographical ordering of values
    const ORDER: Order;

    /// Endianness for integer values; for encodings which preserve lexicographical order,
    /// should be `Endianness::Big`
    const ENDIANNESS: Endianness;
}

pub trait SerializerParams {
    /// True if sequence lengths and other meta-data be put to the end of the buffer, to
    /// preserve lexicographical order. In this mode, buffer size for serialization should
    /// be big enough to fit all serialized data, or serialization will fail.
    const USE_TAIL: bool;

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
    const USE_TAIL: bool = T::USE_TAIL;
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
    const USE_TAIL: bool = true;
    type SeqLenEncoder = varint::VarIntLenEncoder<Self>;
    type DiscriminantEncoder = varint::VarIntDiscrEncoder;
}

/// Lexicographical order-preserving serialization in descending order
#[derive(Copy, Clone, Default)]
pub struct DescendingOrder;

impl EncodingParams for DescendingOrder {
    const ORDER: Order = Order::Descending;
    const ENDIANNESS: Endianness = Endianness::Big;
}

impl SerializerParams for DescendingOrder {
    const USE_TAIL: bool = true;
    type SeqLenEncoder = varint::VarIntLenEncoder<Self>;
    type DiscriminantEncoder = varint::VarIntDiscrEncoder;
}
