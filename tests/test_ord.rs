#![cfg(feature="serde")]

#[macro_use] extern crate serde;
#[macro_use] extern crate serde_derive;

extern crate biord;
extern crate serde_bytes;

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::Debug;

use biord::*;

use serde::de::{ DeserializeOwned };
use serde::ser::Serialize;

fn serialize_asc<T: Serialize + ?Sized>(v: &T) -> Result<Vec<u8>> {
    ser_to_vec_ordered(v, Order::Ascending)
}

fn deserialize_asc<T: DeserializeOwned>(b: &[u8]) -> Result<T> {
    de_from_bytes_ordered_asc(b)
}

fn serialize_desc<T: Serialize + ?Sized>(v: &T) -> Result<Vec<u8>> {
    ser_to_vec_ordered(v, Order::Descending)
}

fn deserialize_desc<T: DeserializeOwned>(b: &mut [u8]) -> Result<T> {
    de_from_bytes_ordered(b, Order::Descending)
}

// Basic tests mostly adapted from 'bincode' crate
fn the_same<V>(element: V)
    where V: Serialize + DeserializeOwned + PartialEq + Debug + 'static,
{
    {
        let encoded = serialize_asc(&element).unwrap();
        let decoded = deserialize_asc(&encoded[..]).unwrap();

        if element != decoded { println!("MISMATCH: {:#?} {:#?}", encoded, decoded); }
        assert_eq!(element, decoded);
    }
    if false {
        let mut encoded = serialize_desc(&element).unwrap();
        let decoded = deserialize_desc(encoded.as_mut_slice()).unwrap();

        if element != decoded { println!("MISMATCH: {:#?} {:#?}", encoded, decoded); }
        assert_eq!(element, decoded);
    }
}

#[test]
fn test_numbers() {
    // unsigned positive
    the_same(5u8);
    the_same(5u16);
    the_same(5u32);
    the_same(5u64);
    the_same(5usize);
    // signed positive
    the_same(5i8);
    the_same(5i16);
    the_same(5i32);
    the_same(5i64);
    the_same(5isize);
    // signed negative
    the_same(-5i8);
    the_same(-5i16);
    the_same(-5i32);
    the_same(-5i64);
    the_same(-5isize);
    // floating
    the_same(-100f32);
    the_same(0f32);
    the_same(5f32);
    the_same(-100f64);
    the_same(5f64);
}

serde_if_integer128! {
    #[test]
    fn test_numbers_128bit() {
        // unsigned positive
        the_same(5u128);
        the_same(u128::max_value());
        // signed positive
        the_same(5i128);
        the_same(i128::max_value());
        // signed negative
        the_same(-5i128);
        the_same(i128::min_value());
    }
}

#[test]
fn test_string() {
    the_same("".to_string());
    the_same("a".to_string());
}

#[test]
fn test_tuple() {
    the_same((1isize,));
    the_same((1isize, 2isize, 3isize));
    the_same((1isize, "foo".to_string(), ()));
}

#[test]
fn test_basic_struct() {
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Easy {
        x: isize,
        s: String,
        y: usize,
    }
    the_same(Easy {
        x: -4,
        s: "foo".to_string(),
        y: 10,
    });
}


#[test]
fn test_nested_struct() {
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Easy {
        x: isize,
        s: String,
        y: usize,
    }
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Nest {
        f: Easy,
        b: usize,
        s: Easy,
    }

    the_same(Nest {
        f: Easy {
            x: -1,
            s: "foo".to_string(),
            y: 20,
        },
        b: 100,
        s: Easy {
            x: -100,
            s: "bar".to_string(),
            y: 20,
        },
    });
}

#[test]
fn test_struct_newtype() {
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct NewTypeStr(usize);

    the_same(NewTypeStr(5));
}

#[test]
fn test_struct_tuple() {
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct TubStr(usize, String, f32);

    the_same(TubStr(5, "hello".to_string(), 3.2));
}

