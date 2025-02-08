pub struct Packet {
    pub offset: u32,
    pub size: u32,
    pub id: u32,

    pub body: Vec<u8>,
}

impl Packet {
    pub fn new(body: Vec<u8>) -> Packet {
        if body.len() < 2 {
            return Packet {
                offset: 0,
                size: 0,
                id: 0,
                body,
            };
        }

        let mut packet = Packet {
            offset: 0,
            size: 0,
            id: 0,
            body,
        };

        packet.size = packet.read_var_int();
        packet.id = packet.read_var_int();

        return packet;
    }

    pub fn read_var_int(&mut self) -> u32 {
        let mut num_read = 0;
        let mut result = 0;
        let mut read: u8;

        loop {
            read = self.body[(self.offset + num_read) as usize];
            let value = (read & 0b01111111) as u32;
            result |= value << (7 * num_read);

            num_read += 1;

            if (read & 0b10000000) == 0 {
                break;
            }
        }

        self.offset += num_read;

        return result;
    }

    pub fn read_long(&mut self) -> u64 {
        let mut result = 0;
        for i in 0..8 {
            result |= (self.body[(self.offset + i) as usize] as u64) << (i * 8);
        }

        self.offset += 8;

        return result;
    }

    pub fn read_uint16(&mut self) -> u16 {
        let result = ((self.body[self.offset as usize] as u16) << 8)
            | (self.body[(self.offset + 1) as usize] as u16);
        self.offset += 2;

        return result;
    }

    pub fn read_string(&mut self) -> String {
        let length = self.read_var_int();
        let result = String::from_utf8(
            self.body[self.offset as usize..(self.offset + length) as usize].to_vec(),
        )
        .unwrap();
        self.offset += length;

        return result;
    }
}

pub struct PacketBuilder {
    body: Vec<u8>,
}

impl PacketBuilder {
    pub fn new() -> PacketBuilder {
        PacketBuilder { body: Vec::new() }
    }

    pub fn write_var_int(&mut self, mut value: u32) {
        loop {
            let mut temp = (value & 0b01111111) as u8;
            value >>= 7;

            if value != 0 {
                temp |= 0b10000000;
            }

            self.body.push(temp);

            if value == 0 {
                break;
            }
        }
    }

    pub fn write_long(&mut self, value: u64) {
        for i in 0..8 {
            self.body.push((value >> (i * 8)) as u8);
        }
    }

    pub fn write_uint16(&mut self, value: u16) {
        self.body.push((value >> 8) as u8);
        self.body.push((value & 0xFF) as u8);
    }

    pub fn write_string(&mut self, value: &str) {
        self.write_var_int(value.len() as u32);
        self.body.extend(value.as_bytes());
    }

    pub fn build(&self) -> Packet {
        let mut packet = PacketBuilder::new();

        packet.write_var_int(self.body.len() as u32);
        packet.body.extend(&self.body);

        return Packet::new(packet.body);
    }
}
