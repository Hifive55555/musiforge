use cpal::traits::{DeviceTrait, HostTrait};
use musiforge::{
    block::{IOData, Time}, create_stream, frame_block, graph_flow::*
};
use plotters::prelude::*;

fn osc(mut time: Time, inputs: &IOData, outputs: &mut IOData, num_channels: usize) {
    // let freq = inputs[0][0];
    let freq = 440.0;
    // println!("freq: {}", freq);
    for frame in outputs[0].chunks_mut(num_channels) {
        let val = time.get_phase_by_freq(freq).sin();
        // println!("val: {}", val);
        for sample in frame.iter_mut() {
            *sample = val;
        }
        time.tick();
    }
}

fn osc_2(time: Time, _inputs: &IOData, outputs: &mut IOData, num_channels: usize) {
    let freq = 660.0;
    let val = time.get_phase_by_freq(freq).sin();
    for sample in outputs[0].iter_mut() {
        *sample = val;
    }
}

fn filter(_time: Time, inputs: &IOData, outputs: &mut IOData, _num_channels: usize) {
    for i in 0..inputs.buffer_size() {
        outputs[0][i] = inputs[0][i];
        // println!("filter: {}", outputs[0][i]);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let builder = GraphFlowBuilder {
        sample_rate: 48000,
        num_channels: 2,
        ..Default::default()
    };
    let mut gf = builder.build();

    let osc_id = gf.add_block(osc);
    let osc_2_id = gf.add_block(frame_block!(osc_2));
    let filter_id = gf.add_block(filter);

    gf.connect(osc_id.port(0), filter_id.port(0));
    gf.connect(osc_2_id.port(0), filter_id.port(0));
    gf.to_output(filter_id.port(0));

    // let freq_listener = gf.add_listener(osc_id.port(0));

    println!("{:?}", gf);

    let stream = create_stream(480, 48000, move |datum: &mut [f32], _| {
        gf.run(datum.len() as u32, datum);
    });
    stream();
    std::thread::park();


    // let app = Content::new(freq_listener);

    // let options = eframe::NativeOptions::default();
    // let _ = eframe::run_native(
    //     "My egui Application",
    //     options,
    //     Box::new(|_cc| Ok(Box::new(app))),
    // );


    Ok(())
}

use eframe::egui;

struct BiBuffer {
    data: [Vec<f32>; 2],
    buffer_index: usize,
}

pub struct Content {
    freq_listener: Listener,
    freq: f32,
}

impl Content {
    fn new(freq_listener: Listener) -> Self {
        freq_listener.send(440.0);

        Content {
            freq_listener,
            freq: 440.0,
        }
    }
}

impl eframe::App for Content {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("ha");
            if ui.add(egui::Slider::new(&mut self.freq, 330.0..=880.0)).changed() {
                self.freq_listener.send(self.freq);
            }
        });
    }
}