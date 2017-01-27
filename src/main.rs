mod fsk;
mod modulator;
mod controller;
mod wav;

use std::io::prelude::*;
use std::fs::File;

fn do_modulation(filename: &str) -> std::io::Result<()> {
    let mut controller = controller::Controller::new(44100.0);
    let mut input = try!(File::open(filename));
    let mut input_data: Vec<u8> = vec![];
    let mut audio_data: Vec<i16> = vec![];

    try!(input.read_to_end(&mut input_data));
    controller.encode(&input_data, &mut audio_data);

    try!(wav::write_wav(44100, &audio_data, "output.wav"));
    Ok(())
}

fn main() {
    println!("Hello, world!");
    let res = do_modulation("test.bin");
    if res.is_err() {
        println!("Unable to modulate: {:?}", res);
        panic!(res);
    }
}
