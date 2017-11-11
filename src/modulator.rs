use fsk;

pub struct Modulator {
    encoder: fsk::FskEncoder,
}

impl Modulator {
    pub fn new(rate: f64) -> Modulator {
        let modulator = Modulator { encoder: fsk::FskEncoder::new(8666.0, 12500.0, 8000.0, rate) };

        modulator
    }

    // Modulate an array of 8-bit bytes into an array of signed 16-bit PCM samples
    pub fn modulate_pcm(&mut self, input: &Vec<u8>) -> Vec<f64> {

        self.encoder.modulate(input)
    }
}