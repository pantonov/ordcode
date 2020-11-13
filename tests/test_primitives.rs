#![allow(clippy::float_cmp)]

use biord::{*, params::*, primitives, bytes_esc  };
use std::{f32, f64};

// test values, few normal ones plus corner cases
const V_U8:  &[u8]  = &[u8::min_value(), 0, 1, 10, 130, u8::max_value()];
const V_U16: &[u16] = &[u16::min_value(), 0, 1, 10, 1000, 65000, u16::max_value()];
const V_U32: &[u32] = &[u32::min_value(), 0, 1, 10, 65000, 999999, u32::max_value()];
const V_U64: &[u64] = &[u64::min_value(), 0, 1, 65000, 999999,
    (2<<40) + 999, u64::max_value()];

const V_I8:  &[i8]  = &[i8::min_value(), 0, 1, 10, 99, -1, -10, -99, i8::max_value()];
const V_I16: &[i16] = &[i16::min_value(), 0, 1, 10, 1000, 32700,
    -1, -10, -1000, -32700, i16::max_value()];
const V_I32: &[i32] = &[i32::min_value(), 0, 1, 10, 65000, 999999,
    -1, -10, -65000, -999999, i32::max_value()];
const V_I64: &[i64] = &[i64::min_value(), 0, 1, 65000, 999999, (2<<40) + 999,
    -1, -65000, -999999, -((2<<40) + 999), i64::max_value()];
#[cfg(not(no_i128))]
const V_U128: &[u128] = &[u128::min_value(), 0, 1, 65000, 999999,
                                (2<<90) + 999, u128::max_value()];
#[cfg(not(no_i128))]
const V_I128: &[i128] = &[i128::min_value(), 0, 1, 65000, 999999, (2<<90) + 999,
        -1, -65000, -999999, -((2<<90) + 999), i128::max_value()];
const V_BOOL: &[bool] = &[true, false];

const V_F32: &[f32] = &[f32::NEG_INFINITY, f32::MIN, 0.0, f32::MIN_POSITIVE,
    f32::MAX, f32::INFINITY, 10.123e10, -111.111e-11, 1.0, -1.0 ];
const V_F64: &[f64] = &[f64::NEG_INFINITY, f64::MIN, 0.0, f64::MIN_POSITIVE,
    f64::MAX, f64::INFINITY,  10.123e10, -111.111e-11, 1.0, -1.0 ];

macro_rules! test_ser {
    ($t:ty, $sfn:ident, $dfn:ident, $tvs:ident) => {
        #[test]
        fn $sfn() {
            for val in $tvs {
                let buf = &mut vec![0_u8; 128];
                let mut bb  = DeBytesWriter::new(buf);
                primitives::$sfn(&mut bb, *val, AscendingOrder).unwrap();
                primitives::$sfn(WriteToTail(&mut bb), *val, DescendingOrder).unwrap();
                let nl = bb.finalize().unwrap();
                let mut r = DeBytesReader::new(&buf[..nl]);
                assert_eq!(primitives::$dfn(&mut r, AscendingOrder).unwrap(), *val);
                assert_eq!(primitives::$dfn(ReadFromTail(&mut r), DescendingOrder).unwrap(), *val);
            }
        }
    }
}
test_ser!(u8,  serialize_u8,  deserialize_u8,  V_U8);
test_ser!(u16, serialize_u16, deserialize_u16, V_U16);
test_ser!(u32, serialize_u32, deserialize_u32, V_U32);
test_ser!(u64, serialize_u64, deserialize_u64, V_U64);
test_ser!(i8,  serialize_i8,  deserialize_i8,  V_I8);
test_ser!(i16, serialize_i16, deserialize_i16, V_I16);
test_ser!(i32, serialize_i32, deserialize_i32, V_I32);
test_ser!(i64, serialize_i64, deserialize_i64, V_I64);

#[cfg(not(no_i128))] test_ser!(u128, serialize_u128, deserialize_u128, V_U128);
#[cfg(not(no_i128))] test_ser!(i128, serialize_i128, deserialize_i128, V_I128);

