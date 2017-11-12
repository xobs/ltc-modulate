mod fsk;
mod modulator;
mod controller;
mod wav;
extern crate cpal;
extern crate elf;

extern crate clap;
use clap::{App, Arg};

use std::fs::File;
use std::io::prelude::*;
use std::sync::{Arc, Mutex, Condvar};
use std::thread;

const DEFAULT_SAMPLE_RATE: f64 = 44100.0;

fn do_modulation(
    source_filename: &str,
    target_filename: &str,
    data_rate: u32,
    os_update: bool,
    version: controller::ProtocolVersion,
    play_file: bool,
    repeats: u32,
) -> std::io::Result<()> {
    let mut output_sample_rate = DEFAULT_SAMPLE_RATE;

    if play_file {
        let endpoint = cpal::default_endpoint().expect("Failed to get default endpoint");
        let format = endpoint
            .supported_formats()
            .unwrap()
            .next()
            .expect("Failed to get endpoint format")
            .with_max_samples_rate();
        output_sample_rate = format.samples_rate.0 as f64;
    }

    let sample_rate = match data_rate {
        0 => output_sample_rate * 4.0,
        1 => output_sample_rate * 2.0,
        2 => output_sample_rate * 1.0,
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
                if section.shdr.shtype == elf::types::SHT_PROGBITS
                    && section.shdr.flags != elf::types::SHF_NONE
                    && section.shdr.addr != 0
                {
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
    let mut audio_data: Vec<f64> = vec![];

    for _ in 0..repeats {
        controller.encode(&input_data, &mut audio_data, data_rate);
        let mut pilot_controller = controller::Controller::new(output_sample_rate, os_update, version);
        pilot_controller.pilot(&mut audio_data, data_rate);
    }

    if play_file {
        let file_end_signal = Arc::new((Mutex::new(0), Condvar::new()));

        let endpoint = cpal::default_endpoint().expect("Failed to get default endpoint");
        let format = endpoint
            .supported_formats()
            .unwrap()
            .next()
            .expect("Failed to get endpoint format")
            .with_max_samples_rate();
        println!("Format selected: {:?}", format);

        let event_loop = cpal::EventLoop::new();
        let voice_id = event_loop.build_voice(&endpoint, &format).unwrap();
        event_loop.play(voice_id);

        let thread_mutex = file_end_signal.clone();
        let audio_data_len = audio_data.len();

        thread::spawn(move || {
            // Produce a sinusoid of maximum amplitude.
            let next_value = || {
                let &(ref num, ref _cvar) = &*thread_mutex;
                {
                    let mut audio_data_pos = num.lock().unwrap();
                    if *audio_data_pos >= audio_data.len() {
                        0.0 as f32
                    } else {
                        let val = audio_data[*audio_data_pos];
                        *audio_data_pos = *audio_data_pos + 1;
                        //cvar.notify_one();
                        val as f32
                    }
                }
            };

            event_loop.run(move |_, buffer| {
                match buffer {
                    cpal::UnknownTypeBuffer::U16(mut buffer) => {
                        for sample in buffer.chunks_mut(format.channels.len()) {
                            let value = ((next_value() * 0.5 + 0.5) * std::u16::MAX as f32) as u16;
                            for out in sample.iter_mut() {
                                *out = value;
                            }
                        }
                    }

                    cpal::UnknownTypeBuffer::I16(mut buffer) => {
                        for sample in buffer.chunks_mut(format.channels.len()) {
                            let value = (next_value() * std::i16::MAX as f32) as i16;
                            for out in sample.iter_mut() {
                                *out = value;
                            }
                        }
                    }

                    cpal::UnknownTypeBuffer::F32(mut buffer) => {
                        for sample in buffer.chunks_mut(format.channels.len()) {
                            let value = next_value();
                            for out in sample.iter_mut() {
                                *out = value;
                            }
                        }
                    }
                };
            });
        });
        println!("Thread is now playing");

        loop {
            {
                let &(ref num, ref _cvar) = &*file_end_signal;
                let offset = num.lock().unwrap();
                if *offset >= audio_data_len {
                    use std::process;
                    process::exit(0);
                }
            }
            use std::time::Duration;
            thread::park_timeout(Duration::from_millis(250));
        }
    } else {
        let mut output: Vec<i16> = Vec::new();
        for sample in audio_data {
            // Map -1 .. 1 to -32767 .. 32768
            output.push((sample * 32767.0).round() as i16);
        }

        try!(wav::write_wav(
            output_sample_rate as u32,
            &output,
            target_filename
        ));
    }
    Ok(())
}

fn main() {
    let matches = App::new("Love-to-Code Program Modulator")
        .version("1.3")
        .author("Sean Cross <sean@xobs.io>")
        .about("Takes compiled code and modulates it for a Love-to-Code sticker")
        .arg(
            Arg::with_name("input")
                .short("i")
                .long("input")
                .value_name("FILENAME")
                .help("Name of the input file")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("output")
                .short("o")
                .long("output")
                .value_name("FILENAME")
                .help("Name of the wave file to write to"),
        )
        .arg(
            Arg::with_name("play")
                .short("w")
                .long("play")
                .help("Play wave audio out the default sound device"),
        )
        .arg(
            Arg::with_name("version")
                .short("p")
                .long("protocol-version")
                .value_name("VERSION")
                .takes_value(true)
                .possible_values(&["1", "2"])
                .default_value("2")
                .help("Data protocol version"),
        )
        .arg(
            Arg::with_name("repeats")
                .short("c")
                .long("repeat-count")
                .value_name("COUNT")
                .takes_value(true)
                .default_value("3")
                .help("Number of times to repeat"),
        )        .arg(
            Arg::with_name("update")
                .short("u")
                .long("update")
                .takes_value(false)
                .help("Generate an OS update waveform"),
        )
        .arg(
            Arg::with_name("rate")
                .short("r")
                .long("rate")
                .possible_values(&["high", "mid", "low"])
                .value_name("RATE")
                .takes_value(true)
                .default_value("high")
                .help("Audio encoding rate"),
        )
        .get_matches();

    let source_filename = matches.value_of("input").unwrap();
    let target_filename = matches.value_of("output").unwrap_or("output.wav");
    let os_update = matches.is_present("update");
    let play_file = matches.is_present("play");
    let repeats = matches.value_of("repeats").unwrap().parse::<u32>().unwrap();
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
    println!(
        "Is update? {}  Data rate: {}  Protocol version: {:?}",
        os_update,
        data_rate,
        protocol_version
    );

    if let Err(err) = do_modulation(
        source_filename,
        target_filename,
        data_rate,
        os_update,
        protocol_version,
        play_file,
        repeats,
    ) {
        println!("Unable to modulate: {}", &err);
        std::process::exit(1);
    }
}
