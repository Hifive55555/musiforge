// extern crate cpal;
// use cpal::

use crate::key::*;
// use std::thread;
// use std::collections::VecDeque;
// use std::collections::HashMap;
use std::f32::consts::PI;
use std::f32::consts::E;

const DOUBLE_PI: f32 = 2.0 * PI;

pub struct Piano {
    generators: Vec<PianoGenerator>,
    alpha: f32,
}

impl Piano {
    pub fn new(alpha: f32) -> Self {
        Piano {
            generators: Vec::new(),
            alpha,
        }
    }

    pub fn send_key<K: Key>(&mut self, key: K) {
        // match key.get_type() {
        //     KeyType::Freq => {
        //         thread::spawn(move || {
        //             let p = PianoGenerator::new();
        //         });
        //     },
        //     _ => {
        //         println!("----- {} -----", key.to_freqkey().0)
        //     },
        // }
        let key = key.to_freqkey();
        self.generators.push(PianoGenerator::new(key, self.alpha));
    }

    pub fn tick(&mut self) -> f32 {
        let mut output: f32 = 0.0;
        self.generators.retain_mut(|p| {
            match p.tick() {
                Some(value) => {
                    output += value;
                    true
                },
                None => false,
            }
        });
        output
    }
}

struct PianoGenerator {
    key: FreqKey,
    oscs: Vec<Oscillator>,
    current_sample: f32,
    // data: VecDeque<f64>,
}

impl PianoGenerator {
    fn new(key: FreqKey, alpha: f32) -> Self {
        // 计算泛音
        let mut oscs: Vec<Oscillator> = Vec::new();
        let mut n: f32 = 1.0;
        loop {
            if key.f * n > 20000.0 {
                break;
            }
            println!("frq: {} , n: {}, vol: {}", key.f * n, n, (1.0 / (n*n)));
            oscs.push(Oscillator {
                sample_rate: 44100.0,
                current_sample_index: 0.0,
                frequency_hz: key.f * n,
                volume: (1.0 / (n*n)) * (DOUBLE_PI * n * alpha).sin() * (DOUBLE_PI * n * alpha).sin(),    // 求解能量收敛的波动方程
            });
            n += 1.0;
        }

        Self {
            key,
            oscs,
            current_sample: 0.0,
            // data: VecDeque::new(),
        }
    }

    fn control(&mut self) -> Option<f32> {
        if self.current_sample <= self.key.duration * 44100.0 {
            // 输出一个 e^(-x) 的衰减值
            Some(E.powf(- self.current_sample))
        } else if self.current_sample <= (self.key.duration + 1.0) * 44100.0 {
            Some(
                // 线性衰减
                E.powf(- self.key.duration * 44100.0) * (self.current_sample + (1.0 - self.key.duration)* 44100.0)
            )
        } else {
            None
        }
    }

    fn tick(&mut self) -> Option<f32> {
        self.current_sample += 1.0 / 44100.0;
        let mut output: f32 = 0.0;
        for osc in &mut self.oscs {
            output += osc.tick();
        }
        // self.data.push_back(output);
        match self.control() {
            Some(reduction) => Some(output * reduction),
            None => None,
        }
    }
}

pub struct Oscillator {
    pub sample_rate: f32,
    pub current_sample_index: f32,
    pub frequency_hz: f32,
    pub volume: f32,    // 看最大值是不是 1
}

impl Oscillator {
    fn advance_sample(&mut self) {
        self.current_sample_index = (self.current_sample_index + 1.0) % self.sample_rate;
    }

    fn calculate_sine_output_from_freq(&self, freq: f32) -> f32 {
        (self.current_sample_index * freq * DOUBLE_PI / self.sample_rate).sin()
    }

    fn tick(&mut self) -> f32 {
        self.advance_sample();
        self.calculate_sine_output_from_freq(self.frequency_hz) * self.volume
    }
}