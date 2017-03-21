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
    pub fn modulate_pcm(&mut self, input: &Vec<u8>) -> Vec<i16> {

        let modulated = self.encoder.modulate(input);
        let mut output: Vec<i16> = Vec::new();

        for sample in modulated {
            // Map -1 .. 1 to -32767 .. 32768
            output.push((sample * 32767.0).round() as i16);
        }

        output
    }
}