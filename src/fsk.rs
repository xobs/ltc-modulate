use std;
use std::f64;

pub struct FskEncoder {
    baud_frac: f64,
    baud_incr: f64,
    phase: f64,
    omega_lo: f64,
    omega_hi: f64,

    current_bit: u8,
    current_byte: u8,
    bit_pos: u32,
    data_pos: usize,

    sample_rate: f64,
    baud_rate: f64,
}

impl FskEncoder {

    pub fn new(f_lo: f64, f_hi: f64, baud_rate: f64, sample_rate: f64) -> FskEncoder {
        FskEncoder {
            sample_rate,
            baud_rate,

            phase: 0.0,
            omega_lo: (2.0 * std::f64::consts::PI * f_lo) / sample_rate,
            omega_hi: (2.0 * std::f64::consts::PI * f_hi) / sample_rate,
            baud_frac: 0.0,
            baud_incr: baud_rate / sample_rate,

            current_bit: 0,
            current_byte: 0,
            bit_pos: 0,
            data_pos: 0,
        }
    }

    // does what you think it does -- input data should be uint8 array, outputdata is floats
    pub fn modulate(&mut self, input: &[u8]) -> Vec<f64> {
        let mut output: Vec<f64> = Vec::new();
        self.data_pos = 0;

        /* We keep these values the same between runs */
        /*
        self.bit_pos = 0;
        self.current_byte = 0;
        self.baud_frac = 0.0;
        */
        output.reserve(8 * input.len() as usize * self.sample_rate as usize / self.baud_rate as usize);

        loop {
            self.baud_frac += self.baud_incr;
            if self.baud_frac >= 1.0 {
                self.baud_frac -= 1.0;
                assert!(self.baud_frac < 1.0);
                if self.bit_pos == 0 {
                    if self.data_pos < input.len() {
                        self.current_byte = input[self.data_pos];
                        self.data_pos += 1;
                        self.bit_pos = 8;
                    } else {
                        return output;
                    }
                }
                self.current_bit = self.current_byte & 1;
                self.current_byte >>= 1;
                self.bit_pos -= 1;
            }
            output.push(self.phase.cos());
            if self.current_bit == 0 {
                self.phase += self.omega_lo;
            } else {
                self.phase += self.omega_hi;
            }
        }
    }
}
