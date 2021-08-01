use std::io::{self, Write};

pub const MTU: u32 = 1400;
#[derive(Debug, Default)]
pub struct Packet {
    pub header: Header,
    pub chan: String,
    pub body: Vec<u8>
}

#[derive(Debug, Default)]
pub struct Header {
    bytes: [u8; 8],
}

#[derive(Debug)]
#[repr(u8)]
pub enum Type {
    Non,
    Con,
    Ack,
    Rst
}

#[derive(Debug)]
#[repr(u8)]
pub enum Compress {
    None,
    Zstd,
    Gzip
}

#[derive(Debug)]
#[repr(u8)]
pub enum Crypto {
    None,
    Aes128Gcm,
    Aes256Gcm,
    ChaCha20Poly1305
}

impl Packet {
    pub fn new() -> Self {
        let header = Header::new();

        Packet {
            header,
            chan: String::new(),
            body: Vec::new()
        }
    }

    pub fn from_bytes(_bytes: &[u8]) {

    }

    pub fn to_bytes(&self) -> io::Result<Vec<u8>> {
        let mut buffer = Vec::new();

        buffer.extend(&self.header.bytes);
        buffer.write_all(self.chan.as_bytes())?;
        buffer.write_all(&[0])?;

        //
        buffer.extend(&self.body);

        Ok(buffer)
    }
}

impl Header {
    pub const VERSION: u8 = 1;

    pub fn new() -> Self {
        let mut header = Header::default();

        header.bytes[2] = Self::VERSION;

        header
    }

    pub fn from_bytes(_bytes: [u8; 8]) {
        todo!()
    }

    pub unsafe fn from_bytes_unchecked(bytes: [u8; 8]) -> Self {
        Header { bytes }
    }

    pub fn message_id(&self) -> u16 {
        let mut bytes = [0u8; 2];
        bytes.copy_from_slice(&self.bytes[..2]);
        u16::from_le_bytes(bytes)
    }

    pub fn set_message_id(&mut self, message_id: u16) {
        self.bytes[..2].copy_from_slice(&message_id.to_le_bytes());
    }

    pub fn r#type(&self) -> Type {
        todo!()
    }

    pub fn set_type(&mut self, r#type: Type) {
        self.bytes[3] = r#type as u8;
    }

    pub fn code(&self) -> u8 {
        self.bytes[4]
    }

    pub fn set_code(&mut self, code: u8) {
        self.bytes[4] = code;
    }

    pub fn compress(&self) -> Compress {
        todo!()
    }

    pub fn set_compress(&mut self, m: Compress) {
        self.bytes[5] &= 0b00001111;
        self.bytes[5] |= (m as u8) << 4;
    }

    pub fn crypto(&self) -> Crypto {
        todo!()
    }

    pub fn set_crypto(&mut self, m: Crypto) {
        self.bytes[5] &= 0b11110000;
        self.bytes[5] |= m as u8;
    }

    pub fn content_type(&self) -> u8 {
       self.bytes[6]
    }

    pub fn set_content_type(&mut self, c: u8) {
        self.bytes[6] = c;
    }

    pub fn ext(&self) -> u8 {
        self.bytes[7]
    }

    pub fn set_ext(&mut self, ext: u8) {
        self.bytes[7] = ext;
    }

    pub fn bytes(&self) -> [u8; 8] {
        self.bytes
    }
}

#[test]
fn set_compress_and_crypto() {
    let mut packet = Packet::new();

    packet.header.set_crypto(Crypto::ChaCha20Poly1305);
    assert_eq!(packet.header.bytes[5], 3);

    packet.header.set_compress(Compress::Zstd);
    assert_eq!(packet.header.bytes[5], 16 + 3);

    packet.header.set_crypto(Crypto::None);
    assert_eq!(packet.header.bytes[5], 16);

    packet.header.set_compress(Compress::None);
    assert_eq!(packet.header.bytes[5], 0);

    packet.header.set_message_id(123);

    panic!("{:?}", packet)
}