#[test]
fn test_option() {
    the_same(Some(5usize));
    the_same(Some("foo bar".to_string()));
    the_same(None::<usize>);
}

#[test]
fn test_enum() {
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    enum TestEnum {
        NoArg,
        OneArg(usize),
        Args(usize, usize),
        AnotherNoArg,
        StructLike { x: usize, y: f32 },
    }
    the_same(TestEnum::NoArg);
    the_same(TestEnum::OneArg(4));
    the_same(TestEnum::Args(4, 5));
    the_same(TestEnum::AnotherNoArg);
    the_same(TestEnum::StructLike { x: 4, y: std::f32::consts::PI });
    the_same(vec![
        TestEnum::NoArg,
        TestEnum::OneArg(5),
        TestEnum::AnotherNoArg,
        TestEnum::StructLike { x: 4, y: 1.4 },
    ]);
}

#[test]
fn test_vec() {
    let v: Vec<u8> = vec![];
    the_same(v);
    the_same(vec![1u64]);
    the_same(vec![1u64, 2, 3, 4, 5, 6]);
}

#[test]
fn test_map() {
    let mut m = HashMap::new();
    m.insert(4u64, "foo".to_string());
    m.insert(0, "bar".to_string());
    m.insert(1342, "ahaha".to_string());
    the_same(m);
}

#[test]
fn test_bool() {
    the_same(true);
    the_same(false);
}

#[test]
fn test_unicode() {
    the_same("å".to_string());
    the_same("aåååååååa".to_string());
}

#[test]
fn test_fixed_size_array() {
    the_same([24u32; 32]);
    the_same([1u64, 2, 3, 4, 5, 6, 7, 8]);
    the_same([0u8; 19]);
}

#[test]
fn encode_box() {
    the_same(Box::new(5));
}

