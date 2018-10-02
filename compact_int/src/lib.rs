extern crate byteorder;
extern crate serde;
extern crate vint;

// The serde_derive crate provides the macros for #[derive(Serialize)] and
// #[derive(Deserialize)]. You won't need these for implementing a data format
// but your unit tests will probably use them - hence #[cfg(test)].
// #[cfg(test)]
// #[macro_use]
// extern crate serde_derive;

// mod de;
mod de;
mod error;
mod ser;
use std::io::Read;
use std::io::Write;

pub use self::error::{Error, Result};

pub fn deserialize_from<R, T>(reader: R) -> Result<T>
where
    R: Read,
    T: serde::de::DeserializeOwned,
{
    let reader = de::IoReader::new(reader);
    let mut deserializer = de::Deserializer::<_>::new(reader);
    serde::Deserialize::deserialize(&mut deserializer)
}

pub fn deserialize<'a, T>(bytes: &'a [u8]) -> Result<T>
where
    T: serde::de::Deserialize<'a>,
{
    let reader = de::SliceReader::new(bytes);
    let mut deserializer = de::Deserializer::new(reader);
    serde::Deserialize::deserialize(&mut deserializer)
}

pub fn serialize<T: ?Sized>(value: &T) -> Result<Vec<u8>>
where
    T: serde::Serialize,
{
    let mut sink = vec![];
    serialize_into(&mut sink, value)?;
    Ok(sink)
}

pub fn serialize_into<W, T: ?Sized>(writer: W, value: &T) -> Result<()>
where
    W: Write,
    T: serde::Serialize,
{
    let mut serializer = ser::Serializer::<_>::new(writer);
    serde::Serialize::serialize(value, &mut serializer)
}

#[cfg(test)]
#[macro_use]
extern crate serde_derive;

#[cfg(test)]
mod tests {
    #[derive(Debug, Serialize, Deserialize)]
    struct TestSer {
        val1: u32,
        val2: u32,
    }
    #[test]
    fn test_struct_u32() {
        let test = TestSer { val1: 5, val2: 5 };
        let encoded: Vec<u8> = super::serialize(&test).unwrap();

        // 1 bytes each u32
        assert_eq!(encoded.len(), 2);

        let decoded: TestSer = super::deserialize(&encoded[..]).unwrap();
        println!("{:?}", decoded);
    }
    #[test]
    fn test_tuple_u32() {
        let test: (u32, u32) = (5, 5);
        let encoded: Vec<u8> = super::serialize(&test).unwrap();

        // 1 bytes each u32
        assert_eq!(encoded.len(), 2);

        let decoded: (u32, u32) = super::deserialize(&encoded[..]).unwrap();
        println!("{:?}", decoded);
        assert_eq!(test, decoded);
    }
    #[test]
    fn test_vec_u32() {
        let test: Vec<u32> = vec![5, 5];
        let encoded: Vec<u8> = super::serialize(&test).unwrap();

        // 1 bytes each u32
        assert_eq!(encoded.len(), 3);

        let decoded: Vec<u32> = super::deserialize(&encoded[..]).unwrap();
        println!("{:?}", decoded);
        assert_eq!(test, decoded);
    }
}
