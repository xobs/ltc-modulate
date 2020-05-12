use modulator;
extern crate byteorder;
extern crate crypto;
extern crate murmur3;

use self::crypto::digest::Digest;
use self::crypto::md5::Md5;

use ::EncodingRate;

use self::byteorder::{LittleEndian, WriteBytesExt};
use std::io::Cursor;

/// Which version of the data strip pattern is used
#[derive(Clone, Copy, Debug)]
pub enum ProtocolVersion {
    /// Original v1 (0xaa, 0x55)
    V1,

    /// Improved v2 (0x35, 0xac, 0x95)
    V2,
}

impl ProtocolVersion {
    pub fn as_num(self) -> u8 {
        match self {
            ProtocolVersion::V1 => 1,
            ProtocolVersion::V2 => 2,
        }
    }
}

pub struct Controller {
    rate: f64,
    os_update: bool,
    modulator: modulator::Modulator,
    protocol_version: ProtocolVersion,
    preamble: Vec<u8>,
    stop_bytes: Vec<u8>,
}

// Preamble sent before every audio packet
const PREAMBLE: [u8; 7] = [0x00, 0x00, 0x00, 0x00, 0xaa, 0x55, 0x42];

// Stop bits, sent to pad the end of transmission
const STOP_BYTES: [u8; 1] = [0xff];

// Packet types
const CONTROL_PACKET: u8 = 0x01;
const DATA_PACKET: u8 = 0x02;
const CONTROL_OS_PACKET: u8 = 0x03;
const DATA_OS_PACKET: u8 = 0x04;

impl Controller {
    pub fn new(rate: f64, os_update: bool, protocol_version: ProtocolVersion) -> Controller {
        Controller {
            rate,
            os_update,
            protocol_version,
            modulator: modulator::Modulator::new(rate),
            preamble: PREAMBLE.to_vec(),
            stop_bytes: STOP_BYTES.to_vec(),
        }
    }

    pub fn make_preamble(&self) -> Vec<u8> {
        let mut header = vec![];
        for byte in &self.preamble {
            header.push(*byte);
        }
        header
    }

    pub fn make_control_header(&self) -> Vec<u8> {
        let mut header = self.make_preamble();
        header.push(self.protocol_version.as_num());
        header.push(CONTROL_PACKET);
        header.push(0x00);
        header.push(0x00);
        header
    }

    pub fn make_data_header(&self, block_number: u16) -> Vec<u8> {
        let mut header = self.make_preamble();
        header.push(self.protocol_version.as_num());
        header.push(DATA_PACKET);
        header.push((block_number & 0xff) as u8);
        header.push(((block_number >> 8) & 0xff) as u8);
        header
    }

    pub fn make_control_os_header(&self) -> Vec<u8> {
        let mut header = self.make_preamble();
        header.push(self.protocol_version.as_num());
        header.push(CONTROL_OS_PACKET);
        header.push(0x00);
        header.push(0x00);
        header
    }

    pub fn make_data_os_header(&self, block_number: u16) -> Vec<u8> {
        let mut header = self.make_preamble();
        header.push(self.protocol_version.as_num());
        header.push(DATA_OS_PACKET);
        header.push((block_number & 0xff) as u8);
        header.push(((block_number >> 8) & 0xff) as u8);
        header
    }

    pub fn append_data(&self, buffer: &mut Vec<u8>, data: &[u8]) {
        for byte in data.iter() {
            buffer.push(*byte);
        }
    }

    pub fn make_footer(&self, data: &[u8]) -> Vec<u8> {
        let hash = 0xdead_beefu32;
        let mut data_cursor = Cursor::new(data);
        data_cursor.set_position(7); // seek past the data header
        let data_hash_32 = murmur3::murmur3_32(&mut data_cursor, hash);
        let mut data_hash = vec![];
        data_hash.write_u32::<LittleEndian>(data_hash_32).unwrap();
        data_hash
    }

    pub fn make_control_packet(&mut self, data: &[u8]) -> Vec<u8> {
        let mut packet = vec![];

        let control_header = if self.os_update {
            self.make_control_os_header()
        } else {
            self.make_control_header()
        };
        self.append_data(&mut packet, &control_header);

        let mut program_length = vec![];
        program_length
            .write_u32::<LittleEndian>(data.len() as u32)
            .unwrap();
        self.append_data(&mut packet, &program_length);

        let program_hash_32 = murmur3::murmur3_32(&mut Cursor::new(&data), 0x32d0_babe);
        let mut program_hash = vec![];
        program_hash
            .write_u32::<LittleEndian>(program_hash_32)
            .unwrap();
        self.append_data(&mut packet, &program_hash);

        let mut program_guid_hasher = Md5::new();
        let mut program_guid_array = [0; 16];
        program_guid_hasher.input(data);
        program_guid_hasher.result(&mut program_guid_array);
        let program_guid = program_guid_array.to_vec();
        self.append_data(&mut packet, &program_guid);

        let footer = self.make_footer(&packet);
        self.append_data(&mut packet, &footer);

        self.append_data(&mut packet, &self.stop_bytes);

        packet
    }

