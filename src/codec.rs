use std::io::{self, Write, Read, Cursor};

use serde_json::{Value, to_writer, from_reader};

use queen::net::Codec;
use queen::crypto::Crypto;
use queen::error::{Result, Error};
use queen::nson::Message;

pub struct JsonCodec;

impl Codec for JsonCodec {
    fn new() -> Self {
        JsonCodec
    }

    fn decode(&mut self, crypto: &Option<Crypto>, mut bytes: Vec<u8>) -> Result<Message> {
        if let Some(crypto) = crypto {
            crypto.decrypt(&mut bytes).map_err(|err|
                Error::InvalidData(format!("{}", err))
            )?;
        }

        let json = decode_json(&bytes)?;

        Ok(json.into())
    }

    fn encode(&mut self, crypto: &Option<Crypto>, message: Message) -> Result<Vec<u8>> {
        let json: serde_json::Value = message.into();

        let mut bytes = encode_json(&json)?;

        if let Some(crypto) = crypto {
            crypto.encrypt(&mut bytes).map_err(|err|
                Error::InvalidData(format!("{}", err))
            )?;
        }

        Ok(bytes)
    }
}

#[inline]
pub(crate) fn write_u32(writer: &mut impl Write, val: u32) -> io::Result<()> {
    writer.write_all(&val.to_le_bytes())
}

#[inline]
pub(crate) fn read_u32(reader: &mut impl Read) -> io::Result<u32> {
    let mut buf = [0; 4];
    reader.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

pub fn encode_json(json: &Value) -> io::Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(64);
    write_u32(&mut buf, 0)?;

    to_writer(&mut buf, json)?;

    let len_bytes = (buf.len() as u32).to_le_bytes();
    buf[..4].clone_from_slice(&len_bytes);

    Ok(buf)
}

pub fn decode_json(slice: &[u8]) -> io::Result<Value> {
    let mut reader = Cursor::new(slice);

    read_u32(&mut reader)?;

    let json: Value = from_reader(&mut reader)?;

    Ok(json)
}

#[cfg(test)]
mod test {
    use serde_json::json;

    use super::{encode_json, decode_json};

    #[test]
    fn encode_and_decode() {
        let json = json!({
            "a": 123,
            "b": "456",
            "c": [7, 8, 9]
        });

        let data = encode_json(&json).unwrap();

        let json2 = decode_json(&data).unwrap();

        assert!(json == json2);
    }
}