#[test]
fn test_cow_serialize() {
    let large_object = vec![1u32, 2, 3, 4, 5, 6];
    let mut large_map = HashMap::new();
    large_map.insert(1, 2);

    #[derive(Serialize, Deserialize, Debug)]
    enum Message<'a> {
        M1(Cow<'a, Vec<u32>>),
        M2(Cow<'a, HashMap<u32, u32>>),
    }

    // Test 1
    {
        let serialized = serialize_asc(&Message::M1(Cow::Borrowed(&large_object))).unwrap();
        let deserialized: Message<'static> = deserialize_asc(&serialized[..]).unwrap();

        match deserialized {
            Message::M1(b) => assert_eq!(b.into_owned(), large_object),
            _ => panic!(),
        }
    }

    // Test 2
    {
        let serialized = serialize_asc(&Message::M2(Cow::Borrowed(&large_map))).unwrap();
        let deserialized: Message<'static> = deserialize_asc(&serialized[..]).unwrap();

        match deserialized {
            Message::M2(b) => assert_eq!(b.into_owned(), large_map),
            _ => panic!(),
        }
    }
}

#[test]
fn test_strbox_serialize_asc() {
    let strx: &'static str = "hello world";
    let serialized = serialize_asc(&Cow::Borrowed(strx)).unwrap();
    let deserialized: Cow<'static, String> = deserialize_asc(&serialized[..]).unwrap();
    let stringx: String = deserialized.into_owned();
    assert_eq!(strx, &stringx[..]);
}

#[test]
fn test_strbox_serialize_desc() {
    let strx: &'static str = "hello world";
    let mut serialized = serialize_desc(&Cow::Borrowed(strx)).unwrap();
    let deserialized: Cow<'static, String> = deserialize_desc(serialized.as_mut_slice()).unwrap();
    let stringx: String = deserialized.into_owned();
    assert_eq!(strx, &stringx[..]);
}

#[test]
fn test_slicebox_serialize_asc() {
    let slice = [1u32, 2, 3, 4, 5];
    let serialized = serialize_asc(&Cow::Borrowed(&slice[..])).unwrap();
    let deserialized: Cow<'static, Vec<u32>> = deserialize_asc(&serialized[..]).unwrap();
    {
        let sb: &[u32] = &deserialized;
        assert_eq!(slice, sb);
    }
    let vecx: Vec<u32> = deserialized.into_owned();
    assert_eq!(slice, &vecx[..]);
}

#[test]
fn test_slicebox_serialize_desc() {
    let slice = [1u32, 2, 3, 4, 5];
    let mut serialized = serialize_desc(&Cow::Borrowed(&slice[..])).unwrap();
    let deserialized: Cow<'static, Vec<u32>> = deserialize_desc(serialized.as_mut_slice()).unwrap();
    {
        let sb: &[u32] = &deserialized;
        assert_eq!(slice, sb);
    }
    let vecx: Vec<u32> = deserialized.into_owned();
    assert_eq!(slice, &vecx[..]);
}

#[test]
fn test_multi_strings() {
    assert!(serialize_asc(&("foo", "bar", "baz")).is_ok());
}

#[test]
fn path_buf_asc() {
    use std::path::{Path, PathBuf};
    let path = Path::new("foo").to_path_buf();
    let serde_encoded = serialize_asc(&path).unwrap();
    let decoded: PathBuf = deserialize_asc(&serde_encoded).unwrap();
    assert_eq!(path.to_str(), decoded.to_str());
}

#[test]
fn path_buf_desc() {
    use std::path::{Path, PathBuf};
    let path = Path::new("foo").to_path_buf();
    let mut serde_encoded = serialize_desc(&path).unwrap();
    let decoded: PathBuf = deserialize_desc(serde_encoded.as_mut_slice()).unwrap();
    assert_eq!(path.to_str(), decoded.to_str());
}

#[test]
fn serde_bytes() {
    use serde_bytes::ByteBuf;
    the_same(ByteBuf::from(vec![1, 2, 3, 4, 5]));
}


#[test]
fn test_vec_parse() {
    #[derive(Serialize, Deserialize, Eq, PartialEq, Debug)]
    struct Foo {
        xstr:   String,
        xbytes: Vec<u8>,
    }

    let f = Foo {
        xstr: "hi".into(),
        xbytes: vec![0, 1, 2, 3],
    };
    if false {
        let encoded = serialize_asc(&f).unwrap();
        let out: Foo = deserialize_asc(&encoded).unwrap();
        assert_eq!(out, f);
    }
    {
        let mut encoded = serialize_desc(&f).unwrap();
        let out: Foo = deserialize_desc(encoded.as_mut_slice()).unwrap();
        assert_eq!(out, f);
    }
}

#[test]
fn test_byteseq() {
    let mut v = vec![];
    for j in 0..255 {
        for i in 0..255 {
            v.push((i^j) as u8);
        }
    }
    the_same(v);
}

/*
#[test]
fn test_writers() {
    let mut b : Vec<u8> = vec![];
    ord::to_bytes_writer(&mut b, "xren", Order::Ascending).unwrap();
    ord::to_bytes_writer(&mut b, "zzzz", Order::Ascending).unwrap();
    let mut r = BytesReader::new(&b);
    let s1 : String = ord::from_bytes_reader(&mut r, Order::Ascending).unwrap();
    let s2 : String = ord::from_bytes_reader(&mut r, Order::Ascending).unwrap();
    assert_eq!(s1, "xren");
    assert_eq!(s2, "zzzz");
}


#[test]
fn test_serializer_api() {
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Foo {
        a:  f32,
        b:  String,
        c:  (u16, u32),
    }
    let d = Foo { a: 11.1234, b: "привет".to_string(), c: (7, 99) };
    let mut b : Vec<u8> = vec![];
    let mut ser = ord::new_serializer_ascending(&mut b);
    d.serialize(&mut ser).unwrap();
    let r = BytesReader::new(&b);
    let mut deser = ord::new_deserializer_ascending(r);
    let res = Foo::deserialize(&mut deser).unwrap();
    let mut reader = deser.into_reader();
    assert!(reader.at_end());
    assert_eq!(d, res);
}

 */
