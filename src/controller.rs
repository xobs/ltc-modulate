use modulator;
extern crate byteorder;
extern crate murmur3;
extern crate crypto;

use self::crypto::md5::Md5;
use self::crypto::digest::Digest;

use std::io::Cursor;
use self::byteorder::{LittleEndian, WriteBytesExt};

pub struct Controller {
    rate: f64,
    modulator: modulator::Modulator,
}

/* Preamble sent before every audio packet */
//const preamble: [u8; 7] = [0x00, 0x00, 0x00, 0x00, 0xaa, 0x55, 0x42];

/* Stop bits, sent to pad the end of transmission */
//const stop_bytes_const: [u8; 1] = [0xff];


/* Protocol version, currently v1.0 */
const PROTOCOL_VERSION: u8 = 0x01;

/* Packet types */
const CONTROL_PACKET: u8 = 0x01;
const DATA_PACKET: u8 = 0x02;

impl Controller {

    pub fn new(rate: f64) -> Controller {
        Controller {
            rate: rate,
            modulator: modulator::Modulator::new(rate),
        }
    }

    pub fn make_control_header(&mut self) -> Vec<u8> {
        vec!(0x00, 0x00, 0x00, 0x00, 0xaa, 0x55, 0x42,
             PROTOCOL_VERSION, CONTROL_PACKET, 0x00, 0x00)
    }

    pub fn make_data_header(&mut self, block_number: u16) -> Vec<u8> {
        vec!(0x00, 0x00, 0x00, 0x00, 0xaa, 0x55, 0x42,
             PROTOCOL_VERSION,
             DATA_PACKET,
             (block_number & 0xff) as u8,
             ((block_number >> 8) & 0xff) as u8)
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

        let control_header = self.make_control_header();
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
        let data_header = self.make_data_header(block_num);
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
        for i in 0..data_len {
            if (i % 16) == 3 {
                packet[i + data_header_len] = packet[i + data_header_len] ^ 0x55;
            } else if (i % 16) == 11 {
                packet[i + data_header_len] = packet[i + data_header_len] ^ 0xaa;
            }
        }
        packet
    }
/*
    makeDataPacket: function(dataIn, blocknum) {
        var i;

        // now assemble the packet
        var preamble = this.preamble;
        var header = this.makeDataHeader(blocknum);

        // Ensure the "data" payload is 256 bytes long.
        var data = new Uint8Array(256);
        for (i = 0; i < data.length; i++) data[i] = 0xff; // data.fill(0xff)
        this.appendData(data, dataIn, 0);

        var footer = this.makeFooter(header, data);
        var stop = this.stop_bytes;

        // 256 byte payload, preamble, sector offset + 4 bytes hash + 1 byte stop
        var packetlen = preamble.length + header.length + data.length + footer.length + stop.length;

        // now stripe the buffer to ensure transitions for baud sync
        // don't stripe the premable or the hash
        for (i = 0; i < data.length; i++) {
            if ((i % 16) == 3)
                data[i] ^= 0x55;
            else if ((i % 16) == 11)
                data[i] ^= 0xaa;
        }

        return this.makePacket(preamble, header, data, footer, stop);
    },
*/
    pub fn make_silence(&mut self, msecs: u32) -> Vec<i16> {
        let mut buffer: Vec<i16> = vec![];

        let silence_length = (self.rate / (1000.0 / msecs as f64)).ceil() as usize;
        buffer.resize(silence_length, 0);
        buffer
    }

