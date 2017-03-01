mod fsk;
mod modulator;
mod controller;
mod wav;
extern crate elf;

extern crate clap;
use clap::{Arg, App};

use std::io::prelude::*;
use std::fs::File;

fn do_modulation(source_filename: &str, target_filename: &str, os_update: bool) -> std::io::Result<()> {
    let mut controller = controller::Controller::new(44100.0, os_update);

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
                if section.shdr.shtype == elf::types::SHT_PROGBITS
                && section.shdr.flags != elf::types::SHF_NONE 
                && section.shdr.addr != 0 {
                    data.extend(section.data);
                }
            }
            data
        },
        Err(_) => {
            let mut input = try!(File::open(source_filename));
            let mut input_data: Vec<u8> = vec![];
            try!(input.read_to_end(&mut input_data));
            input_data
        }
    };
    let mut audio_data: Vec<i16> = vec![];

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
                        .arg(Arg::with_name("UPDATE")
                                .short("u")
                                .long("update")
                                .help("Generate an OS update waveform")
                        )
                        .get_matches();

    let source_filename = matches.value_of("INPUT").unwrap();
    let target_filename = matches.value_of("OUTPUT").unwrap();
    let os_update = matches.value_of("UPDATE").is_some();
    let res = do_modulation(source_filename, target_filename, os_update);
    if res.is_err() {
        let err = res.err().unwrap();
        println!("Unable to modulate: {}", &err);
        std::process::exit(1);
    }
}
