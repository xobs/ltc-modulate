mod fsk;
mod modulator;
mod controller;
mod wav;

extern crate clap;
use clap::{Arg, App};

use std::io::prelude::*;
use std::fs::File;

fn do_modulation(source_filename: &str, target_filename: &str) -> std::io::Result<()> {
    let mut controller = controller::Controller::new(44100.0);
    let mut input = try!(File::open(source_filename));
    let mut input_data: Vec<u8> = vec![];
    let mut audio_data: Vec<i16> = vec![];

    try!(input.read_to_end(&mut input_data));
    controller.encode(&input_data, &mut audio_data);

    try!(wav::write_wav(44100, &audio_data, target_filename));
    Ok(())
}

fn main() {
    let matches = App::new("Love-to-Code Program Modulator")
                        .version("1.0")
                        .author("Sean Cross <sean@xobs.io>")
                        .about("Takes compiled code and modulates it for a Love-to-Code sticker")
                        .arg(Arg::with_name("INPUT")
                                .short("i")
                                .long("input")
                                .value_name("FILENAME")
                                .help("Name of the input file")
                                .takes_value(true)
                                .required(true)
                        )
                        .arg(Arg::with_name("OUTPUT")
                                .short("o")
                                .long("output")
                                .value_name("FILENAME")
                                .help("Name of the wave file to write to")
                                .required(true)
                        )
                        .get_matches();

    let source_filename = matches.value_of("INPUT").unwrap();
    let target_filename = matches.value_of("OUTPUT").unwrap();
    let res = do_modulation(source_filename, target_filename);
    if res.is_err() {
        let err = res.err().unwrap();
        println!("Unable to modulate: {}", &err);
        std::process::exit(1);
    }
}
