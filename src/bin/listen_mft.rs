extern crate serde_json;
use clap::{App, Arg};
use std::process::exit;
use std::io::Write;
use std::thread;
use std::fs::File;
use std::sync::mpsc;
use std::sync::mpsc::{Sender, Receiver};
use rusty_usn::record::UsnEntry;
use rswinthings::file::pipe::create_pipe;
use rswinthings::utils::json::get_difference_value;
use rswinthings::utils::debug::set_debug_level;
use rswinthings::mft::EntryListener;
use rswinthings::usn::listener::UsnVolumeListener;
use rswinthings::file::FileHandle;

static VERSION: &'static str = "0.2.0";


fn make_app<'a, 'b>() -> App<'a, 'b> {
    let format = Arg::with_name("file")
        .short("-f")
        .long("file")
        .value_name("FILE")
        .takes_value(true)
        .help("The file to difference.");

    let namedpipe = Arg::with_name("named_pipe")
        .long("named_pipe")
        .value_name("NAMEDPIPE")
        .takes_value(true)
        .help("The named pipe to write out to.");

    let debug = Arg::with_name("debug")
        .short("-d")
        .long("debug")
        .value_name("DEBUG")
        .takes_value(true)
        .possible_values(&["Off", "Error", "Warn", "Info", "Debug", "Trace"])
        .help("Debug level to use.");

    App::new("listen_mft")
        .version(VERSION)
        .author("Matthew Seyer <https://github.com/forensicmatt/RsWindowsThingies>")
        .about("See the differences in MFT attirbues.")
        .arg(format)
        .arg(namedpipe)
        .arg(debug)
}


fn run(mut listener: EntryListener, mut named_pipe_opt: Option<File>) {
    let (tx, rx): (Sender<UsnEntry>, Receiver<UsnEntry>) = mpsc::channel();

    let mut previous_value = listener.get_current_value().expect("Unable to get current mft entry value");
    match named_pipe_opt {
        Some(ref mut fh) => {
            fh.write(&previous_value.to_string().into_bytes());
        },
        None => {
            println!("{}", previous_value.to_string());
        }
    }

    let volume_str = listener.get_volume_string().expect("Error getting volume path.");
    let usn_volume_listener = UsnVolumeListener::new(
        volume_str,
        false,
        tx.clone()
    );

    let _thread = thread::spawn(move || {
        usn_volume_listener.listen_to_volume(None)
    });

    loop {
        let usn_entry = match rx.recv() {
            Ok(e) => e,
            Err(_) => panic!("Disconnected!"),
        };

        let file_ref = usn_entry.record.get_file_reference();

        if file_ref.entry != listener.entry_to_monitor as u64 {
            continue;
        }

        let current_value = listener.get_current_value().expect("Unable to get current mft entry value");

        let difference_value = get_difference_value(
            &previous_value,
            &current_value
        );

        match difference_value.as_object() {
            None => continue,
            Some(o) => {
                if o.is_empty() {
                    continue;
                }

                let value_str = serde_json::to_string_pretty(
                    &difference_value
                ).expect("Unable to format Value");
                
                match named_pipe_opt {
                    Some(ref mut fh) => {
                        fh.write(&format!("{}", value_str).into_bytes());
                    },
                    None => {
                        println!("{}", value_str);
                    }
                }
        
                previous_value = current_value.to_owned();
            }
        }
    }
}


fn main() {
    let app = make_app();
    let options = app.get_matches();

    // Set debug
    match options.value_of("debug") {
        Some(d) => set_debug_level(d).expect(
            "Error setting debug level"
        ),
        None => {}
    }

    let file_path = match options.value_of("file") {
        Some(p) => p,
        None => {
            eprintln!("file parameter was expected.");
            exit(-1);
        }
    };

    let named_pipe = match options.value_of("named_pipe") {
        Some(p) => {
            Some(
                create_pipe(p).expect("blahh")
            )
        },
        None => None
    };

    let listener = EntryListener::new(
        file_path
    ).expect("Error creating EntryListener");

    run(listener, named_pipe);
}