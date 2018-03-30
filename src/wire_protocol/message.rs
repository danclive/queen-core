use std::io::{Read, Write};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use super::error::Result;

/// Struct of the message
///
/// ```
/// //  00 01 02 03 04 05 06 07 08 09 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31
/// // +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
/// // |                                         Message Length                                        |
/// // +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
/// // |                                           Message ID                                          |
/// // +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
/// // |                                            Target...                                          |
/// // +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
/// // |                                            Origin...                                          |
/// // +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
/// // |                    OpCode                     |                 Content Type                  |
/// // +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
/// // |                                                                                               |
/// // //                                                                                             //
/// // //                                            Data                                             //
/// // //                                                                                             //
/// // |                                                                                               |
/// // +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
///
/// ```
//
// opcode:
//
// connect:     00000001 00000001
// connack:     00000001 00000010
// ping:        00000001 00000100
// pong:        00000001 00001000
//
// request:     00000010 00000001
// response:    00000010 00000010
// watch:       00000010 00000100
// watchack:    00000010 00001000
//
// subscribe:   00000100 00000001
// suback:      00000100 00000010
// unsubscribe: 00000100 00000100
// unsuback:    00000100 00001000
// publish:     00000100 00010000
// puback:      00000100 00100000
//

bitflags! {
    pub struct OpCode: u16 {
        const CONNECT       = 0b00000001_00000001;
        const CONNACK       = 0b00000001_00000010;
        const PING          = 0b00000001_00000100;
        const PONG          = 0b00000001_00001000;

        const REQUEST       = 0b00000010_00000001;
        const RESPONSE      = 0b00000010_00000010;
        const WATCH         = 0b00000010_00000100;
        const WATCHACK      = 0b00000010_00001000;

        const SUBSCRIBE     = 0b00000100_00000001;
        const SUBACK        = 0b00000100_00000010;
        const UNSUBSCRIBE   = 0b00000100_00000100;
        const UNSUBACK      = 0b00000100_00001000;
        const PUBLISH       = 0b00000100_00010000;
        const PUBACK        = 0b00000100_00100000;

        const UNKNOW        = 0b10000000_00000001;
        const ERROR         = 0b10000000_00000010;
    }
}

impl Default for OpCode {
    fn default() -> OpCode {
        OpCode::UNKNOW
    }
}

#[derive(Debug, Clone, Default)]
pub struct Message {
    //pub message_length: u32,
    pub message_id: u32,
    pub target: String,
    pub origin: String,
    pub opcode: OpCode,
    pub content_type: u16,
    pub body: Vec<u8>
}

impl Message {
    pub fn new(
        //message_length: u32,
        message_id: u32,
        target: String,
        origin: String,
        opcode: OpCode,
        content_type: u16,
        body: Vec<u8>
    ) -> Message {
        Message {
            message_id,
            target,
            origin,
            opcode,
            content_type,
            body
        }
    }

    pub fn len(&self) -> usize {
        let mut total_length = 4 + 4; // message_length + message_id
        total_length += self.target.len() + 1; // target
        total_length += self.origin.len() + 1; // origin
        total_length += 2 + 2; // opcode + content_type
        total_length += self.body.len(); // body

        total_length
    }

    pub fn write<W: Write>(&self, buffer: &mut W) -> Result<()> {

        let total_length = self.len();

        buffer.write_u32::<LittleEndian>(total_length as u32)?;
        buffer.write_u32::<LittleEndian>(self.message_id)?;
        write_cstring(buffer, &self.target)?;
        write_cstring(buffer, &self.origin)?;
        buffer.write_u16::<LittleEndian>(self.opcode.bits())?;
        buffer.write_u16::<LittleEndian>(self.content_type)?;
        buffer.write(&self.body)?;

        Ok(())
    }

    pub fn read<R: Read>(buffer: &mut R) -> Result<Message> {

        let mut total_length = buffer.read_u32::<LittleEndian>()?;
        total_length -= 4;

        let message_id = buffer.read_u32::<LittleEndian>()?;
        total_length -= 4;

        let target = read_cstring(buffer)?;
        total_length -= target.len() as u32 + 1;

        let origin = read_cstring(buffer)?;
        total_length -= origin.len() as u32 + 1;

        let opcode = buffer.read_u16::<LittleEndian>()?;
        total_length -= 2;

        let content_type = buffer.read_u16::<LittleEndian>()?;
        total_length -= 2;

        let body = if total_length > 0 {
            let mut body = vec![0u8; total_length as usize];
            let read_size = buffer.read(&mut body)? as u32;

            if read_size < total_length {
                panic!("read_size({:?}) < total_length({:?})", read_size, total_length);
            }

            body
        } else {
            vec![]
        };

        let opcode = OpCode::from_bits(opcode).unwrap_or_default();

        Ok(Message {
            message_id,
            target,
            origin,
            opcode,
            content_type,
            body
        })
    }
}

fn write_cstring<W>(writer: &mut W, s: &str) -> Result<()>
    where W: Write + ?Sized
{
    writer.write_all(s.as_bytes())?;
    writer.write_u8(0)?;
    Ok(())
}

fn read_cstring<R: Read + ?Sized>(reader: &mut R) -> Result<String> {
    let mut v = Vec::new();

    loop {
        let c = reader.read_u8()?;
        if c == 0 {
            break;
        }
        v.push(c);
    }

    Ok(String::from_utf8(v)?)
}
