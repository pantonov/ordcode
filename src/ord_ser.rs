// Serde serializer for data format which preserves lexicographical ordering of values

use crate::{Error, TailWriteBytes, Result, SerializerParams, LengthEncoder, AscendingOrder };
use serde::{ser, Serialize};
use serde::export::PhantomData;

/// Serialization data format version
//pub const VERSION: u8 = 1;

// Serde serializer which preserves lexicographical ordering of values
pub struct OrderedSerializer<W, P> {
    writer: W,
    _marker: std::marker::PhantomData<P>,
}

impl<W, P> OrderedSerializer<W, P>
    where W: TailWriteBytes,
          P: SerializerParams,
{
    pub fn new(writer: W, _params: P) -> Self {
        Self { writer, _marker: PhantomData }
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

macro_rules! serialize_fn {
    ($fn:ident, $t:ty) => {
        fn $fn(self, v: $t) -> Result { crate::primitives::$fn(&mut self.writer, v, AscendingOrder) }
    }
}

impl<'a, W, P> ser::Serializer for &'a mut OrderedSerializer<W, P>
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
        let len = len.ok_or_else(|| Error::SerializeSequenceMustHaveLength)?;
        SerializeCompoundSeq::new(len, self)
    }
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap> {
        let len = len.ok_or_else (|| Error::SerializeSequenceMustHaveLength)?;
        SerializeCompoundSeq::new(len, self)
    }
    #[cfg(any(feature = "std", feature = "alloc"))]
    fn collect_str<T: ?Sized>(self, value: &T) -> Result<Self::Ok, Self::Error>
        where
            T: core::fmt::Display,
    {
        self.serialize_str(&value.to_string())
    }
}

pub struct SerializeCompound<'a, W, P: SerializerParams> {
    ser: &'a mut OrderedSerializer<W, P>,
}

impl <'a, W, P: SerializerParams> SerializeCompound<'a, W, P> {
    fn new(ser: &'a mut OrderedSerializer<W, P>) -> Self {
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
    ser: &'a mut OrderedSerializer<W, P>,
}

impl <'a, W, P> SerializeCompoundSeq<'a,  W, P>
    where W: TailWriteBytes,
          P: SerializerParams,
{
    fn new(len: usize, ser: &'a mut OrderedSerializer<W, P>) -> Result<Self> {
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
