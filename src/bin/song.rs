use std::sync::{Arc, Mutex};

use musiforge::{
    create_stream, init_logger,
    musiblock::{pattern, AdditiveSynth, MidiRack, Note, Oscillator},
    ClockTime,
};

const SAMPLE_RATE: u32 = 48000;


fn create_graph_flow() -> impl FnMut(ClockTime<SAMPLE_RATE>) -> f32 {
    let notes: Vec<Note<ClockTime<SAMPLE_RATE>, [u8; 3]>> = vec![
        (1.0, [0x90, 67, 100]),
        (1.6, [0x80, 67, 100]),
        (2.0, [0x90, 67, 100]),
        (2.6, [0x80, 67, 100]),
        (1.0, [0x90, 55, 100]),
        (2.8, [0x80, 55, 100]),

        (3.0, [0x90, 74, 100]),
        (3.6, [0x80, 74, 100]),
        (4.0, [0x90, 74, 100]),
        (4.6, [0x80, 74, 100]),
        (3.0, [0x90, 66, 100]),
        (4.8, [0x80, 66, 100]),

        (5.0, [0x90, 76, 100]),
        (5.6, [0x80, 76, 100]),
        (6.0, [0x90, 76, 100]),
        (6.6, [0x80, 76, 100]),
        (5.0, [0x90, 67, 100]),
        (6.8, [0x80, 67, 100]),

        (6.8, [0x90, 64, 50]),
        (7.0, [0x80, 64, 50]),

        (7.0, [0x90, 74, 100]),
        (8.6, [0x80, 74, 100]),
        (6.9, [0x90, 59, 100]),
        (8.8, [0x80, 59, 100]),
    ]
    .iter()
    .map(|(time, signal)| Note {
        time: ClockTime::from(*time),
        signal: *signal,
    })
    .collect();

    let pat = pattern(notes);
    let mut modulate = Oscillator::new(3.0, SAMPLE_RATE as f32);
    let additive_synth = Arc::new(Mutex::new(AdditiveSynth::new(4, SAMPLE_RATE as f32)));
    let mut synth_rack = MidiRack::new(Arc::clone(&additive_synth));

    move |g_time: ClockTime<SAMPLE_RATE>| -> f32 {
        // send keys
        for midi_msg in pat(g_time).iter() {
            synth_rack.send(midi_msg);
        };
        // modulate harmonics
        additive_synth.lock().unwrap().harmonic_vols[1] = (1.0 + modulate.tick()) / 3.0;
        additive_synth.lock().unwrap().harmonic_vols[2] = (1.0 + modulate.tick()) / 4.0;
        additive_synth.lock().unwrap().harmonic_vols[3] = (1.0 + modulate.tick()) / 5.0;

        synth_rack.tick()
    }
}

fn data_callback() -> impl FnMut(&mut [f32], &cpal::OutputCallbackInfo) + Send + 'static {
    let mut global_clock: ClockTime<SAMPLE_RATE> = ClockTime::new();
    let mut graph_flow = create_graph_flow();

    move |data: &mut [f32], _info: &cpal::OutputCallbackInfo| {
        for frame in data.chunks_mut(2) {
            global_clock.tick();
            let vol = graph_flow(global_clock);
            for sample in frame.iter_mut() {
                *sample = vol;
            }
        }
    }
}

fn main() {
    init_logger();
    let gf = create_stream(10240, data_callback());
    gf();
    std::thread::park();
}
