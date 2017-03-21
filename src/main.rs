mod fsk;
mod modulator;
mod controller;
mod wav;
extern crate elf;

extern crate clap;
use clap::{Arg, App};

use std::io::prelude::*;
use std::fs::File;

fn do_modulation(source_filename: &str,
                 target_filename: &str,
                 data_rate: u32,
                 os_update: bool,
                 version: controller::ProtocolVersion)
                 -> std::io::Result<()> {

    let sample_rate = match data_rate {
        0 => 44100.0 * 4.0,
        1 => 44100.0 * 2.0,
        2 => 44100.0 * 1.0,
        r => panic!("Unrecognized data rate: {}", r),
    };
    let mut controller = controller::Controller::new(sample_rate, os_update, version);

    let input_data = match elf::File::open_path(source_filename) {
        Ok(e) => {
            println!("opened ELF file: {}", e.ehdr);
            if e.ehdr.machine != elf::types::EM_ARM {
                panic!("ELF file detected, but not for ARM");
            }
            if e.ehdr.class != elf::types::ELFCLASS32 {
                panic!("ELF file must contain 32-bit code");
            }
            if e.ehdr.data != elf::types::ELFDATA2LSB {
                panic!("ELF file must be little endian");
            }

            let mut data = vec![];
            for section in e.sections {
                // It's unclear what exactly should be included,
                // but this seems to produce the correct output.
                if section.shdr.shtype == elf::types::SHT_PROGBITS &&
                   section.shdr.flags != elf::types::SHF_NONE &&
                   section.shdr.addr != 0 {
                    data.extend(section.data);
                }
            }
            data
        }
        Err(_) => {
            let mut input = try!(File::open(source_filename));
            let mut input_data: Vec<u8> = vec![];
            try!(input.read_to_end(&mut input_data));
            input_data
        }
    };
    let mut audio_data: Vec<i16> = vec![];

    controller.encode(&input_data, &mut audio_data, data_rate);
    let mut pilot_controller = controller::Controller::new(44100.0, os_update, stripe);
    pilot_controller.pilot(&mut audio_data, data_rate);
    controller.encode(&input_data, &mut audio_data, data_rate);

    try!(wav::write_wav(44100, &audio_data, target_filename));
    Ok(())
}

fn main() {
    let matches = App::new("Love-to-Code Program Modulator")
        .version("1.1")
        .author("Sean Cross <sean@xobs.io>")
        .about("Takes compiled code and modulates it for a Love-to-Code sticker")
        .arg(Arg::with_name("input")
            .short("i")
            .long("input")
            .value_name("FILENAME")
            .help("Name of the input file")
            .takes_value(true)
            .required(true))
        .arg(Arg::with_name("output")
            .short("o")
            .long("output")
            .value_name("FILENAME")
            .help("Name of the wave file to write to")
            .required(true))
        .arg(Arg::with_name("version")
            .short("s")
            .long("protocol-version")
            .value_name("VERSION")
            .takes_value(true)
            .possible_values(&["1", "2"])
            .default_value("2")
            .help("Data protocol version"))
        .arg(Arg::with_name("update")
            .short("u")
            .long("update")
            .takes_value(false)
            .help("Generate an OS update waveform"))
        .arg(Arg::with_name("rate")
            .short("r")
            .long("rate")
            .possible_values(&["high", "mid", "low"])
            .value_name("RATE")
            .takes_value(true)
            .default_value("high")
            .help("Audio encoding rate"))
        .get_matches();

    let source_filename = matches.value_of("input").unwrap();
    let target_filename = matches.value_of("output").unwrap();
    let os_update = matches.is_present("update");
    let protocol_version = match matches.value_of("version") {
        Some("1") => controller::ProtocolVersion::V1,
        Some("2") => controller::ProtocolVersion::V2,
        Some(x) => panic!("Unrecognized version found: {}", x),
        None => panic!("No protocol version specified"),
    };
    let data_rate = match matches.value_of("rate") {
        Some("low") => 0,
        Some("mid") => 1,
        Some("high") => 2,
        Some(x) => panic!("Unrecognized rate found: {}", x),
        None => panic!("No valid rate specified"),
    };

    println!("Modulating {} into {}.", source_filename, target_filename);
    println!("Is update? {}  Data rate: {}  Protocol version: {:?}",
             os_update,
             data_rate,
             protocol_version);

    if let Err(err) = do_modulation(source_filename,
                                    target_filename,
                                    data_rate,
                                    os_update,
                                    protocol_version) {
        println!("Unable to modulate: {}", &err);
        std::process::exit(1);
    }
}
