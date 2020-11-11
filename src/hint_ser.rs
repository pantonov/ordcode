// Fast, binary encoding, platform-neutral serializer for Serde

use crate::{Error,  Result};
use serde::{ser, Serialize};
use std::mem::size_of;

/// Serialization data format version
pub const VERSION: u8 = 1;

// Serde binary serializer, like bincode but platform independent
pub struct SizeCalc {
    size:   usize,
}

impl SizeCalc {
    #[inline]
    pub fn new() -> Self { Self { size: 0 } }

    #[inline]
    pub fn size(&self) -> usize { self.size }

    // add serialized size of primitive type
    #[inline]
    fn add_ty<T>(&mut self) { self.size += size_of::<T>(); }

    // add serialized length of sequence length or discriminant value
    #[inline]
    fn add_len(&mut self, v: usize) { self.size += Self::len_size(v); }

    #[inline]
    fn len_size(v: usize) -> usize { crate::varint::varu64_encoded_len(v as u64) as usize }
}

macro_rules! serialize_fn {
    ($fn:ident, $t:ty) => {
        #[inline]
        fn $fn(self, _v: $t) -> Result {
            self.add_ty::<$t>();
            Ok(())
        }
    }
}

impl<'a> ser::Serializer for &'a mut SizeCalc {
    type Ok = ();
    type Error = Error;

    type SerializeSeq = SerializeCompound<'a>;
    type SerializeTuple = SerializeCompound<'a>;
    type SerializeTupleStruct = SerializeCompound<'a>;
    type SerializeTupleVariant = SerializeCompound<'a>;
    type SerializeMap = SerializeCompound<'a>;
    type SerializeStruct = SerializeCompound<'a>;
    type SerializeStructVariant = SerializeCompound<'a>;

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
    serialize_fn!(serialize_char,  char);
    #[inline]
    fn serialize_str(self, v: &str) -> Result {
        self.serialize_bytes(v.as_ref())
    }
    #[inline]
    fn serialize_bytes(self, v: &[u8]) -> Result {
        self.add_len(v.len());
        self.size += v.len();
        Ok(())
    }
    #[inline]
    fn serialize_none(self) -> Result {
        self.add_ty::<u8>();
        Ok(())
    }
    #[inline]
    fn serialize_some<T>(self, value: &T) -> Result
        where T: ?Sized + Serialize,
    {
        self.add_ty::<u8>();
        value.serialize(self)
    }
    #[inline]
    fn serialize_unit(self) -> Result { Ok(()) }

    #[inline]
    fn serialize_unit_struct(self, _name: &'static str) -> Result {
        self.serialize_unit()
    }
    #[inline]
    fn serialize_unit_variant(self, _name: &'static str, variant_index: u32,
                              _variant: &'static str) -> Result {
        self.add_len(variant_index as usize);
        Ok(())
    }
    #[inline]
    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result
        where T: ?Sized + Serialize,
    {
        value.serialize(self)
    }
    #[inline]
    fn serialize_newtype_variant<T: ?Sized>(self, _name: &'static str,
                                            variant_index: u32, _variant: &'static str,
                                            value: &T) -> Result
        where T: serde::ser::Serialize,
    {
        self.add_len(variant_index as usize);
        value.serialize(self)
    }

    #[inline]
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        Ok(SerializeCompound { ser: self })
    }
    #[inline]
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        Ok(SerializeCompound { ser: self })
    }
    #[inline]
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        self.add_len(variant_index as usize);
        Ok(SerializeCompound { ser: self })
    }
    #[inline]
    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        Ok(SerializeCompound { ser: self })
    }
    #[inline]
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        self.add_len(variant_index as usize);
        Ok(SerializeCompound { ser: self })
    }
    #[inline]
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq> {
        let len = len.ok_or_else(|| errobj!(SerializeSequenceMustHaveLength))?;
        self.add_len(len);
        Ok(SerializeCompound { ser: self })
    }
    #[inline]
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap> {
        let len = len.ok_or_else(|| errobj!(SerializeSequenceMustHaveLength))?;
        self.add_len(len);
        Ok(SerializeCompound { ser: self })
    }
}

pub struct SerializeCompound<'a> {
    ser: &'a mut SizeCalc,
}

macro_rules! seq_compound_impl {
    ($tn:ident, $fn:ident) => {
        impl<'a> serde::ser::$tn for SerializeCompound<'a>
        {
            type Ok = ();
            type Error = Error;

            #[inline]
            fn $fn<T: ?Sized>(&mut self, value: &T) -> Result
                where T: serde::ser::Serialize,
            {
                value.serialize(&mut *self.ser)
            }
            #[inline]
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
        impl<'a> serde::ser::$tn for SerializeCompound<'a>
        {
            type Ok = ();
            type Error = Error;

            #[inline]
            fn serialize_field<T: ?Sized>(&mut self, _key: &'static str, value: &T) -> Result
                where T: serde::ser::Serialize,
            {
                value.serialize(&mut *self.ser)
            }
            #[inline]
            fn end(self) -> Result {
                Ok(())
            }
        }
    }
}

struct_compound_impl!(SerializeStruct);
struct_compound_impl!(SerializeStructVariant);

macro_rules! serialize_mapitem {
    ($fn:ident) => {
        #[inline]
        fn $fn<T: ?Sized>(&mut self, value: &T) -> Result
            where T: serde::ser::Serialize,
        {
            value.serialize(&mut *self.ser)
        }
    }
}

impl<'a> serde::ser::SerializeMap for SerializeCompound<'a>
{
    type Ok = ();
    type Error = Error;

    serialize_mapitem!(serialize_key);
    serialize_mapitem!(serialize_value);
    #[inline]
    fn end(self) -> Result { Ok(()) }
}