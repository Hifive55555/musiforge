use std::sync::{Arc, Mutex};
use std::thread;
use std::error::Error;
use std::io::{stdin, stdout, Write};
use log::debug;
use cpal::traits::StreamTrait;
use midir::{Ignore, MidiIO, MidiInput, MidiOutput};

use musiforge::musiblock::Piano;
use musiforge::init_logger;
use musiforge::config::stream_setup_for;

fn main() -> Result<(), Box<dyn Error>> {
    init_logger();

    // 加载 Piano, (Piano 需要在多个线程中使用，out_device 线程、监听 midi 线程，故需要用 Arc<Mutex>)
    let piano = Arc::new(Mutex::new(Piano::new()));
    connect_out_device(piano.clone());

    // 实时设备连接
    let mut midi_in = MidiInput::new("midir forwarding input")?;
    midi_in.ignore(Ignore::None);

    let in_port = select_port(&midi_in, "input")?;
    let in_port_name = midi_in.port_name(&in_port)?;
    println!("\nOpening connections {}", in_port_name);

    // _conn_in needs to be a named parameter, because it needs to be kept alive until the end of the scope
    let _conn_in = midi_in.connect(
        &in_port,
        "midir-forward",
        move |stamp, message, _| {
            piano.lock().unwrap().handle_midi_message(message);
            debug!("{}: {:?} (len = {})", stamp, message, message.len());
        },
        (),
    )?;

    let mut input = String::new();
    stdin().read_line(&mut input)?; // wait for next enter key press

    println!("Closing connections");
    Ok(())

    // handle_thread.join().unwrap();
}

fn select_port<T: MidiIO>(midi_io: &T, descr: &str) -> Result<T::Port, Box<dyn Error>> {
    println!("Available {} ports:", descr);
    let midi_ports = midi_io.ports();
    for (i, p) in midi_ports.iter().enumerate() {
        println!("{}: {}", i, midi_io.port_name(p)?);
    }
    print!("Please select {} port: ", descr);
    stdout().flush()?;
    let mut input = String::new();
    stdin().read_line(&mut input)?;
    let port = midi_ports
        .get(input.trim().parse::<usize>()?)
        .ok_or("Invalid port number")?;
    Ok(port.clone())
}

fn connect_out_device(p: Arc<Mutex<Piano>>) {
    thread::spawn(move || -> anyhow::Result<()> {
        let p = p.clone();
        let stream = stream_setup_for(
            move |data: &mut [f32], num_channels, time| {
            for frame in data.chunks_mut(num_channels) {
                let value = p.lock().unwrap().tick();
                for sample in frame.iter_mut() {
                    *sample = value;
                }
            }
        }).unwrap();
        stream.play().unwrap();
        // std::thread::sleep(std::time::Duration::from_millis(8000));
        std::thread::park();
        Ok(())
    });
}
