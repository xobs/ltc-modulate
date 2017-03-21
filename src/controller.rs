use modulator;
extern crate byteorder;
extern crate murmur3;
extern crate crypto;

use self::crypto::md5::Md5;
use self::crypto::digest::Digest;

use std::io::Cursor;
use self::byteorder::{LittleEndian, WriteBytesExt};

/// Which version of the data strip pattern is used
#[derive(Clone, Copy, Debug)]
pub enum ProtocolVersion {
    /// Original v1 (0xaa, 0x55)
    V1,

    /// Improved v2 (0x35, 0xac, 0x95)
    V2,
}

pub struct Controller {
    rate: f64,
    os_update: bool,
    modulator: modulator::Modulator,
    protocol_version: ProtocolVersion,
}

// Preamble sent before every audio packet
// const preamble: [u8; 7] = [0x00, 0x00, 0x00, 0x00, 0xaa, 0x55, 0x42];

// Stop bits, sent to pad the end of transmission
// const stop_bytes_const: [u8; 1] = [0xff];


// Protocol version, currently v1.0
const PROTOCOL_VERSION: u8 = 0x01;

// Packet types
const CONTROL_PACKET: u8 = 0x01;
const DATA_PACKET: u8 = 0x02;
const CONTROL_OS_PACKET: u8 = 0x03;
const DATA_OS_PACKET: u8 = 0x04;

impl Controller {
    pub fn new(rate: f64, os_update: bool, protocol_version: ProtocolVersion) -> Controller {
        Controller {
            rate: rate,
            os_update: os_update,
            protocol_version: protocol_version,
            modulator: modulator::Modulator::new(rate),
        }
    }

    pub fn make_control_header(&mut self) -> Vec<u8> {
        vec![0x00, 0x00, 0x00, 0x00, 0xaa, 0x55, 0x42, PROTOCOL_VERSION, CONTROL_PACKET, 0x00, 0x00]
    }

    pub fn make_data_header(&mut self, block_number: u16) -> Vec<u8> {
        vec![0x00,
             0x00,
             0x00,
             0x00,
             0xaa,
             0x55,
             0x42,
             PROTOCOL_VERSION,
             DATA_PACKET,
             (block_number & 0xff) as u8,
             ((block_number >> 8) & 0xff) as u8]
    }

    pub fn make_control_os_header(&mut self) -> Vec<u8> {
        vec![0x00,
             0x00,
             0x00,
             0x00,
             0xaa,
             0x55,
             0x42,
             PROTOCOL_VERSION,
             CONTROL_OS_PACKET,
             0x00,
             0x00]
    }

    pub fn make_data_os_header(&mut self, block_number: u16) -> Vec<u8> {
        vec![0x00,
             0x00,
             0x00,
             0x00,
             0xaa,
             0x55,
             0x42,
             PROTOCOL_VERSION,
             DATA_OS_PACKET,
             (block_number & 0xff) as u8,
             ((block_number >> 8) & 0xff) as u8]
    }

    pub fn append_data(&mut self, buffer: &mut Vec<u8>, data: Vec<u8>) {
        for byte in data.iter() {
            buffer.push(*byte);
        }
    }

    pub fn make_footer(&mut self, data: &Vec<u8>) -> Vec<u8> {
        let hash = 0xdeadbeefu32;
        let mut data_cursor = Cursor::new(data);
        data_cursor.set_position(7); // seek past the data header
        let data_hash_32 = murmur3::murmur3_32(&mut data_cursor, hash);
        let mut data_hash = vec![];
        data_hash.write_u32::<LittleEndian>(data_hash_32).unwrap();
        data_hash
    }

