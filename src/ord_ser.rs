use crate::{Error, FormatVersion, buf::TailWriteBytes, Result,
            params::{SerializerParams, LengthEncoder }};
use crate::params::{AscendingOrder, PortableBinary, NativeBinary};
use crate::primitives::SerializableValue;
use serde::{ser, Serialize};

/// `serde` serializer for binary data format which may preserve lexicographical ordering of values
///
/// The data format is customizable: you can choose lexicographical ordering for encoding
/// of primitive types, endianness, encoding of lengths and enum discriminants; please see
/// `SerializerParams` trait. This crate provides `params::AscendingOrder`, which is a default
/// parameter set for `Serializer` which has the property to preserve lexicographical ordering
/// of serialized values. To obtain the descending lexicographical ordering, resulting byte buffer
/// should be bitwise inverted, e.g. with `primitives::invert_buffer())`.
///
/// Serializer requires access to double-ended data buffer, which should implement
/// `WriteBytes` and `TailWriteBytes` traits. This crate provides `DeWriteBuffer` type, which
/// is a wrapper around user-provided mutable slice to be used as a write buffer.
///
/// Serializer does not allocate anything: double-ended buffer should be big enough to contain
/// serialized data. To know required buffer size in advance, please use `calc_size` with same
/// `SerializerParams`. Size calculation is cheap, for fixed-size structures it folds into
/// compile-time constant.
pub struct Serializer<W, P> {
    writer: W,
    params: P,
}

impl<W, P> Serializer<W, P>
    where W: TailWriteBytes,
          P: SerializerParams,
{
    pub fn new(writer: W, params: P) -> Self {
        Self { writer, params }
    }
    pub fn into_writer(self) -> W { self.writer }

    #[inline]
    fn write_len(&mut self, v: usize) -> Result {
        P::SeqLenEncoder::write(&mut self.writer, v)
    }
    fn write_discr(&mut self, v: u32) -> Result {
        P::DiscriminantEncoder::write(&mut self.writer, v)
    }
}

impl<W> FormatVersion<AscendingOrder> for Serializer<W, AscendingOrder>  {
    const VERSION: u32 = 1;
}

impl<W> FormatVersion<PortableBinary> for Serializer<W, PortableBinary>  {
    const VERSION: u32 = 1;
}

impl<W> FormatVersion<NativeBinary> for Serializer<W, NativeBinary>  {
    const VERSION: u32 = 1;
}

macro_rules! serialize_fn {
    ($fn:ident, $t:ty) => {
        fn $fn(self, v: $t) -> Result {
            v.to_writer(&mut self.writer, self.params)
        }
    }
}

impl<'a, W, P> ser::Serializer for &'a mut Serializer<W, P>
    where W: TailWriteBytes,
          P: SerializerParams,
{
    type Ok = ();
    type Error = Error;

    type SerializeSeq = SerializeCompoundSeq<'a, W, P>;
    type SerializeTuple = SerializeCompound<'a, W, P>;
    type SerializeTupleStruct = SerializeCompound<'a, W, P>;
    type SerializeTupleVariant = SerializeCompound<'a, W, P>;
    type SerializeMap = SerializeCompoundSeq<'a, W, P>;
    type SerializeStruct = SerializeCompound<'a, W, P>;
    type SerializeStructVariant = SerializeCompound<'a, W, P>;

    serialize_fn!(serialize_bool, bool);
    serialize_fn!(serialize_u8,   u8);
    serialize_fn!(serialize_u16,  u16);
    serialize_fn!(serialize_u32,  u32);
    serialize_fn!(serialize_u64,  u64);
    serialize_fn!(serialize_i8,   i8);
    serialize_fn!(serialize_i16,  i16);
    serialize_fn!(serialize_i32,  i32);
    serialize_fn!(serialize_i64,  i64);
    serialize_fn!(serialize_f32,  f32);
    serialize_fn!(serialize_f64,  f64);
    serde_if_integer128! {
        serialize_fn!(serialize_u128,  u128);
        serialize_fn!(serialize_i128,  i128);
    }
    serialize_fn!(serialize_char, char);

    fn serialize_str(self, v: &str) -> Result {
        self.serialize_bytes(v.as_ref())
    }
    fn serialize_bytes(self, v: &[u8]) -> Result {
        self.write_len(v.len())?;
        self.writer.write(&v)
    }
    fn serialize_none(self) -> Result {
        self.serialize_u8(0)
    }
    fn serialize_some<T>(self, value: &T) -> Result
        where T: ?Sized + Serialize,
    {
        self.serialize_u8(1)?;
        value.serialize(self)
    }
    fn serialize_unit(self) -> Result { Ok(()) }

    fn serialize_unit_struct(self, _name: &'static str) -> Result {
        self.serialize_unit()
    }
    fn serialize_unit_variant(self, _name: &'static str, variant_index: u32,
                              _variant: &'static str) -> Result {
        self.write_discr(variant_index)
    }
    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result
        where T: ?Sized + Serialize,
    {
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: ?Sized>(self, _name: &'static str,
                                            variant_index: u32, _variant: &'static str,
                                            value: &T) -> Result
        where T: serde::ser::Serialize,
    {
        self.write_discr(variant_index)?;
        value.serialize(self)
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        Ok(SerializeCompound::new(self))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        Ok(SerializeCompound::new(self))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        self.write_discr(variant_index)?;
        Ok(SerializeCompound::new(self))
    }
    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        Ok(SerializeCompound::new(self))
    }
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        self.write_discr(variant_index)?;
        Ok(SerializeCompound::new(self))
    }
    // map and seq are variable-length sequences, use double encoding
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq> {
        let len = len.ok_or(Error::SerializeSequenceMustHaveLength)?;
        SerializeCompoundSeq::new(len, self)
    }
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap> {
        let len = len.ok_or(Error::SerializeSequenceMustHaveLength)?;
        SerializeCompoundSeq::new(len, self)
    }
    #[cfg(not(feature = "std"))]
    fn collect_str<T: ?Sized>(self, _value: &T) -> Result<Self::Ok, Self::Error> where
        T: core::fmt::Display {
        Err(Error::CannotSerializeDisplayInNoStdContext)
    }
}

