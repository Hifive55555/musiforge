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

    pub enum Waveform {
        Sine,
        Square,
        Saw,
        Triangle,
    }

    pub struct Oscillator {
        pub sample_rate: f32,
        pub waveform: Waveform,
        pub current_sample_index: f32,
        pub frequency_hz: f32,
    }

    impl Oscillator {
        fn advance_sample(&mut self) {
            self.current_sample_index = (self.current_sample_index + 1.0) % self.sample_rate;
        }

        fn set_waveform(&mut self, waveform: Waveform) {
            self.waveform = waveform;
        }

        fn calculate_sine_output_from_freq(&self, freq: f32) -> f32 {
            let two_pi = 2.0 * std::f32::consts::PI;
            (self.current_sample_index * freq * two_pi / self.sample_rate).sin()
        }

        fn is_multiple_of_freq_above_nyquist(&self, multiple: f32) -> bool {
            self.frequency_hz * multiple > self.sample_rate / 2.0
        }

        fn sine_wave(&mut self) -> f32 {
            self.advance_sample();
            self.calculate_sine_output_from_freq(self.frequency_hz)
        }

        fn generative_waveform(&mut self, harmonic_index_increment: i32, gain_exponent: f32) -> f32 {
            self.advance_sample();
            let mut output = 0.0;
            let mut i = 1;
            while !self.is_multiple_of_freq_above_nyquist(i as f32) {
                let gain = 1.0 / (i as f32).powf(gain_exponent);
                output += gain * self.calculate_sine_output_from_freq(self.frequency_hz * i as f32);
                i += harmonic_index_increment;
            }
            output
        }

        fn square_wave(&mut self) -> f32 {
            self.generative_waveform(2, 1.0)
        }

        fn saw_wave(&mut self) -> f32 {
            self.generative_waveform(1, 1.0)
        }

        fn triangle_wave(&mut self) -> f32 {
            self.generative_waveform(2, 2.0)
        }

        fn tick(&mut self) -> f32 {
            match self.waveform {
                Waveform::Sine => self.sine_wave(),
                Waveform::Square => self.square_wave(),
                Waveform::Saw => self.saw_wave(),
                Waveform::Triangle => self.triangle_wave(),
            }
        }
    }

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
        let mut oscillator = Oscillator {
            waveform: Waveform::Sine,
            sample_rate: config.sample_rate.0 as f32,
            current_sample_index: 0.0,
            frequency_hz: 440.0,
        };
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
}
