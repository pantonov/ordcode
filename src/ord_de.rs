// Serde deserializer for data format which preserves lexicographical ordering of values
use crate::{Error, TailReadBytes, Result, SerializerParams, LengthEncoder};
use serde::de::IntoDeserializer;

pub struct Deserializer<R, P> {
    reader: R,
    params: P,
}

impl<'de, R, P> Deserializer<R, P>
    where R: TailReadBytes,
          P: SerializerParams,
{
    #[must_use]
    pub fn new(reader: R, params: P) -> Self {
        Deserializer { reader, params }
    }
    pub fn into_reader(self) -> R { self.reader }

    fn visit_bytebuf<V, F>(&mut self, f: F) -> Result<V::Value>
        where V: serde::de::Visitor<'de>,
              F: FnOnce(&[u8]) -> Result<V::Value>
    {
        let len = P::SeqLenEncoder::read(&mut self.reader)?;
        self.reader.read(len, f)
    }
}

macro_rules! impl_nums {
    ($ty:ty, $dser_method:ident, $visitor_method:ident) => {
        #[inline]
        fn $dser_method<V>(self, visitor: V) -> Result<V::Value>
            where V: serde::de::Visitor<'de>,
        {
            let value = crate::primitives::$dser_method(&mut self.reader, self.params)?;
            visitor.$visitor_method(value)
        }
    }
}

impl<'a, 'de: 'a, R, P> serde::Deserializer<'de> for &'a mut Deserializer<R, P>
    where
        R: TailReadBytes,
        P: SerializerParams,
{
    type Error = Error;

    fn deserialize_any<V>(self, _visitor: V) -> Result<V::Value>
        where
            V: serde::de::Visitor<'de>,
    {
        Err(Error::DeserializeAnyNotSupported)
    }
    impl_nums!(u8,  deserialize_u8,  visit_u8);
    impl_nums!(u16, deserialize_u16, visit_u16);
    impl_nums!(u32, deserialize_u32, visit_u32);
    impl_nums!(u64, deserialize_u64, visit_u64);
    impl_nums!(i8,  deserialize_i8,  visit_i8);
    impl_nums!(i16, deserialize_i16, visit_i16);
    impl_nums!(i32, deserialize_i32, visit_i32);
    impl_nums!(i64, deserialize_i64, visit_i64);
    impl_nums!(f32, deserialize_f32, visit_f32);
    impl_nums!(f64, deserialize_f64, visit_f64);
    impl_nums!(bool, deserialize_bool, visit_bool);

    serde_if_integer128! {
        impl_nums!(u128, deserialize_u128, visit_u128);
        impl_nums!(i128, deserialize_i128, visit_i128);
    }
    impl_nums!(char, deserialize_char, visit_char);

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
        where
            V: serde::de::Visitor<'de>,
    {
        self.deserialize_bytes(visitor)
    }
    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
        where
            V: serde::de::Visitor<'de>,
    {
        self.visit_bytebuf::<V,_>(|buf| {
            visitor.visit_string(String::from_utf8(Vec::from(buf)).
                map_err(|_| Error::InvalidUtf8Encoding)?)
        })
    }
    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value>
        where
            V: serde::de::Visitor<'de>,
    {
        self.visit_bytebuf::<V,_>(|buf| {
            visitor.visit_bytes(buf)
        })
    }
    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value>
        where
            V: serde::de::Visitor<'de>,
    {
        self.deserialize_bytes(visitor)
    }
    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
        where
            V: serde::de::Visitor<'de>,
    {
        let value = crate::primitives::deserialize_u8(&mut self.reader, self.params)?;
        match value {
            0 => visitor.visit_none(),
            1 => visitor.visit_some(&mut *self),
            _ => Err(Error::InvalidTagEncoding),
        }
    }
    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value>
        where
            V: serde::de::Visitor<'de>,
    {
        visitor.visit_unit()
    }
    fn deserialize_unit_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
        where
            V: serde::de::Visitor<'de>,
    {
        visitor.visit_unit()
    }
    fn deserialize_newtype_struct<V>(self, _name: &str, visitor: V) -> Result<V::Value>
        where
            V: serde::de::Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }
    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
        where
            V: serde::de::Visitor<'de>,
    {
        let len = P::SeqLenEncoder::read(&mut self.reader)?;
        self.deserialize_tuple(len, visitor)
    }
    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value>
        where
            V: serde::de::Visitor<'de>,
    {
        visitor.visit_seq(SeqAccess { deserializer: self, len })
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value>
        where
            V: serde::de::Visitor<'de>,
    {
        self.deserialize_tuple(len, visitor)
    }
    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor<'de>,
    {
        let len = P::SeqLenEncoder::read(&mut self.reader)?;
        visitor.visit_map(MapAccess { deserializer: self, len })
    }
    fn deserialize_struct<V>(
        self,
        _name: &str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
        where
            V: serde::de::Visitor<'de>,
    {
        self.deserialize_tuple(fields.len(), visitor)
    }
    fn deserialize_enum<V>(
        self,
        _enum: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
        where
            V: serde::de::Visitor<'de>,
    {
        impl<'a, 'de: 'a, R, P> serde::de::EnumAccess<'de> for &'a mut Deserializer<R, P>
            where
                R: TailReadBytes,
                P: SerializerParams,
        {
            type Error = Error;
            type Variant = Self;

            fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant)>
                where
                    V: serde::de::DeserializeSeed<'de>,
            {
                let idx = P::DiscriminantEncoder::read(&mut self.reader)?;
                let val: Result<_> = seed.deserialize(idx.into_deserializer());
                Ok((val?, self))
            }
        }
        visitor.visit_enum(self)
    }
    fn deserialize_identifier<V>(self, _visitor: V) -> Result<V::Value>
        where
            V: serde::de::Visitor<'de>,
    {
        Err(Error::DeserializeIdentifierNotSupported)
    }

    fn deserialize_ignored_any<V>(self, _visitor: V) -> Result<V::Value>
        where
            V: serde::de::Visitor<'de>,
    {
        Err(Error::DeserializeIgnoredAny)
    }
    fn is_human_readable(&self) -> bool {
        false
    }
}

struct SeqAccess<'a, R: TailReadBytes, P: SerializerParams> {
    deserializer: &'a mut Deserializer<R, P>,
    len: usize,
}

