use musiforge::ui::Content;
use musiforge::config::stream_setup_for;
use musiforge::{musiblock, init_logger, approx_eq};
use musiforge::musiblock::{Piano, Node};

// use env_logger;
use log::info;
use cpal::traits::StreamTrait;
// use eframe::egui;
use std::thread;
use std::sync::{mpsc, Mutex, Arc};
// use std::rc::Arc;

fn main() -> eframe::Result<()> {
    // 注册 env_logger
    init_logger();

    thread::spawn(move || -> anyhow::Result<()> {
        let mut p = Piano::new();
        
        let stream = stream_setup_for(
            move |data: &mut [f32], num_channels, time| {
            if approx_eq(time, 3.0) {
                let midi_message: &[u8; 3] = &[0x90, 0x40, 0x90];
                p.handle_midi_message(midi_message);
            }
            if approx_eq(time, 4.0) {
                let midi_message: &[u8; 3] = &[0x80, 0x40, 0x90];
                p.handle_midi_message(midi_message);
            }
            for frame in data.chunks_mut(num_channels) {
                let value = p.tick();
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
    std::thread::park();
    return Ok(());
    

    // Initialize ui
    let options = eframe::NativeOptions::default();
    // eframe::run_native(
    //     "Keyboard events",
    //     options,
    //     Box::new(|_cc| Box::new(Content::new(tx))),
    // )
}