    pub fn make_control_packet(&mut self, data: &Vec<u8>) -> Vec<u8> {

        let mut packet = vec![];

        let control_header = if self.os_update {
            self.make_control_os_header()
        } else {
            self.make_control_header()
        };
        self.append_data(&mut packet, control_header);

        let mut program_length = vec![];
        program_length.write_u32::<LittleEndian>(data.len() as u32).unwrap();
        self.append_data(&mut packet, program_length);

        let program_hash_32 = murmur3::murmur3_32(&mut Cursor::new(&data), 0x32d0babe);
        let mut program_hash = vec![];
        program_hash.write_u32::<LittleEndian>(program_hash_32).unwrap();
        self.append_data(&mut packet, program_hash);

        let mut program_guid_hasher = Md5::new();
        let mut program_guid_array = [0; 16];
        program_guid_hasher.input(data);
        program_guid_hasher.result(&mut program_guid_array);
        let program_guid = program_guid_array.to_vec();
        self.append_data(&mut packet, program_guid);

        let footer = self.make_footer(&packet);
        self.append_data(&mut packet, footer);

        let stop_bytes = vec![0xff, 0xff];
        self.append_data(&mut packet, stop_bytes);

        packet
    }

    pub fn make_data_packet(&mut self, data_in: &Vec<u8>, block_num: u16) -> Vec<u8> {
        let mut packet = vec![];
        let mut data = data_in.clone();
        let data_header = if self.os_update {
            self.make_data_os_header(block_num)
        } else {
            self.make_data_header(block_num)
        };
        let data_header_len = data_header.len();
        self.append_data(&mut packet, data_header);

        // Ensure the "data" payload is 256 bytes long.
        data.resize(256, 0xff);
        let data_len = data.len();
        self.append_data(&mut packet, data);

        let footer = self.make_footer(&packet);
        self.append_data(&mut packet, footer);

        let stop_bytes = vec![0xff, 0xff];
        self.append_data(&mut packet, stop_bytes);

        // After the hash has been computed, stripe the data portion
        // with a pattern of 0x55 and 0xaa.  This provides some level
        // of DC balance, even at the end where we have lots of 0xff.

        match self.protocol_version {
            ProtocolVersion::V1 => {
                for i in 0..data_len {
                    if (i % 16) == 3 {
                        packet[i + data_header_len] = packet[i + data_header_len] ^ 0x55;
                    } else if (i % 16) == 11 {
                        packet[i + data_header_len] = packet[i + data_header_len] ^ 0xaa;
                    }
                }
            },

            ProtocolVersion::V2 =>
                // modulate the packet # and payload
                // so skip preamble + version + type (7 bytes preamble + 1 byte version + 1 byte type = 9)
                // and then "add 2" in the modular math loop because on the demod side we are 2-offset
                // also skip capping hash
                // for some reason, the Rust version puts 2 bytes 0xff pad, which isn't necessary
                // hence the end is 6 bytes off set (4 bytes hash + 2 bytes pad)
                for i in 9..(packet.len() - 6) {
                    if ((i-9+2) % 3) == 0 {
                        packet[i] = packet[i] ^ 0x35;
                    } else if ((i-9+2) % 3) == 1 {
                        packet[i] = packet[i] ^ 0xac;
                    } else if ((i-9+2) % 3) == 2 {
                        packet[i] = packet[i] ^ 0x95;
                    }
                },
        }

        packet
    }

    pub fn make_silence(&mut self, msecs: u32) -> Vec<i16> {
        let mut buffer: Vec<i16> = vec![];

        let silence_length = (self.rate / (1000.0 / msecs as f64)).ceil() as usize;
        buffer.resize(silence_length, 0);
        buffer
    }

    pub fn make_zero(&mut self, number: u32) -> Vec<u8> {
        let mut buffer: Vec<u8> = vec![];

        buffer.resize((number / 8) as usize, 0);

        buffer
    }

    pub fn pilot(&mut self, output: &mut Vec<i16>, rate: u32) {
        if rate == 0 {
            // low rate preamble
            let data = self.make_zero(4000); // ~0.5secs
            let mut audio = self.modulator.modulate_pcm(&data);
            output.append(&mut audio);
        } else {
            // high rate preamble
            // // no preamble at high rate, this is the default
            // let data = self.make_one(3000); // ~0.5secs
            // let mut audio = self.modulator.modulate_pcm(&data);
            // output.append(&mut audio);
            //

        }
    }

    pub fn encode(&mut self, input: &Vec<u8>, output: &mut Vec<i16>, rate: u32) {
        let mut silence_divisor = 1;
        if rate == 0 {
            silence_divisor = 4;
        } else if rate == 1 {
            silence_divisor = 2;
        }
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
            packet_num = packet_num & 0xff;
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