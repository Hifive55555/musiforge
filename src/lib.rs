// use std::collections::HashMap;
pub mod ui;
pub mod key;
pub mod musiblock;

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

pub mod config {
    extern crate anyhow;
    extern crate cpal;

    use cpal::{
        traits::{DeviceTrait, HostTrait},
        SizedSample,
    };
    use cpal::{FromSample, Sample};

    pub fn stream_setup_for<SampleType>(
        process_fn: impl FnMut(&mut [SampleType], usize, f32) + Send + 'static
    ) -> Result<cpal::Stream, anyhow::Error>
    where
        SampleType: SizedSample + FromSample<f32>,
    {
        let (_host, device, config) = host_device_setup()?;

        match config.sample_format() {
            cpal::SampleFormat::I8 => make_stream::<i8, SampleType>(&device, &config.into(), process_fn),
            cpal::SampleFormat::I16 => make_stream::<i16, SampleType>(&device, &config.into(), process_fn),
            cpal::SampleFormat::I32 => make_stream::<i32, SampleType>(&device, &config.into(), process_fn),
            cpal::SampleFormat::I64 => make_stream::<i64, SampleType>(&device, &config.into(), process_fn),
            cpal::SampleFormat::U8 => make_stream::<u8, SampleType>(&device, &config.into(), process_fn),
            cpal::SampleFormat::U16 => make_stream::<u16, SampleType>(&device, &config.into(), process_fn),
            cpal::SampleFormat::U32 => make_stream::<u32, SampleType>(&device, &config.into(), process_fn),
            cpal::SampleFormat::U64 => make_stream::<u64, SampleType>(&device, &config.into(), process_fn),
            cpal::SampleFormat::F32 => make_stream::<f32, SampleType>(&device, &config.into(), process_fn),
            cpal::SampleFormat::F64 => make_stream::<f64, SampleType>(&device, &config.into(), process_fn),
            sample_format => Err(anyhow::Error::msg(format!(
                "Unsupported sample format '{sample_format}'"
            ))),
        }
    }

    pub fn host_device_setup(
    ) -> Result<(cpal::Host, cpal::Device, cpal::SupportedStreamConfig), anyhow::Error> {
        let host = cpal::default_host();

        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow::Error::msg("Default output device is not available"))?;
        println!("Output device : {}", device.name()?);

        let config = device.default_output_config()?;
        println!("Default output config : {:?}", config);

        Ok((host, device, config))
    }

    pub fn make_stream<T, SampleType>(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        mut process_fn: impl FnMut(&mut [SampleType], usize, f32) + Send + 'static,
    ) -> Result<cpal::Stream, anyhow::Error>
    where
        T: SizedSample + FromSample<f32>,
        SampleType: SizedSample + FromSample<f32>,
    {
        let num_channels = config.channels as usize;
        let err_fn = |err| eprintln!("Error building output sound stream: {}", err);

        let time_at_start = std::time::Instant::now();
        println!("Time at start: {:?}", time_at_start);

        let stream = device.build_output_stream(
            config,
            move |output: &mut [SampleType], _: &cpal::OutputCallbackInfo| {
                let time_since_start = std::time::Instant::now()
                    .duration_since(time_at_start)
                    .as_secs_f32();
                process_fn(output, num_channels, time_since_start)
            },
            err_fn,
            None,
        )?;

        Ok(stream)
    }

}

pub trait OutputTrait {
    // 每个实现该 trait 的结构体首先要设置输出列表

    // 主动寻找 output
    fn set_output(&mut self, output: Box<&dyn InputTrait>);
    // 1. 通常增加一个 HashMap 的 KV 对（便于程序辨别）
    // 2. 通过访问该块的公有标识属性（便于使用者辨别）
    // 返回身份符

    // 双向建立起一个信道（多线程或单线程）
    // 多线程：通过一个特定的监听信道
    // 单线程：通过 HashMap 标识符
    fn send(&mut self);
}

pub trait InputTrait {
    //主动寻找 input
    fn set_input(&mut self, output: Box<&dyn OutputTrait>);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }

    #[test]
    fn envelop() {
        use musiblock::{Envelope, Node, CurveType};
        let mut env_master = Envelope::from(vec![
            Node {t:0.0, v: 1.0, curve: CurveType::Linear, if_hold: false},
            Node {t:0.5, v: 0.7, curve: CurveType::Linear, if_hold: true},
            Node {t:1.0, v: 0.0, curve: CurveType::Linear, if_hold: false},
        ]);
        while let Some(value) = env_master.tick() {
            if approx_eq(value, 0.0) {println!("1 {}", value);}
            if approx_eq(value, 3.0) {
                println!("r {}", value);
                env_master.release_hold();
            }
        } 
        println!("结束！");
    }
}

pub fn approx_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.004
}

use log::info;

pub fn init_logger() {
    // use chrono::Local;
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
