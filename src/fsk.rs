use std;
use std::f64;

pub struct FskEncoder {
    f_hi: u32,
    f_lo: u32,
    baud_rate: u32,
    sample_rate: u32,

    baud_frac: f64,
    baud_incr: f64,
    phase: f64,
    omega_lo: f64,
    omega_hi: f64,

    current_bit: u8,
    current_byte: u8,
    bit_pos: u32,
    data_pos: usize,
}

impl FskEncoder {

    pub fn new(f_hi: u32, f_lo: u32, baud_rate: u32, sample_rate: u32) -> FskEncoder {
        FskEncoder {
            f_hi: f_hi,
            f_lo: f_lo,
            baud_rate: baud_rate,
            sample_rate: sample_rate,

            phase: 0.0,
            omega_lo: (2.0 * std::f64::consts::PI * f_lo as f64) / sample_rate as f64,
            omega_hi: (2.0 * std::f64::consts::PI * f_hi as f64) / sample_rate as f64,
            baud_frac: 0.0,
            baud_incr: baud_rate as f64 / sample_rate as f64,

            current_bit: 0,
            current_byte: 0,
            bit_pos: 0,
            data_pos: 0,
        }
    }

    pub fn samples_per_bit(self: FskEncoder) -> f64 {
        self.sample_rate as f64 / self.baud_rate as f64
    }

    // does what you think it does -- input data should be uint8 array, outputdata is floats
    pub fn modulate(&mut self, input: Vec<u8>) -> Vec<f64> {
        let mut output: Vec<f64> = Vec::new();
//        for(var i = 0; i < outputData.length; i++) {
        loop {
//            this.state.baud_frac += this.state.baud_incr;
            self.baud_frac = self.baud_frac + self.baud_incr;
//            if( this.state.baud_frac >= 1) {
            if self.baud_frac >= 1.0 {
//                this.state.baud_frac -= 1;
                self.baud_frac -= 1.0;
//                if( this.state.bitpos == 0 ) {
                if self.bit_pos == 0 {
//                    if( this.state.datapos <= inputData.length ) {
                    if self.data_pos <= input.len() {
//                        this.state.curbyte = inputData[this.state.datapos++];
                        self.current_byte = input[self.data_pos];
                        self.data_pos = self.data_pos + 1;
//                        this.state.bitpos = 8;
                        self.bit_pos = 8;
                    } else {
//                        return outputData;
                        break;
                    }
                }
//                this.state.current_bit = this.state.curbyte & 1;
                self.current_bit = self.current_byte & 1;
//                this.state.curbyte >>= 1;
                self.current_byte = self.current_byte >> 1;
//                this.state.bitpos--;
            }
//            outputData[i] = Math.cos(this.state.phase);
            output.push(self.phase.cos());
//            if( this.state.current_bit == 0) {
            if self.current_bit == 0 {
//                this.state.phase += this.state.omega_lo;
                self.phase = self.phase + self.omega_lo;
            } else {
//                this.state.phase += this.state.omega_hi;
                self.phase = self.phase + self.omega_hi;
            }
        }

//        this.state.datapos = 0;
        self.data_pos = 0;
//        return outputData;
        output
    }

}
/*
function FskEncoder(sampleRate) {
}

FskEncoder.prototype = {
    f_lo: 8666,
    f_hi: 12500,
    baud_rate: 8000,
    sample_rate: 0, // comes from calling function based on browser/computer config
    samplesPerBit: 0.0,

    state : {
    },

    PHASE_BITS: 16,
    PHASE_BASE: (1 << 16), // hardcode PHASE_BITS here b/c javascript can't reference initializers in initializers

    // compute samples per bit. Needed to allocate audio buffers before modulating
    samplesPerBit: function() {
        return  this.sample_rate / this.baud_rate; // Not rounded! Floating point!
    },

    // for debug.
    dumpBuffer: function(buf) {
        var out = "";
        for (var i = 0; i < buf.length; i++)
            out += "0x" + buf[i].toString(16) + ",";
        return out;
    },

    // does what you think it does -- input data should be uint8 array, outputdata is floats
    modulate: function(inputData, outputData) {
        for(var i = 0; i < outputData.length; i++) {
            this.state.baud_frac += this.state.baud_incr;
            if( this.state.baud_frac >= 1) {
                this.state.baud_frac -= 1;
                if( this.state.bitpos == 0 ) {
                    if( this.state.datapos <= inputData.length ) {
                        this.state.curbyte = inputData[this.state.datapos++];
                        this.state.bitpos = 8;
                    } else {
                        return outputData;
                    }
                }
                this.state.current_bit = this.state.curbyte & 1;
                this.state.curbyte >>= 1;
                this.state.bitpos--;
            }
            outputData[i] = Math.cos(this.state.phase);
            if( this.state.current_bit == 0) {
                this.state.phase += this.state.omega_lo;
            } else {
                this.state.phase += this.state.omega_hi;
            }
        }

        this.state.datapos = 0;
        return outputData;
    }
};
*/