impl<'a, 'de: 'a, R: TailReadBytes, P: SerializerParams> serde::de::SeqAccess<'de> for SeqAccess<'a, R, P>
{
    type Error = Error;
    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
        where
            T: serde::de::DeserializeSeed<'de>,
    {
        if self.len > 0 {
            self.len -= 1;
            let value = seed.deserialize(&mut *self.deserializer)?;
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }
    fn size_hint(&self) -> Option<usize> {
        Some(self.len)
    }
}

struct MapAccess<'a, R: TailReadBytes, P: SerializerParams> {
    deserializer: &'a mut Deserializer<R, P>,
    len: usize,
}
impl<'a, 'de: 'a, R: TailReadBytes, P: SerializerParams> serde::de::MapAccess<'de> for MapAccess<'a, R, P>
{
    type Error = Error;
    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
        where
            K: serde::de::DeserializeSeed<'de>,
    {
        if self.len > 0 {
            self.len -= 1;
            let key = seed.deserialize(&mut *self.deserializer)?;
            Ok(Some(key))
        } else {
            Ok(None)
        }
    }
    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
        where
            V: serde::de::DeserializeSeed<'de>,
    {
        let value = seed.deserialize(&mut *self.deserializer)?;
        Ok(value)
    }
    fn size_hint(&self) -> Option<usize> {
        Some(self.len)
    }
}

impl<'a, 'de: 'a, R, P> serde::de::VariantAccess<'de> for &'a mut Deserializer<R, P>
    where R: TailReadBytes,
          P: SerializerParams,
{
    type Error = Error;

    fn unit_variant(self) -> Result {
        Ok(())
    }
    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value>
        where
            T: serde::de::DeserializeSeed<'de>,
    {
        seed.deserialize(self)
    }

    fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value>
        where
            V: serde::de::Visitor<'de>,
    {
        serde::Deserializer::deserialize_tuple(self, len, visitor)
    }

    fn struct_variant<V>(self, fields: &'static [&'static str], visitor: V) -> Result<V::Value>
        where
            V: serde::de::Visitor<'de>,
    {
        serde::Deserializer::deserialize_tuple(self, fields.len(), visitor)
    }
}

