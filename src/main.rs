use musiforge::ui::Content;
use musiforge::config::stream_setup_for;
use musiforge::{musiblock, key::*};

// use env_logger;
use log::{debug, info, trace, warn};
use cpal::traits::StreamTrait;
// use eframe::egui;
use std::thread;
use std::sync::{mpsc, Mutex, Arc};
// use std::rc::Arc;
// use std::f32::EPSILON;

fn main() -> eframe::Result<()> {
    // 注册 env_logger
    env_logger::init();
    // let (tx, rx) = mpsc::channel::<Box<dyn Key>>();
    let mut waiting_keys: Arc<Mutex<Vec<Box<dyn Key>>>> = Arc::new(Mutex::new(Vec::new()));
    let tx = Arc::clone(&waiting_keys);

    thread::spawn(move || -> anyhow::Result<()> {
        info!("starting piano");
        let mut p = musiblock::Piano::new(1.0 / 3.0);
        
        let stream = stream_setup_for(
            move |data: &mut [f32], num_channels, time_start| {
            let binding = Arc::clone(&waiting_keys);
            let mut waiting_keys = binding.lock().unwrap();
            
            // 在env_logger中打印调试信息
            info!("KKK: {:?}", waiting_keys);
            waiting_keys.retain(|key| {
                p.send_key(Box::new(key.to_freqkey()));
                false
            });
    
            for frame in data.chunks_mut(num_channels) {
                let value = p.tick();
                for sample in frame.iter_mut() {
                    *sample = value;
                }
            }
        })?;
        stream.play()?;
        // std::thread::sleep(std::time::Duration::from_millis(8000));
        std::thread::sleep(std::time::Duration::from_millis(100000000000000));
        Ok(())
    });
    

    // Initialize ui
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Keyboard events",
        options,
        Box::new(|_cc| Box::new(Content::new(tx))),
    )
}

// 比较两个 f32 值
fn approx_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.004
}