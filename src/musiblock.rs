use std::collections::HashMap;
use std::f32::consts::PI;
// use std::hash::Hash;
use log::{debug, error};
// use std::cell::RefCell;
// use std::rc::Rc;
use std::sync::{Arc, Mutex};

use super::{SAMPLE_RATE, db_to_vol};
const DOUBLE_PI: f32 = 2.0 * PI;

pub struct Piano
{
    key_generators: HashMap<u8, Arc<Mutex<PianoGenerator>>>,  // 现在的目标键对应的发生器的指针
    queue_generators: Vec<Arc<Mutex<PianoGenerator>>>,  // 发生器队列
    alpha: f32,
    limiter_master: Limiter,
}

impl Piano {
    pub fn new() -> Self {
        debug!("初始化 Pinao");
        Piano {
            key_generators: HashMap::new(),
            queue_generators: Vec::new(),
            alpha: 0.33,
            limiter_master: Limiter::new(),
        }
    }

    // 处理MIDI消息
    pub fn handle_midi_message (&mut self, midi_message: &[u8]) {
        // 这里尚未管通道
        const NOTE_ON_MSG: u8 = 0x90;
        const NOTE_OFF_MSG: u8 = 0x80;
        
        if midi_message[0] == NOTE_ON_MSG {
            debug!("发送了 Midi - NOTE ON 信号 {:?}", midi_message);
            // 发送停止符
            match self.key_generators.get_mut(&midi_message[1]) {
                Some(correspond_generator) => {correspond_generator.lock().unwrap().off();}
                None => {}
            }
            // 添加新发生器，并更改键字典映射
            let new_generator = Arc::new(Mutex::new(
                PianoGenerator::new(midi_message, self.alpha, midi_message[2] as f32 / 12.7)
            ));
            self.queue_generators.push(new_generator.clone());
            self.key_generators.insert(midi_message[1], new_generator);

        } else if midi_message[0] == NOTE_OFF_MSG {
            debug!("发送 Midi - NOTE OFF 信号 {:?}", midi_message);
            // 发送停止符
            if let Some(generator) = self.key_generators.remove(&midi_message[1]) {
                generator.lock().unwrap().off();
            } else {
                error!("未找到触发的键 {}", midi_message[1])
            }
        }
        debug!("当前发生器队列数 {}", self.queue_generators.len());
    }

    pub fn tick(&mut self) -> f32 {
        let mut output: f32 = 0.0;
        // 清理空闲发生器，并求所有的振幅加和
        self.queue_generators.retain_mut(|p| {
            match p.lock().unwrap().tick() {
                Some(value) => {
                    output += value;
                    true
                },
                None => {
                    debug!("一个 弦 结束了其生命周期");
                    false
                },
            }
        });
        // self.limiter_master.tick(output)
        clip(output)
    }
}

struct PianoGenerator
{
    oscs: Vec<Oscillator>,
    env_master: Envelope,  // 总 ENVELOP 函数
    t: f32,
    vol: f32,
}

impl PianoGenerator {
    fn new(midi_message: &[u8], alpha: f32, db: f32) -> Self {
        // 计算泛音
        let mut oscs = Vec::new();
        let mut n: f32 = 1.0;
        // 计算对应频率（A4 = 440.0 Hz = 69）
        let freq = 440.0_f32 * 2.0_f32.powf((midi_message[1] as i8 - 0x45) as f32 / 12.0);  // 440 * 2^((key-69)/12)
        loop {
            if freq * n > 20000.0 {
                break;
            }
            let partial_volume = (1.0 / (n*n)) * (DOUBLE_PI * freq * n * alpha).sin(); // 求解能量收敛的波动方程
            // debug!("Acticvate Key - frq: {} , n: {}, vol: {}", freq * n, n, partial_volume);
            oscs.push(Oscillator::new(freq * n, partial_volume));
            n += 1.0;
        }
        // 生成主包络
        let env_master = Envelope::from(vec![
            Node {t:0.0, v: 1.0, curve: CurveType::Linear, if_hold: false},
            Node {t:0.5, v: 0.7, curve: CurveType::Linear, if_hold: true},
            Node {t:1.0, v: 0.0, curve: CurveType::Linear, if_hold: false},
        ]);
        // 计算按键 db 对应的 vol
        let vol = db_to_vol(db);
        debug!("初始化 PianoGenerator, 基频 {} Hz, 按压响度 {} db, 转换电平 {} V", freq, db, vol);
        Self {
            oscs,
            env_master,
            t: 0.0,
            vol,
        }
    }

    fn off(&mut self) {
        // 释放 hold，使其衰减（目前只能无脑release，之后考虑定向释放）
        self.env_master.release_hold();
    }

    fn tick(&mut self) -> Option<f32> {
        self.t += 1.0 / SAMPLE_RATE;
        let mut output: f32 = 0.0;
        for osc in &mut self.oscs {
            output += osc.tick();
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

// 被迫做成结构体
struct Oscillator {
    t: f32,
    pub freq: f32,
    pub volume: f32,
}

impl Oscillator {
    fn new(freq: f32, volume: f32) -> Self {
        Self {freq, t: 0.0, volume}
    }

    fn tick(&mut self) -> f32 {
        self.t = (self.t + 1.0) % SAMPLE_RATE;
        (self.t * self.freq * DOUBLE_PI / SAMPLE_RATE).sin() * self.volume * 0.1
    }
}

#[derive(Copy, Clone)]
#[derive(Debug)]
pub enum CurveType {
    Linear,
    // 下面的以后做，但是意味着 Envelop 的架构要整体改，目前只能邻近插值
    Newton,
    Hermite,
    CubicSpline,
}

#[derive(Copy, Clone)]
#[derive(Debug)]
pub struct Node {
    pub t: f32,
    pub v: f32,
    pub curve: CurveType,
    pub if_hold: bool,
}

// 包络生成器
pub struct Envelope {
    nodes: Vec<Node>,
    current_time: f32,
    current_value: f32,
    current_index: usize,

    in_hold: bool,
    ready_release: bool,  // 待释放队列（因为无法定向释放，故只需要存储带释放的数量）
}

use super::approx_eq;
// use std::cmp::Ordering;

impl Envelope {
    // ...
    
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            current_time: 0.0,
            current_value: 0.0,
            current_index: 0,

            in_hold: false,
            ready_release: false,
        }
    }

    pub fn from(nodes: Vec<Node>) -> Self {
        let mut envelope = Self {
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
            self.current_time += 1.0/SAMPLE_RATE;
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

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::init_logger;

    #[test]
        fn it_works() {
            init_logger();
            let mut p = Piano::new();
            p.handle_midi_message(&[0x90, 0x44, 0x90]);
        }
}