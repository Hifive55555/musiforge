use musiforge::ui::Content;
use musiforge::config::stream_setup_for;
use musiforge::{musiblock, key::*};

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
    // let (tx, rx) = mpsc::channel::<Box<dyn Key>>();
    let mut waiting_keys: Arc<Mutex<Vec<Box<dyn Key>>>> = Arc::new(Mutex::new(Vec::new()));
    let tx = Arc::clone(&waiting_keys);

    thread::spawn(move || -> anyhow::Result<()> {
        info!("starting piano");
        let mut p = musiblock::Piano::new(1.0 / 5.0);
        
        let stream = stream_setup_for(
            move |data: &mut [f32], num_channels, time_start| {
            let binding = Arc::clone(&waiting_keys);
            let mut waiting_keys = binding.lock().unwrap();
            
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
        std::thread::park();
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
    // use std::f32::EPSILON;
    (a - b).abs() < 0.004
}

fn init_logger() {
    use chrono::Local;
    use std::io::Write;
    use env_logger::fmt::Color;
    use env_logger::Env;
    use log::LevelFilter;

    let env = Env::default().filter_or("MY_LOG_LEVEL", "debug");
    // let file_path = "target/log.log";
    
    // 设置日志打印格式
    env_logger::Builder::from_env(env)
    .format(|buf, record| {
        let level_color = match record.level() {
            log::Level::Error => Color::Red,
            log::Level::Warn => Color::Yellow,
            log::Level::Info => Color::Green,
            log::Level::Debug | log::Level::Trace => Color::Cyan,
        };

        let mut level_style = buf.style();
        level_style.set_color(level_color).set_bold(true);

        let mut style = buf.style();
        style.set_color(Color::White).set_dimmed(true);

        writeln!(
            buf,
            "{} [ {} ] {}",
            // Local::now().format("%Y-%m-%d %H:%M:%S"),
            level_style.value(record.level()),
            style.value(record.module_path().unwrap_or("<unnamed>")),
            record.args()
        )
    })
    .filter(None, LevelFilter::Debug)
    // .target(std::fs::File::create(file_path).unwrap())
    .init();
    info!("env_logger initialized.");
}
