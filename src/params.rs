use crate::{ Result, ReadBytes, WriteBytes, varint };

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

    /// True if sequence lengths and other meta-data be put to the end of the buffer, to
    /// preserve lexicographical order. In this mode, buffer size for serialization should
    /// be big enough to fit all serialized data, or serialization will fail.
    const USE_TAIL: bool;

    /// Encoder for sequence lengths
    type SeqLenEncoder: LenEncoder;

    /// Encoder for discriminant values
    type DiscriminantEncoder: LenEncoder;

    // this is to allow access to these associated constants from ZST object
    #[doc(hidden)]
    #[inline]
    fn order(&self) -> Order { Self::ORDER }

    #[doc(hidden)]
    #[inline]
    fn endianness(&self) -> Endianness { Self::ENDIANNESS }
}

impl<T> EncodingParams for &T where T: EncodingParams {
    const ORDER: Order = T::ORDER;
    const ENDIANNESS: Endianness = T::ENDIANNESS;
    const USE_TAIL: bool = T::USE_TAIL;
    type SeqLenEncoder = T::SeqLenEncoder;
    type DiscriminantEncoder = T::DiscriminantEncoder;
}

/// Encoder for array lengths, enum discriminants etc.
pub trait LenEncoder {
    /// Calculate serialized size for value
    fn calc_size(value: usize) -> usize;
    fn read(reader: impl ReadBytes, params: impl EncodingParams) -> Result<usize>;
    fn write(writer: impl WriteBytes, params: impl EncodingParams, value: usize) -> Result;
}

/// Variable-length encoding for array lengths, enum discriminants etc.
pub struct VarIntLenEncoder;

impl LenEncoder for VarIntLenEncoder {
    #[inline]
    fn calc_size(value: usize) -> usize {
        varint::varu64_encoded_len(value as u64) as usize
    }
    #[inline]
    fn read(reader: impl ReadBytes, _params: impl EncodingParams) -> Result<usize> {
        varint::varu64_decode_from_reader(reader).map(|v| v as usize)
    }
    #[inline]
    fn write(writer: impl WriteBytes, _params: impl EncodingParams, value: usize) -> Result {
        varint::varu64_encode_to_writer(writer, value as u64)
    }
}

/// Parameters for lexicographical order-preserving serialization in ascending order
#[derive(Copy, Clone)]
pub struct AscendingOrder;

/// Parameters for lexicographical order-preserving serialization in descending order
#[derive(Copy, Clone)]
pub struct DescendingOrder;

impl EncodingParams for AscendingOrder {
    const ORDER: Order = Order::Ascending;
    const ENDIANNESS: Endianness = Endianness::Big;
    const USE_TAIL: bool = true;
    type SeqLenEncoder = VarIntLenEncoder;
    type DiscriminantEncoder = VarIntLenEncoder;
}

impl EncodingParams for DescendingOrder {
    const ORDER: Order = Order::Descending;
    const ENDIANNESS: Endianness = Endianness::Big;
    const USE_TAIL: bool = true;
    type SeqLenEncoder = VarIntLenEncoder;
    type DiscriminantEncoder = VarIntLenEncoder;
}