use std::collections::HashMap;
use std::f32::consts::PI;
use log::debug;

use crate::ClockTime;

use super::db_to_vol;
const DOUBLE_PI: f32 = 2.0 * PI;
const NOTE_ON_MSG: u8 = 0x90;
const NOTE_OFF_MSG: u8 = 0x80;


fn midi_to_freq(midi_key: u8) -> f32 {
    return 440.0_f32 * 2.0_f32.powf((midi_key as i8 - 0x45) as f32 / 12.0); // 440 * 2^((key-69)/12)
}

pub struct Oscillator {
    sample_rate: f32,
    pub t: f32,
    pub freq: f32,
}

impl Oscillator {
    pub fn new(freq: f32, sample_rate: f32) -> Self {
        Self {
            freq,
            t: 0.0,
            sample_rate,
        }
    }

    pub fn tick(&mut self) -> f32 {
        self.t = (self.t + 1.0) % (self.sample_rate * self.freq);
        (self.t * self.freq * DOUBLE_PI / self.sample_rate).sin()
    }
}

pub struct AdditiveSynth {
    sample_rate: f32,
    pub osc_num: usize,
    pub harmonic_vols: Vec<f32>,
    pub env_master: Envelope,  // 总 ENVELOP 函数
}

impl AdditiveSynth {
    pub fn new(osc_num: usize, sample_rate: f32) -> Self {
        let harmonic_vols = vec![0.1; osc_num];
        // 生成主包络
        let env_master = Envelope::from(vec![
            Node {t:0.0, v: 1.0, curve: CurveType::Linear, if_hold: false},
            Node {t:0.5, v: 0.7, curve: CurveType::Linear, if_hold: true},
            Node {t:1.0, v: 0.0, curve: CurveType::Linear, if_hold: false},
        ], sample_rate);

        AdditiveSynth {
            sample_rate,
            osc_num,
            harmonic_vols,
            env_master,
        }
    }
}

pub struct AdditiveUnit {
    osc_num: usize,
    oscs: Vec<Oscillator>,
    harmonic_vols: Vec<f32>,
    env_master: Envelope,
    vol: f32,
}

impl MidiUnit for AdditiveUnit {
    fn send(&mut self, midi_msg: &[u8]) {
        match midi_msg[0] {
            NOTE_ON_MSG => {
                debug!("发送了 Midi - NOTE ON 信号 {:?}", midi_msg);
                self.vol = midi_msg[2] as f32 / 128.0;
            },

            NOTE_OFF_MSG => {
                debug!("发送 Midi - NOTE OFF 信号 {:?}", midi_msg);
                // 释放 hold，使其衰减（目前只能无脑release，之后考虑定向释放）
                self.env_master.release_hold();
            },

            _ => {}
        };
    }

    fn tick(&mut self) -> Option<f32> {
        let mut output = 0.0;
        for i in 0..self.osc_num {
            output += self.oscs[i].tick() * self.harmonic_vols[i] * self.vol;
        }
        // 计算 midi 按键“声速”
        output = clip(clip(output) * self.vol);
        // 通过 envelop
        match self.env_master.tick() {
            Some(reduction) => Some(output * reduction),
            None => None,
        }
    }
}

impl MidiSynth<AdditiveUnit> for AdditiveSynth {
    fn spawn(&self, midi_msg: &[u8]) -> AdditiveUnit {
        let base_freq = midi_to_freq(midi_msg[1]);

        let oscs = (1..=self.osc_num)
            .map(|i| Oscillator::new(base_freq * i as f32, self.sample_rate))
            .collect();

        AdditiveUnit {
            osc_num: self.osc_num,
            oscs,
            harmonic_vols: self.harmonic_vols.clone(),
            env_master: self.env_master.clone(),
            vol: db_to_vol(midi_msg[2] as f32 / 12.7),
        }
    }
}

pub trait MidiUnit {
    fn send(&mut self, midi_msg: &[u8]);
    fn tick(&mut self) -> Option<f32>;
}

