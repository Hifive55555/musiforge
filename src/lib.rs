pub mod key;
pub mod musiblock;
pub mod effects;


pub mod config {
    extern crate anyhow;
    extern crate cpal;

    use cpal::{
        traits::{DeviceTrait, HostTrait},
        SizedSample,
    };
    use cpal::FromSample;

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
    fn envelop() {
        use musiblock::{Envelope, Node, CurveType};
        let mut env_master = Envelope::from(vec![
            Node {t:0.0, v: 1.0, curve: CurveType::Linear, if_hold: false},
            Node {t:0.5, v: 0.7, curve: CurveType::Linear, if_hold: true},
            Node {t:1.0, v: 0.0, curve: CurveType::Linear, if_hold: false},
        ], 48000.0);
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


const MAX_NORMALIZED_VALUE: f32 = 1.0;

// 分贝与电平转换
pub fn db_to_vol(dbfs_value: f32) -> f32 {
    let factor = f32::exp((dbfs_value / 10.0).ln()); // 使用e为底的对数进行逆运算
    factor * MAX_NORMALIZED_VALUE.signum()
}

pub fn vol_to_db(normalized_value: f32) -> f32 {
    10.0 * f32::log10(normalized_value.abs() / MAX_NORMALIZED_VALUE)
}


use cpal::{
    traits::{DeviceTrait, StreamTrait},
    BufferSize, StreamConfig,
};

use config::host_device_setup;


pub fn create_stream<SampleType, DataFunc>(
    buffer_size: u32,
    data_callback: DataFunc,
) -> impl Fn()
where
    SampleType: cpal::SizedSample,
    DataFunc: FnMut(&mut [SampleType], &cpal::OutputCallbackInfo) + Send + 'static,
{
    let (_host, device, config) = host_device_setup().unwrap();
    let mut config = StreamConfig::from(config);
    config.buffer_size = BufferSize::Fixed(buffer_size);

    let _num_channels = config.channels as usize;
    // let time_at_start = std::time::Instant::now();

    let error_callback = |err| eprintln!("Error building output sound stream: {}", err);
    // let data_callback = move |output: &mut [SampleType], _: &cpal::OutputCallbackInfo| {};

    let stream = device
        .build_output_stream(&config, data_callback, error_callback, None)
        .unwrap();

    move || {
        println!("Stream is playing!");
        stream.play().unwrap();
    }
}


use std::time::Duration;

/// 播放时间
#[derive(PartialEq, Clone, Copy)]
pub struct ClockTime<const SAMPLE_RATE: u32> {
    pub sec: i64,
    pub sample: u32,
}

impl<const SAMPLE_RATE: u32> ClockTime<SAMPLE_RATE> {
    pub fn new() -> Self {
        ClockTime {
            sec: 0,
            sample: 0
        }
    }

    pub fn tick(&mut self) {
        self.sample += 1;
        self.sec += (self.sample / SAMPLE_RATE) as i64;
        self.sample %= SAMPLE_RATE;
    }
}

impl<const SAMPLE_RATE: u32> std::ops::Add<ClockTime<SAMPLE_RATE>> for ClockTime<SAMPLE_RATE> {
    type Output = ClockTime<SAMPLE_RATE>;

    fn add(self, rhs: ClockTime<SAMPLE_RATE>) -> Self::Output {
        let sample = self.sample + rhs.sample;
        let sec = self.sec + rhs.sec + (sample / SAMPLE_RATE) as i64;
        let sample = sample / SAMPLE_RATE;

        ClockTime {
            sec,
            sample
        }
    }
}

impl<const SAMPLE_RATE: u32> std::ops::Sub<ClockTime<SAMPLE_RATE>> for ClockTime<SAMPLE_RATE> {
    type Output = ClockTime<SAMPLE_RATE>;

    fn sub(self, rhs: ClockTime<SAMPLE_RATE>) -> Self::Output {
        let sec: i64;
        let sample: u32;

        if self.sample > rhs.sample {
            sample = self.sample - rhs.sample;
            sec = self.sec - rhs.sec;
        } else {
            sample = SAMPLE_RATE + self.sample - rhs.sample;
            sec = self.sec - rhs.sec - 1;
        }

        ClockTime {
            sec,
            sample
        }
    }
}

impl<const SAMPLE_RATE: u32> From<ClockTime<SAMPLE_RATE>> for Duration {
    fn from(value: ClockTime<SAMPLE_RATE>) -> Self {
        Duration::new(
            value.sec.abs() as u64,
            value.sample * 1000_000_000 / SAMPLE_RATE
        )
    }
}

impl<const SAMPLE_RATE: u32> From<f32> for ClockTime<SAMPLE_RATE> {
    fn from(value: f32) -> Self {
        let sec = value as i64;
        let sample = ((value - sec as f32) * SAMPLE_RATE as f32) as u32;

        ClockTime {
            sec,
            sample
        }
    }
}

