//! Write native model JSON with finite numeric tokens and the reader's manifest byte limit.

use std::io::{self, Write};

use serde::ser::{self, Serialize, Serializer};

pub(super) fn encode(value: &impl Serialize, limit: usize) -> serde_json::Result<Vec<u8>> {
    let mut output = Output {
        bytes: Vec::new(),
        limit,
    };
    let mut json = serde_json::Serializer::new(&mut output);
    value.serialize(Numbers(&mut json))?;
    Ok(output.bytes)
}

struct Output {
    bytes: Vec<u8>,
    limit: usize,
}

impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(io::Error::other("native manifest exceeds its byte budget"));
        }
        self.bytes.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct Numbers<S>(S);
struct Collection<S>(S);
struct Child<'a, T: ?Sized>(&'a T);

impl<T: Serialize + ?Sized> Serialize for Child<'_, T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(Numbers(serializer))
    }
}

// Delegate JSON atoms with no numeric policy, and re-enter it for each nested collection member.
macro_rules! atoms {
    ($(fn $method:ident($($arg:ident: $ty:ty),*);)+) => {$(
        fn $method(self, $($arg: $ty),*) -> Result<S::Ok, S::Error> {
            self.0.$method($($arg),*)
        }
    )+};
}

impl<S: Serializer> Serializer for Numbers<S> {
    type Ok = S::Ok;
    type Error = S::Error;
    type SerializeSeq = Collection<S::SerializeSeq>;
    type SerializeTuple = Collection<S::SerializeTuple>;
    type SerializeTupleStruct = Collection<S::SerializeTupleStruct>;
    type SerializeTupleVariant = Collection<S::SerializeTupleVariant>;
    type SerializeMap = Collection<S::SerializeMap>;
    type SerializeStruct = Collection<S::SerializeStruct>;
    type SerializeStructVariant = Collection<S::SerializeStructVariant>;

    atoms! {
        fn serialize_bool(value: bool);
        fn serialize_i8(value: i8);
        fn serialize_i16(value: i16);
        fn serialize_i32(value: i32);
        fn serialize_i64(value: i64);
        fn serialize_i128(value: i128);
        fn serialize_u8(value: u8);
        fn serialize_u16(value: u16);
        fn serialize_u32(value: u32);
        fn serialize_u64(value: u64);
        fn serialize_u128(value: u128);
        fn serialize_str(value: &str);
        fn serialize_char(value: char);
        fn serialize_bytes(value: &[u8]);
        fn serialize_none();
        fn serialize_unit();
        fn serialize_unit_struct(name: &'static str);
        fn serialize_unit_variant(name: &'static str, index: u32, variant: &'static str);
    }

    fn serialize_f32(self, value: f32) -> Result<S::Ok, S::Error> {
        self.0
            .serialize_f32(if value.is_finite() { value } else { 0.0 })
    }

    fn serialize_f64(self, value: f64) -> Result<S::Ok, S::Error> {
        self.0
            .serialize_f64(if value.is_finite() { value } else { 0.0 })
    }

    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<S::Ok, S::Error> {
        self.0.serialize_some(&Child(value))
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<S::Ok, S::Error> {
        self.0.serialize_newtype_struct(name, &Child(value))
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<S::Ok, S::Error> {
        self.0
            .serialize_newtype_variant(name, index, variant, &Child(value))
    }

    fn serialize_seq(self, size: Option<usize>) -> Result<Self::SerializeSeq, S::Error> {
        self.0.serialize_seq(size).map(Collection)
    }
    fn serialize_tuple(self, size: usize) -> Result<Self::SerializeTuple, S::Error> {
        self.0.serialize_tuple(size).map(Collection)
    }
    fn serialize_tuple_struct(
        self,
        name: &'static str,
        size: usize,
    ) -> Result<Self::SerializeTupleStruct, S::Error> {
        self.0.serialize_tuple_struct(name, size).map(Collection)
    }
    fn serialize_tuple_variant(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        size: usize,
    ) -> Result<Self::SerializeTupleVariant, S::Error> {
        self.0
            .serialize_tuple_variant(name, index, variant, size)
            .map(Collection)
    }
    fn serialize_map(self, size: Option<usize>) -> Result<Self::SerializeMap, S::Error> {
        self.0.serialize_map(size).map(Collection)
    }
    fn serialize_struct(
        self,
        name: &'static str,
        size: usize,
    ) -> Result<Self::SerializeStruct, S::Error> {
        self.0.serialize_struct(name, size).map(Collection)
    }
    fn serialize_struct_variant(
        self,
        name: &'static str,
        index: u32,
        variant: &'static str,
        size: usize,
    ) -> Result<Self::SerializeStructVariant, S::Error> {
        self.0
            .serialize_struct_variant(name, index, variant, size)
            .map(Collection)
    }
}

macro_rules! members {
    ($trait:ident { $($method:ident($($extra:ident: $ty:ty),*)),+ }) => {
        impl<S: ser::$trait> ser::$trait for Collection<S> {
            type Ok = S::Ok;
            type Error = S::Error;
            $(
                fn $method<T: Serialize + ?Sized>(&mut self, $($extra: $ty,)* value: &T) -> Result<(), S::Error> {
                    self.0.$method($($extra,)* &Child(value))
                }
            )+
            fn end(self) -> Result<S::Ok, S::Error> {
                self.0.end()
            }
        }
    };
}

members!(SerializeSeq { serialize_element() });
members!(SerializeTuple { serialize_element() });
members!(SerializeTupleStruct { serialize_field() });
members!(SerializeTupleVariant { serialize_field() });
members!(SerializeMap { serialize_key(), serialize_value() });
members!(SerializeStruct { serialize_field(name: &'static str) });
members!(SerializeStructVariant { serialize_field(name: &'static str) });

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_non_finite_values_remain_distinct_from_absence() -> serde_json::Result<()> {
        let value = [(Some(f32::INFINITY), None), (Some(2.0), Some(f64::NAN))];
        let json = encode(&value, 100)?;
        assert_eq!(json, br#"[[0.0,null],[2.0,0.0]]"#);
        assert!(encode(&value, json.len() - 1).is_err());
        assert_eq!(encode(&value, json.len())?, json);
        Ok(())
    }
}
