use std::{
    error::Error,
    io::stdin,
    sync::{Arc, Mutex},
};

use musiforge::{
    create_stream, init_logger,
    musiblock::{listen, AdditiveSynth, AdditiveUnit, MidiRack},
    ClockTime,
};

const SAMPLE_RATE: u32 = 48000;

fn create_graph_flow() -> impl FnMut() -> f32 {
    let additive_synth = Arc::new(Mutex::new(AdditiveSynth::new(5, SAMPLE_RATE as f32)));
    let synth_rack = Arc::new(Mutex::new(MidiRack::new(additive_synth)));
    let synth_rack_2 = Arc::clone(&synth_rack);
    std::thread::spawn(move || {
        listen::<SAMPLE_RATE, AdditiveSynth, AdditiveUnit>(synth_rack_2).unwrap();
    });

    move || -> f32 { synth_rack.lock().unwrap().tick() }
}

fn data_callback() -> impl FnMut(&mut [f32], &cpal::OutputCallbackInfo) + Send + 'static {
    let mut global_clock: ClockTime<SAMPLE_RATE> = ClockTime::new();
    let mut graph_flow = create_graph_flow();

    move |data: &mut [f32], _info: &cpal::OutputCallbackInfo| {
        for frame in data.chunks_mut(2) {
            global_clock.tick();
            let vol = graph_flow();
            for sample in frame.iter_mut() {
                *sample = vol;
            }
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    init_logger();

    let gf = create_stream(1024, data_callback());
    gf();

    let mut input = String::new();
    stdin().read_line(&mut input)?; // wait for next enter key press

    println!("Closing connections");
    Ok(())

    // handle_thread.join().unwrap();
}