pub trait MidiSynth<U: MidiUnit> {
    fn spawn(&self, midi_msg: &[u8]) -> U;
}

pub struct MidiRack<S, U>
where
    S: MidiSynth<U>,
    U: MidiUnit,
{
    key_units: HashMap<u8, U>,
    synth: Arc<Mutex<S>>,
}

impl<S, U> MidiRack<S, U>
where
    S: MidiSynth<U>,
    U: MidiUnit,
{
    pub fn new(synth: Arc<Mutex<S>>) -> Self {
        debug!("初始化 MIDI Rack");

        MidiRack {
            key_units: HashMap::new(),
            synth,
        }
    }

    pub fn send(&mut self, midi_msg: &[u8]) {
        match midi_msg[0] {
            NOTE_ON_MSG => {
                if self.key_units.contains_key(&midi_msg[1]) || self.key_units.len() < 8 {
                    self.key_units.insert(midi_msg[1], self.synth.lock().unwrap().spawn(midi_msg));
                }
            },
            NOTE_OFF_MSG => {
            },
            _ => {}
        };
        if let Some(unit) = self.key_units.get_mut(&midi_msg[1]) {
            unit.send(midi_msg);
        }
    }

    pub fn tick(&mut self) -> f32 {
        let mut out = 0.0;
        let mut remove_keys = Vec::new();
        for (key, unit) in self.key_units.iter_mut() {
            if let Some(vol) = unit.tick() {
                out += vol;
            } else {
                remove_keys.push(*key);
            }
        }
        for remove_key in remove_keys {
            self.key_units.remove(&remove_key);
        }
        out
    }
}



#[derive(Copy, Clone)]
pub enum CurveType {
    Linear,
    // 下面的以后做，但是意味着 Envelop 的架构要整体改，目前只能邻近插值
    Newton,
    Hermite,
    CubicSpline,
}

#[derive(Copy, Clone)]
pub struct Node {
    pub t: f32,
    pub v: f32,
    pub curve: CurveType,
    pub if_hold: bool,
}

// 包络生成器
#[derive(Clone)]  // 考虑去除
pub struct Envelope {
    sample_rate: f32,
    nodes: Vec<Node>,
    current_time: f32,
    current_value: f32,
    current_index: usize,

    in_hold: bool,
    ready_release: bool,  // 待释放队列（因为无法定向释放，故只需要存储带释放的数量）
}

use super::approx_eq;

impl Envelope {
    // ...
    
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            nodes: Vec::new(),
            current_time: 0.0,
            current_value: 0.0,
            current_index: 0,

            in_hold: false,
            ready_release: false,
        }
    }

    pub fn from(nodes: Vec<Node>, sample_rate: f32) -> Self {
        let mut envelope = Self {
            sample_rate,
            nodes,
            current_time: 0.0,
            current_value: 0.0,
            current_index: 0,

            in_hold: false,
            ready_release: false,
        };
        envelope.sort();
        envelope
    }

    pub fn tick(&mut self) -> Option<f32> {
        if !self.in_hold {
            self.current_time += 1.0 / self.sample_rate;
        } else {
            return Some(self.current_value);
        }
        // 边界检查，到达最后一个节点时返回 None
        if self.current_index + 1 == self.nodes.len() {return None;}
        // 当前的节点，若存在下一个节点，则伺机等待
        let node = self.nodes[self.current_index];
        let node_2 = self.nodes[self.current_index+1];
        // 判断并计算插值
        self.current_value = Self::interpolation(&[node, node_2], self.current_time - node.t);
        // 判断是切换下一个节点
        if approx_eq(self.current_time, node_2.t) {
            // debug!("切换节点: {}", self.current_index);
            // 不仅节点为 hold 类型，还要判断是否已经提前释放了
            if node_2.if_hold {
                if !self.ready_release {self.in_hold = true;}
                self.ready_release = false;
            }
            self.current_index += 1;
        }

        Some(self.current_value)
    }

    // 由外部控制释放 hold
    pub fn release_hold(&mut self) {
        if self.in_hold {
            self.in_hold = false;
        } else {
            self.ready_release = true;
        }
    }

    pub fn push(&mut self, node: Node) {
        self.nodes.push(node);
        self.sort();
    }

    fn sort(&mut self) {
        self.nodes.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap());
    }

    // 插值算法实现（由上一个节点确定曲线类型）
    fn interpolation(nodes: &[Node], current_time: f32) -> f32 {
        // ...
        match nodes[0].curve {
            CurveType::Linear => {
                nodes[0].v + current_time * (nodes[0].v - nodes[1].v)/(nodes[0].t - nodes[1].t)
            }
            _ => {1.0}
        }
    }
}