    pub fn encode(&mut self, input: &Vec<u8>, output: &mut Vec<i16>) {
        let file_length = input.len();

        /* Note: Maximum of 65536 blocks */
        let blocks = ((file_length as f64 / 256.0).ceil()) as u16;

        let mut audio = self.make_silence(250);
        output.append(&mut audio);

        let data = self.make_control_packet(&input);
        let mut audio = self.modulator.modulate_pcm(&data);
        output.append(&mut audio);

        let mut audio = self.make_silence(100);
        output.append(&mut audio);

        // Make two header packets
        let data = self.make_control_packet(&input);
        let mut audio = self.modulator.modulate_pcm(&data);
        output.append(&mut audio);

        let mut audio = self.make_silence(500);
        output.append(&mut audio);

        for mut packet_num in 0..blocks {
            packet_num = packet_num & 0xff;
            let slice_start = packet_num * 256;
            let mut packet_data: Vec<u8> = vec![];
            for i in 0..256 {
                let target_offset = (slice_start + i) as usize;
                if target_offset < input.len() {
                    packet_data.push(input[target_offset]);
                }
                else {
                    packet_data.push(0xff);
                }
            }
            let data = self.make_data_packet(&packet_data, packet_num as u16);
            let mut audio = self.modulator.modulate_pcm(&data);
            output.append(&mut audio);

            let mut audio = self.make_silence(80);
            output.append(&mut audio);
        }

        let mut audio = self.make_silence(500);
        output.append(&mut audio);
    }
    /*
            var fileLen = data.length;
            var blocks = Math.ceil(fileLen / 256);
            var rawPcmData = [];

            var pcmPacket;
            this.tag = tag;

            // Additional padding to work around anti-pop hardware/software
            this.makeSilence(rawPcmData, 250);

            pcmPacket = this.modulator.modulatePcm(this.makeCtlPacket(array.subarray(0, fileLen)));
            for (var i = 0; i < pcmPacket.length; i++)
                rawPcmData.push(pcmPacket[i]);

            // Make silence here
            this.makeSilence(rawPcmData, 100);

            pcmPacket = this.modulator.modulatePcm(this.makeCtlPacket(array.subarray(0, fileLen)));
            for (var i = 0; i < pcmPacket.length; i++)
                rawPcmData.push(pcmPacket[i]);

            // More silence
            this.makeSilence(rawPcmData, 500);

            for (var block = 0; block < blocks; block++) {
                var start = block * 256;
                var end = start + 256;
                if (end > fileLen)
                end = fileLen;
                pcmPacket = this.modulator.modulatePcm(this.makeDataPacket(array.subarray(start, end), block));
                for (var i = 0; i < pcmPacket.length; i++)
                    rawPcmData.push(pcmPacket[i]);

                // Inter-packet silence
                this.makeSilence(rawPcmData, 80);
            }

            // Additional padding to work around anti-pop hardware/software
            this.makeSilence(rawPcmData, 250);

            this.playCount = 0;
            tag.pause();
            tag.onended = function() {
                // Play again if we haven't hit the limit'
                this.playCount++;
                if (this.playCount < this.maxPlays) {
                    tag.play();
                }
                else {
                    this.tag.onended = undefined;
                    if (this.endCallback)
                        this.endCallback();
                }
            }.bind(this);
            
            if (isMP3)
                this.transcodeMp3(rawPcmData, tag);
            else if (isWav)
                this.transcodeWav(rawPcmData, tag);
            this.pcmData = rawPcmData;
            tag.play();
        },
*/
}
/*
(function (window) {
    'use strict';

    var ModulationController = function(params) {

        if (!params)
            params = new Object();

        this.canvas = params.canvas || undefined;
        this.endCallback = params.endCallback || undefined;

        /* Are these needed? */
        this.saveState = false;

        this.isSending = false;
        this.playing = false;
        this.done = false;
        this.stoppedAt = 0;
        this.playCount = 0;
        this.maxPlays = 3;
        this.byteArray = null;
        this.rate = 44100;
        this.pcmData = null;

        this.PROT_VERSION = 0x01;   // Protocol v1.0

        this.CONTROL_PACKET = 0x01;
        this.DATA_PACKET = 0x02;

        this.modulator = new Modulator({
            rate: this.rate
        }); // the modulator object contains our window's audio context

        /* Preamble sent before every audio packet */
        this.preamble = [0x00, 0x00, 0x00, 0x00, 0xaa, 0x55, 0x42];

        /* Stop bits, sent to pad the end of transmission */
        this.stop_bytes = [0xff];
    }

    ModulationController.prototype = {

        makeControlHeader: function() {
            return [this.PROT_VERSION, this.CONTROL_PACKET, 0x00, 0x00];
        },

        makeDataHeader: function(blockNum) {
            return [this.PROT_VERSION, this.DATA_PACKET, blockNum & 0xff, (blockNum >> 8) & 0xff];
        },

        getPcmData: function() {
            return this.pcmData;
        },

        sendData: function(data) {
            if (data) {
                var dataLength = data.length;
                var array = new Uint8Array(new ArrayBuffer(dataLength));
                for (i = 0; i < dataLength; i++)
                    array[i] = data.charCodeAt(i);

                this.byteArray = array;
                this.playCount = 0;
                this.transcodePacket(0);
                this.isSending = true;
            }
        },

        // this is the core function for transcoding
        // two object variables must be set:
        // byteArray.
        // byteArray is the binary file to transmit

        // the parameter to this, "index", is a packet counter. We have to recursively call
        // transcodePacket using callbacks triggered by the completion of audio playback. I couldn't
        // think of any other way to do it.
        transcodePacket: function(index) {
            var fileLen = this.byteArray.length;
            var blocks = Math.ceil(fileLen / 256);
            var packet;

            // index 0 & 1 create identical control packets. We transmit the control packet
            // twice in the beginning because (a) it's tiny and almost free and (b) if we
            // happen to miss it, we waste an entire playback cycle before we start committing
            // data to memory
            if (index == 0  || index == 1) {
                packet = this.makeCtlPacket(this.byteArray.subarray(0, fileLen));
            }
            else {
                // data index starts at 2, due to two sends of the control packet up front
                var block = index - 2;
                var start = block * 256;
                var end = start + 256;
                if (end > fileLen)
                    end = fileLen;
                packet = this.makeDataPacket(this.byteArray.subarray(start, end), block);
            }

            this.silence = false;
            this.modulator.modulate(packet);
            this.modulator.playLoop(this, this.finishPacketPlayback, index + 1);
            this.modulator.drawWaveform(this.canvas);
        },

        transcodeToAudioTag: function(data, tag, type) {
            var isMP3 = (type.toLowerCase() === 'mp3');
            var isWav = (type.toLowerCase() == "wav");

            var array = new Uint8Array(data.length);
            for (i = 0; i < data.length; i++)
                array[i] = data.charCodeAt(i);

            var fileLen = data.length;
            var blocks = Math.ceil(fileLen / 256);
            var rawPcmData = [];

            var pcmPacket;
            this.tag = tag;

            // Additional padding to work around anti-pop hardware/software
            this.makeSilence(rawPcmData, 250);

            pcmPacket = this.modulator.modulatePcm(this.makeCtlPacket(array.subarray(0, fileLen)));
            for (var i = 0; i < pcmPacket.length; i++)
                rawPcmData.push(pcmPacket[i]);

            // Make silence here
            this.makeSilence(rawPcmData, 100);

            pcmPacket = this.modulator.modulatePcm(this.makeCtlPacket(array.subarray(0, fileLen)));
            for (var i = 0; i < pcmPacket.length; i++)
                rawPcmData.push(pcmPacket[i]);

            // More silence
            this.makeSilence(rawPcmData, 500);

            for (var block = 0; block < blocks; block++) {
                var start = block * 256;
                var end = start + 256;
                if (end > fileLen)
                end = fileLen;
                pcmPacket = this.modulator.modulatePcm(this.makeDataPacket(array.subarray(start, end), block));
                for (var i = 0; i < pcmPacket.length; i++)
                    rawPcmData.push(pcmPacket[i]);

                // Inter-packet silence
                this.makeSilence(rawPcmData, 80);
            }

            // Additional padding to work around anti-pop hardware/software
            this.makeSilence(rawPcmData, 250);

            this.playCount = 0;
            tag.pause();
            tag.onended = function() {
                // Play again if we haven't hit the limit'
                this.playCount++;
                if (this.playCount < this.maxPlays) {
                    tag.play();
                }
                else {
                    this.tag.onended = undefined;
                    if (this.endCallback)
                        this.endCallback();
                }
            }.bind(this);
            
            if (isMP3)
                this.transcodeMp3(rawPcmData, tag);
            else if (isWav)
                this.transcodeWav(rawPcmData, tag);
            this.pcmData = rawPcmData;
            tag.play();
        },

        transcodeWav: function(samples, tag) {

            var pcmData = [];//new Uint8Array(new ArrayBuffer(samples.length * 2));
            for (var i = 0; i < samples.length; i++) {
                
                // Convert from 16-bit PCM to two's compliment 8-bit buffers'
                var sample = samples[i];

                // Javascript doesn't really do two's compliment
                if (sample < 0)
                    sample = (0xffff - ~sample);

                pcmData.push(Math.round(sample) & 0xff);
                pcmData.push(Math.round(sample >> 8) & 0xff);
            }

            var pcmObj = new pcm({
                channels: 1,
                rate: this.rate,
                depth: 16
            }).toWav(pcmData);
            tag.src = pcmObj.encode();
        },

        makeSilence: function(buffer, msecs) {
            var silenceLen = Math.ceil(this.rate / (1000.0 / msecs));
            for (var i = 0; i < silenceLen; i++)
                buffer.push(0);
        },

        finishPacketPlayback: function(index) {

            if (!this.isSending)
                return false;

            // If "silence" is false, then we just played a data packet.  Play silence now.
            if (this.silence == false) {
                this.silence = true;

                if (index == 1)
                    this.modulator.silence(100); // redundant send of control packet
                else if (index == 2)
                    this.modulator.silence(500); // 0.5s for bulk flash erase to complete
                else
                    this.modulator.silence(80); // slight pause between packets to allow burning
                this.modulator.playLoop(this, this.finishPacketPlayback, index);
                return true;
            }

            if (((index - 2) * 256) < this.byteArray.length) {
                // if we've got more data, transcode and loop
                this.transcodePacket(index);
                return true;
            }
            else {
                // if we've reached the end of our data, check to see how
                // many times we've played the entire file back. We want to play
                // it back a couple of times because sometimes packets get
                // lost or corrupted.
                if (this.playCount < 2) { // set this higher for more loops!
                    this.playCount++;
                    this.transcodePacket(0); // start it over!
                    return true;
                }
                else {
                    this.audioEndCB(); // clean up the UI when done
                    return false;
                }
            }

        },

        makeUint32: function(num) {
            return [num & 0xff,
                   (num >> 8) & 0xff,
                   (num >> 16) & 0xff,
                   (num >> 24) & 0xff];
        },

        makeUint16: function(num) {
            return [num & 0xff,
                   (num >> 8) & 0xff];
        },

        /* Appends "src" to "dst", beginning at offset "offset".
         * Handy for populating data buffers.
         */
        appendData: function(dst, src, offset) {
            var i;
            for (i = 0; i < src.length; i++)
                dst[offset + i] = src[i];
            return i;
        },

        makeHash: function(data, hash) {
            return this.makeUint32(murmurhash3_32_gc(data, hash));
        },

        makeFooter: function(packet) {
            var hash = 0xdeadbeef;
            var data = new Array();
            var i;
            var j;

            // Join all argument arrays together into "data"
            for (i = 0; i < arguments.length; i++)
                for (j = 0; j < arguments[i].length; j++)
                    data.push(arguments[i][j]);

            return this.makeHash(data, hash);
        },

        makePacket: function() {
            var len = 0;
            var i;
            for (i = 0; i < arguments.length; i++)
                len += arguments[i].length;

            var pkt = new Uint8Array(len);
            var offset = 0;

            for (i = 0; i < arguments.length; i++)
                offset += this.appendData(pkt, arguments[i], offset);

            return pkt;
        },

        makeCtlPacket: function(data) {
            // parameters from microcontroller spec. Probably a better way
            // to do this in javascript, but I don't know how (seems like "const" could be used, but not universal)
            var preamble = this.preamble;
            var header = this.makeControlHeader();
            var program_length = this.makeUint32(data.length);
            var program_hash = this.makeHash(data, 0x32d0babe);  // 0x32d0babe by convention
            var program_guid_str = SparkMD5.hash(String.fromCharCode.apply(null,data), false);
            var program_guid = [];
            var i;
            for (i = 0; i < program_guid_str.length-1; i += 2)
                program_guid.push(parseInt(program_guid_str.substr(i,2),16));

            var footer = this.makeFooter(header, program_length, program_hash, program_guid);
            var stop = this.stop_bytes;

            return this.makePacket(preamble, header, program_length, program_hash, program_guid, footer, stop);
        },

        makeDataPacket: function(dataIn, blocknum) {
            var i;

            // now assemble the packet
            var preamble = this.preamble;
            var header = this.makeDataHeader(blocknum);

            // Ensure the "data" payload is 256 bytes long.
            var data = new Uint8Array(256);
            for (i = 0; i < data.length; i++) data[i] = 0xff; // data.fill(0xff)
            this.appendData(data, dataIn, 0);

            var footer = this.makeFooter(header, data);
            var stop = this.stop_bytes;

            // 256 byte payload, preamble, sector offset + 4 bytes hash + 1 byte stop
            var packetlen = preamble.length + header.length + data.length + footer.length + stop.length;

            // now stripe the buffer to ensure transitions for baud sync
            // don't stripe the premable or the hash
            for (i = 0; i < data.length; i++) {
                if ((i % 16) == 3)
                    data[i] ^= 0x55;
                else if ((i % 16) == 11)
                    data[i] ^= 0xaa;
            }

            return this.makePacket(preamble, header, data, footer, stop);
        },

        // once all audio is done playing, call this to reset UI elements to idle state
        audioEndCB: function() {
            this.isSending = false;
        },

        stop: function() {
            this.isSending = false;
        },

        isRunning: function() {
            return this.isSending;
        }
    }

    /* Set up the constructor, so we can do "new ModulationController()" */
    window.ModulationController = function(params) {
        return new ModulationController(params);
    };
}(this));
*/