pub struct SerializeCompound<'a, W, P: SerializerParams> {
    ser: &'a mut Serializer<W, P>,
}

impl <'a, W, P: SerializerParams> SerializeCompound<'a, W, P> {
    fn new(ser: &'a mut Serializer<W, P>) -> Self {
        Self { ser }
    }
}

macro_rules! seq_compound_impl {
    ($tn:ident, $fn:ident) => {
        impl<'a, W, P> serde::ser::$tn for SerializeCompound<'a, W, P>
            where W: TailWriteBytes,
                  P: SerializerParams,
        {
            type Ok = ();
            type Error = Error;

            fn $fn<T: ?Sized>(&mut self, value: &T) -> Result
                where T: serde::ser::Serialize,
            {
                value.serialize(&mut *self.ser)
            }
            fn end(self) -> Result {
                Ok(())
            }
        }
    }
}

seq_compound_impl!(SerializeSeq,   serialize_element);
seq_compound_impl!(SerializeTuple, serialize_element);
seq_compound_impl!(SerializeTupleStruct,  serialize_field);
seq_compound_impl!(SerializeTupleVariant, serialize_field);

macro_rules! struct_compound_impl {
    ($tn:ident) => {
        impl<'a, W, P> serde::ser::$tn for SerializeCompound<'a, W, P>
            where W: TailWriteBytes,
                  P: SerializerParams,
        {
            type Ok = ();
            type Error = Error;

            fn serialize_field<T: ?Sized>(&mut self, _key: &'static str, value: &T) -> Result
                where T: serde::ser::Serialize,
            {
                value.serialize(&mut *self.ser)
            }
            fn end(self) -> Result {
                Ok(())
            }
        }
    }
}

struct_compound_impl!(SerializeStruct);
struct_compound_impl!(SerializeStructVariant);

pub struct SerializeCompoundSeq<'a, W, P: SerializerParams> {
    ser: &'a mut Serializer<W, P>,
}

impl <'a, W, P> SerializeCompoundSeq<'a,  W, P>
    where W: TailWriteBytes,
          P: SerializerParams,
{
    fn new(len: usize, ser: &'a mut Serializer<W, P>) -> Result<Self> {
        ser.write_len(len)?;
        Ok(Self { ser })
    }
}

macro_rules! serialize_seqitem {
    ($fn:ident) => {
        fn $fn<T: ?Sized>(&mut self, value: &T) -> Result
            where T: serde::ser::Serialize,
        {
            value.serialize(&mut *self.ser)
        }
    }
}

impl<'a, W, P> serde::ser::SerializeSeq for SerializeCompoundSeq<'a, W, P>
    where W: TailWriteBytes,
          P: SerializerParams,
{
    type Ok = ();
    type Error = Error;

    serialize_seqitem!(serialize_element);
    fn end(self) -> Result { Ok(()) }
}

impl<'a, W, P> serde::ser::SerializeMap for SerializeCompoundSeq<'a, W, P>
    where W: TailWriteBytes,
          P: SerializerParams,
{
    type Ok = ();
    type Error = Error;

    serialize_seqitem!(serialize_key);
    serialize_seqitem!(serialize_value);
    fn end(self) -> Result { Ok(()) }
}