// 目前先搞线性的
struct Compressor {
    threshold: f32,
    kick: f32
}

impl Compressor {
    pub fn tick(&self, input: f32) -> f32 {
        // if input < self.kick {return input;}
        // else {
        // }
        0.0
    }
}

struct Limiter {
    threshold: f32,
}

impl Limiter {
    pub fn new() -> Self {
        Self {threshold: 10.0}
    }
    pub fn tick(&self, input: f32) -> f32 {
        if input < self.threshold {input}
        else {self.threshold}
    }
}

pub fn clip(input: f32) -> f32 {
    input.min(1.0).max(-1.0)
}

#[derive(Copy, Clone)]
pub struct Note<SampleType, SignalType: Clone>
{
    pub time: SampleType,
    pub signal: SignalType,
}


// pattern for Duration
pub fn pattern<const SAMPLE_RATE: u32, SignalType: Clone>(
    notes: Vec<Note<ClockTime<SAMPLE_RATE>, SignalType>>
) -> impl Fn(ClockTime<SAMPLE_RATE>) -> Vec<SignalType> {
    move |time: ClockTime<SAMPLE_RATE>| {
        let mut signals = Vec::new();
        for note in &notes {
            if note.time == time {
                // println!("time: {}", time.sec);
                signals.push(note.signal.clone());
            }
        }
        signals
    }
}

use midir::{Ignore, MidiIO, MidiInput};
use std::{
    sync::{Arc, Mutex},
    error::Error,
    io::{stdin, stdout, Write},
};

pub fn listen<const SAMPLE_RATE: u32, S, U>(
    sender: Arc<Mutex< MidiRack<S, U> >>
) -> Result<(), Box<dyn Error>>
where
    S: MidiSynth<U> + Send + Sync + 'static,
    U: MidiUnit + Send + Sync + 'static,
{
    // 实时设备连接
    let mut midi_in = MidiInput::new("midir forwarding input")?;
    midi_in.ignore(Ignore::None);

    let in_port = select_port(&midi_in, "input")?;
    let in_port_name = midi_in.port_name(&in_port)?;
    println!("\nOpening connections {}", in_port_name);

    // _conn_in needs to be a named parameter, because it needs to be kept alive until the end of the scope
    let _conn_in = midi_in.connect(
        &in_port,
        "midir-forward",
        move |stamp, message, _| {
            sender.lock().unwrap().send(message);
            debug!("{}: {:?} (len = {})", stamp, message, message.len());
        },
        (),
    )?;

    std::thread::park();

    Ok(())
}

fn select_port<T: MidiIO>(midi_io: &T, descr: &str) -> Result<T::Port, Box<dyn Error>> {
    println!("Available {} ports:", descr);
    let midi_ports = midi_io.ports();
    for (i, p) in midi_ports.iter().enumerate() {
        println!("{}: {}", i, midi_io.port_name(p)?);
    }
    print!("Please select {} port: ", descr);
    stdout().flush()?;
    let mut input = String::new();
    stdin().read_line(&mut input)?;
    let port = midi_ports
        .get(input.trim().parse::<usize>()?)
        .ok_or("Invalid port number")?;
    Ok(port.clone())
}