test_ser!(f32, serialize_f32, deserialize_f32, V_F32);
test_ser!(f64, serialize_f64, deserialize_f64, V_F64);

test_ser!(bool,serialize_bool, deserialize_bool, V_BOOL);

macro_rules! test_cmpi {
    ($tn:ident, $t:ty, $sf:ident, $tvs:ident) => {
        #[test]
        fn $tn() {
            fn encode_asc(v: $t) -> Vec<u8> {
                let mut s = vec![];
                primitives::$sf(&mut s, v, AscendingOrder).unwrap();
                s
            }
            fn encode_desc(v: $t) -> Vec<u8> {
                let mut s = vec![];
                primitives::$sf(&mut s, v, DescendingOrder).unwrap();
                s
            }
            for v1 in $tvs {
                for v2 in $tvs {
                    assert_eq!(encode_asc(*v1) <= encode_asc(*v2), *v1 <= *v2);
                    assert_eq!(encode_asc(*v1) > encode_asc(*v2),  *v1 > *v2);
                    assert_eq!(encode_desc(*v1) <= encode_desc(*v2), *v1 >= *v2);
                    assert_eq!(encode_desc(*v1) > encode_desc(*v2), *v1 < *v2);
                }
            }
        }
    }
}
test_cmpi!(cmp_u8,  u8,  serialize_u8,  V_U8);
test_cmpi!(cmp_u16, u16, serialize_u16, V_U16);
test_cmpi!(cmp_u32, u32, serialize_u32, V_U32);
test_cmpi!(cmp_u64, u64, serialize_u64, V_U64);
test_cmpi!(cmp_i8,  i8,  serialize_i8,  V_I8);
test_cmpi!(cmp_i16, i16, serialize_i16, V_I16);
test_cmpi!(cmp_i32, i32, serialize_i32, V_I32);
test_cmpi!(cmp_i64, i64, serialize_i64, V_I64);

#[cfg(not(no_i128))] test_cmpi!(cmp_u128, u128, serialize_u128, V_U128);
#[cfg(not(no_i128))] test_cmpi!(cmp_i128, i128, serialize_i128, V_I128);

test_cmpi!(cmp_f32, f32, serialize_f32, V_F32);
test_cmpi!(cmp_f64, f64, serialize_f64, V_F64);

#[test]
fn test_cmp_f32_asc() {
    fn encode_asc(v: f32) -> Vec<u8> {
        let mut s = vec![];
        primitives::serialize_f32(&mut s, v, AscendingOrder).unwrap();
        s
    }
    assert!(encode_asc(f32::NEG_INFINITY) < encode_asc(f32::MIN));
    assert!(encode_asc(-0.0f32) < encode_asc(-0.0f32 + f32::EPSILON));
    assert!(encode_asc(-0f32) < encode_asc(0f32));
    assert!(encode_asc(0f32) < encode_asc(f32::MIN_POSITIVE));
    assert!(encode_asc(f32::MAX / 2.) < encode_asc(f32::MAX));
    assert!(encode_asc(f32::MAX) < encode_asc(f32::INFINITY));
    assert!(encode_asc(f32::INFINITY) < encode_asc(f32::NAN));
}

#[test]
fn test_cmp_f32_desc() {
    fn encode_desc(v: f32) -> Vec<u8> {
        let mut s = vec![];
        primitives::serialize_f32(&mut s, v, DescendingOrder).unwrap();
        s
    }
    assert!(encode_desc(f32::NEG_INFINITY) > encode_desc(f32::MIN));
    assert!(encode_desc(-0.0f32) > encode_desc(-0.0f32 + f32::EPSILON));
    assert!(encode_desc(-0f32) > encode_desc(0f32));
    assert!(encode_desc(0f32) > encode_desc(f32::MIN_POSITIVE));
    assert!(encode_desc(f32::MAX/2.) > encode_desc(f32::MAX));
    assert!(encode_desc(f32::MAX) > encode_desc(f32::INFINITY));
    assert!(encode_desc(f32::INFINITY) > encode_desc(f32::NAN));
}

