use super::key;
use super::key::FreqKey;

use eframe::egui;
use egui::*;
use std::f32::EPSILON;
use std::sync::{Arc, Mutex};

// #[derive(Default)]
pub struct Content {
    text: String,
    // tx: std::sync::mpsc::Sender<Box<dyn key::Key>>,
    tx: Arc<Mutex<Vec<Box<dyn key::Key>>>>,

    duration: f32,
    freq: f32,
    ln_freq: f32,
    compare_freq: f32,
}

impl Content {
    pub fn new(tx: Arc<Mutex<Vec<Box<dyn key::Key>>>>) -> Self {
        Self {
            text: String::from(""),
            tx,

            duration: 1.0,
            freq: 440.0,
            ln_freq: 440.0_f32.ln(),
            compare_freq: 0.0,
        }
    }
}

impl eframe::App for Content {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            
            if (self.freq - self.compare_freq).abs() > EPSILON {
                self.ln_freq = self.freq.ln();
            } else {
                self.freq = self.ln_freq.exp();
            }
            self.compare_freq = self.freq;

            ui.heading("My egui Application");
            ui.add(egui::Slider::new(&mut self.ln_freq, 20.0_f32.ln()..=20000.0_f32.ln()).text("ln(f)"));
            ui.add(egui::DragValue::new(&mut self.freq).speed(1.0));
            ui.horizontal( |ui| {
                if ui.button("Increment").clicked() {
                    self.freq += 0.1;
                }
                if ui.button("Decrement").clicked() {
                    self.freq -= 0.1;
                }
            });
            ui.add(egui::Slider::new(&mut self.duration, 0.1..=10.0).text("Duration"));
            ui.label(format!("Duration {0}s , Frequency{1} Hz", self.duration, self.freq));
            // Send Key!!
            if ui.button("Send").clicked() {
                let mut waiting_keys = self.tx.lock().unwrap();
                waiting_keys.push(Box::new(
                    FreqKey { f: self.freq, volume: 1, duration: self.duration})
                );
                println!("Keys' length: {}", waiting_keys.len());
            }

            // =================================== //

            if ui.button("Clear").clicked() {
                self.text.clear();
            }
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    ui.label(&self.text);
                });

            if ctx.input(|i| i.key_pressed(Key::A)) {
                self.text.push_str("\nPressed");
            }
            if ctx.input(|i| i.key_down(Key::A)) {
                self.text.push_str("\nHeld");
                ui.ctx().request_repaint(); // make sure we note the holding.
            }
            if ctx.input(|i| i.key_released(Key::A)) {
                self.text.push_str("\nReleased");
            }
        });
    }
}