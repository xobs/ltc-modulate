use fsk;

pub struct Modulator {
    encoder: fsk::FskEncoder,
}

impl Modulator {
    pub fn new(sample_rate: f64, baud_rate: f64, f_lo: f64, f_hi: f64) -> Modulator {
        Modulator { encoder: fsk::FskEncoder::new(f_lo, f_hi, baud_rate, sample_rate) }
    }

    // Modulate an array of 8-bit bytes into an array of signed 16-bit PCM samples
    pub fn modulate_pcm(&mut self, input: &[u8]) -> Vec<f64> {
        self.encoder.modulate(input)
    }
}