#[test]
fn test_cmp_f64_asc() {
    fn encode(v: f64) -> Vec<u8> {
        let mut s = vec![];
        primitives::serialize_f64(&mut s, v, AscendingOrder).unwrap();
        s
    }
    assert!(encode(f64::NEG_INFINITY) < encode(f64::MIN));
    assert!(encode(-0.0f64) < encode(-0.0f64 + f64::EPSILON));
    assert!(encode(-0f64) < encode(0f64));
    assert!(encode(0f64) < encode(f64::MIN_POSITIVE));

    assert!(encode(f64::MAX/2.) < encode(f64::MAX));
    assert!(encode(f64::MAX) < encode(f64::INFINITY));
    assert!(encode(f64::INFINITY) < encode(f64::NAN));
}

#[test]
fn test_cmp_f64_desc() {
    fn encode(v: f64) -> Vec<u8> {
        let mut s = vec![];
        primitives::serialize_f64(&mut s, v, DescendingOrder).unwrap();
        s
    }
    assert!(encode(f64::NEG_INFINITY) > encode(f64::MIN));
    assert!(encode(-0.0f64) > encode(-0.0f64 + f64::EPSILON));
    assert!(encode(-0f64) > encode(0f64));
    assert!(encode(0f64) > encode(f64::MIN_POSITIVE));

    assert!(encode(f64::MAX/2.) > encode(f64::MAX));
    assert!(encode(f64::MAX) > encode(f64::INFINITY));
    assert!(encode(f64::INFINITY) > encode(f64::NAN));
}

#[test]
fn test_esc_enclen_asc() {
    let v = vec![0,0,0xF8,3,1,0,0xFF,0xF8,0xFE,1,2,7,0,1,0xFE];
    let mut s = vec![];
    bytes_esc::serialize_bytes(&mut s, v.as_slice(), AscendingOrder).unwrap();
    let mut r = DeBytesReader::new(&s);
    let len = bytes_esc::bytes_length(&mut r, AscendingOrder).unwrap();
    assert!(v.len() == len);
}

#[test]
fn test_esc_enclen_desc() {
    let v = vec![0,0,0xF8,3,1,0,7,0xFF,0xF8,0xFE,1,2,7,0,1,0xFE];
    let mut s = vec![];
    bytes_esc::serialize_bytes(&mut s, v.as_slice(), DescendingOrder).unwrap();
    let mut r = DeBytesReader::new(&s);
    let len = bytes_esc::bytes_length(&mut r, DescendingOrder).unwrap();
    assert!(v.len() == len);
}

fn cmp_esc_bytes_nested(param: impl EncodingParams) {
    let data = vec![0, 0u8,0,0xFF,0xF8,7, 3,1,0,0xFF,0,0xFE,1,2,3,0,1,
                              0xF1, 0xF1, 0xFF, 0xF1, 0x01, 0x0E, 0x00, 0x0E, 0xFE ];
    let mut s1 = vec![];
    let mut s2 = vec![];
    bytes_esc::serialize_bytes(&mut s1, data.as_slice(), param).unwrap();
    bytes_esc::serialize_bytes(&mut s2, &s1, param).unwrap();
    //println!("serialized step1={:#?} step2={:#?}", s1, s2);
    let mut r2 = DeBytesReader::new(&s2);
    let dv2= bytes_esc::deserialize_bytes_to_vec(&mut r2, param).unwrap();
    //println!("deserialized step2={:#?}", dv2);
    let mut r1 = DeBytesReader::new(&dv2);
    let dv1 = bytes_esc::deserialize_bytes_to_vec(&mut r1, param).unwrap();
    //println!("deserialized step1={:#?}", dv1);
    assert_eq!(data, dv1);
}

#[test]
fn bytes_esc_nested_asc() {
    cmp_esc_bytes_nested(AscendingOrder);
}

#[test]
fn bytes_esc_nested_desc() {
    cmp_esc_bytes_nested(DescendingOrder);
}