    pub fn make_data_packet(&mut self, data_in: &[u8], block_num: u16) -> Vec<u8> {
        let mut packet = vec![];
        let mut data = data_in.to_owned();
        let data_header = if self.os_update {
            self.make_data_os_header(block_num)
        } else {
            self.make_data_header(block_num)
        };
        let data_header_len = data_header.len();
        self.append_data(&mut packet, &data_header);

        // Ensure the "data" payload is 256 bytes long.
        data.resize(256, 0xff);
        let data_len = data.len();
        self.append_data(&mut packet, &data);

        let footer = self.make_footer(&packet);
        self.append_data(&mut packet, &footer);

        // let stop_bytes = vec![0xff, 0xff];
        self.append_data(&mut packet, &self.stop_bytes);

        // After the hash has been computed, stripe the data portion
        // with a pattern of 0x55 and 0xaa.  This provides some level
        // of DC balance, even at the end where we have lots of 0xff.

        match self.protocol_version {
            ProtocolVersion::V1 => {
                for i in 0..data_len {
                    if (i % 16) == 3 {
                        packet[i + data_header_len] ^= 0x55;
                    } else if (i % 16) == 11 {
                        packet[i + data_header_len] ^= 0xaa;
                    }
                }
            }

            ProtocolVersion::V2 => {
                // modulate the packet # and payload
                // so skip preamble + version + type (7 bytes preamble + 1 byte version + 1 byte type = 9)
                // and then "add 2" in the modular math loop because on the demod side we are 2-offset
                // also skip capping hash and stop bytes
                let mod_range = (self.preamble.len() + 2)..(packet.len() - (4 + self.stop_bytes.len()));
                for i in mod_range {
                    if ((i - 9 + 2) % 3) == 0 {
                        packet[i] ^= 0x35;
                    } else if ((i - 9 + 2) % 3) == 1 {
                        packet[i] ^= 0xac;
                    } else if ((i - 9 + 2) % 3) == 2 {
                        packet[i] ^= 0x95;
                    }
                }
            }
        }

        packet
    }

    pub fn make_silence(&mut self, msecs: u32) -> Vec<f64> {
        let mut buffer: Vec<f64> = vec![];

        let silence_length = (self.rate / (1000.0 / msecs as f64)).ceil() as usize;
        buffer.resize(silence_length, 0f64);
        buffer
    }

    pub fn make_zero(&mut self, number: u32) -> Vec<u8> {
        let mut buffer: Vec<u8> = vec![];

        buffer.resize((number / 8) as usize, 0);

        buffer
    }

    pub fn pilot(&mut self, output: &mut Vec<f64>, rate: &EncodingRate) {
        if *rate == EncodingRate::Low {
            let data = self.make_zero(4000); // ~0.5secs
            let mut audio = self.modulator.modulate_pcm(&data);
            output.append(&mut audio);
        } else {
            // // no preamble at high rate, this is the default
            // let data = self.make_one(3000); // ~0.5secs
            // let mut audio = self.modulator.modulate_pcm(&data);
            // output.append(&mut audio);
            //
        }
    }

    pub fn encode(&mut self, input: &[u8], output: &mut Vec<f64>, rate: &EncodingRate) {
        let silence_divisor = rate.silence_divisor();
        let file_length = input.len();

        // Note: Maximum of 65536 blocks
        let blocks = ((file_length as f64 / 256.0).ceil()) as u16;

        let mut audio = self.make_silence(250 / silence_divisor);
        output.append(&mut audio);

        let data = self.make_control_packet(&input);
        let mut audio = self.modulator.modulate_pcm(&data);
        output.append(&mut audio);

        let mut audio = self.make_silence(100 / silence_divisor);
        output.append(&mut audio);

        // Make two header packets
        let data = self.make_control_packet(&input);
        let mut audio = self.modulator.modulate_pcm(&data);
        output.append(&mut audio);

        let mut audio = self.make_silence(500 / silence_divisor);
        output.append(&mut audio);

        for mut packet_num in 0..blocks {
            packet_num &= 0xff;
            let slice_start = packet_num * 256;
            let mut packet_data: Vec<u8> = vec![];
            for i in 0..256 {
                let target_offset = (slice_start + i) as usize;
                if target_offset < input.len() {
                    packet_data.push(input[target_offset]);
                } else {
                    packet_data.push(0xff);
                }
            }
            let data = self.make_data_packet(&packet_data, packet_num as u16);
            let mut audio = self.modulator.modulate_pcm(&data);
            output.append(&mut audio);

            let mut audio = self.make_silence(80 / silence_divisor);
            output.append(&mut audio);
        }

        let mut audio = self.make_silence(500 / silence_divisor);
        output.append(&mut audio);
    }
}
