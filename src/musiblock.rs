use std::collections::HashMap;
use std::f32::consts::PI;
// use std::hash::Hash;
use log::debug;
use std::cell::RefCell;
use std::rc::Rc;

const DOUBLE_PI: f32 = 2.0 * PI;

pub struct Piano
{
    key_generators: HashMap<u8, Rc<RefCell<PianoGenerator>>>,  // 现在的目标键对应的发生器的指针
    queue_generators: Vec<Rc<RefCell<PianoGenerator>>>,  // 发生器队列
    alpha: f32,
}

impl Piano {
    pub fn new() -> Self {
        Piano {
            key_generators: HashMap::new(),
            queue_generators: Vec::new(),
            alpha: 0.33,
        }
    }

    // 处理MIDI消息
    pub fn handle_midi_message ( mut self, midi_message: &[u8]) {
        // 这里尚未管通道
        const NOTE_ON_MSG: u8 = 0x90;
        const NOTE_OFF_MSG: u8 = 0x80;
        if midi_message[0] == NOTE_ON_MSG {
            // 发送停止符
            match self.key_generators.get_mut(&midi_message[1]) {
                Some(correspond_generator) => {correspond_generator.borrow_mut().off();}
                None => {}
            }
            // 添加新发生器，并更改键字典映射
            let new_generator = Rc::new(RefCell::new(PianoGenerator::new(midi_message, self.alpha)));
            self.queue_generators.push(new_generator.clone());
            self.key_generators.insert(midi_message[1], new_generator);
        } else if midi_message[0] == NOTE_OFF_MSG {
            // 发送停止符
            if let Some(mut generator) = self.key_generators.remove(&midi_message[1]) {
                generator.borrow_mut().off();
            }
        }
    }

    pub fn tick(&mut self) -> f32 {
        let mut output: f32 = 0.0;
        // 清理空闲发生器，并求所有的振幅加和
        self.queue_generators.retain_mut(|p| {
            match p.borrow_mut().tick() {
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

struct PianoGenerator
{
    oscs: Vec<Box<dyn FnMut() -> f32>>,  // 里面就是相当于 tick() 的振荡器函数
    env_master: Envelope,  // 总 ENVELOP 函数
    current_sample: f32,
}

impl PianoGenerator {
    fn new(key: &[u8], alpha: f32) -> Self {
        // 计算泛音
        let mut oscs: Vec<Box<dyn FnMut() -> f32>> = Vec::new();
        let mut n: f32 = 1.0;
        // 计算对应频率（A4 = 440.0 Hz = 57）
        let freq = 440.0_f32 * 2.0_f32.powf((key[1] - 57) as f32 / 12.0);  // 440 * 2^((key-57)/12)
        loop {
            if freq * n > 20000.0 {
                break;
            }
            let volume = (1.0 / (n*n)) * (DOUBLE_PI * freq * n * alpha).sin(); // 求解能量收敛的波动方程
            // debug!("Acticvate Key - frq: {} , n: {}, vol: {}", freq * n, n, volume);
            oscs.push(Box::new(oscillator(44100.0, freq * n, volume)));
            n += 1.0;
        }
        // 生成主包络
        let env_master = Envelope::from(vec![
            Node {t:0.0, v: 1.0, curve: CurveType::Linear, if_hold: false},
            Node {t:0.5, v: 0.7, curve: CurveType::Linear, if_hold: true},
            Node {t:1.0, v: 0.0, curve: CurveType::Linear, if_hold: false},
        ]);
        Self {
            oscs,
            env_master,
            current_sample: 0.0,
        }
    }

    fn off(&mut self) {
        // 释放 hold，使其衰减（目前只能无脑release，之后考虑定向释放）
        self.env_master.release_hold();
    }

    fn tick(&mut self) -> Option<f32> {
        self.current_sample += 1.0 / 44100.0;
        let mut output: f32 = 0.0;
        for osc in &mut self.oscs {
            output += osc();
        }
        // 通过 env
        match self.env_master.tick() {
            Some(reduction) => Some(output * reduction),
            None => None,
        }
    }
}

// 振荡生成器
pub fn oscillator (
    sample_rate: f32,
    freq: f32,
    volume: f32
) -> impl FnMut() -> f32 {
    let mut t: f32 = 0.0;
    // 返回一个每次 tick 就发生值的“振荡器”闭包
    move || -> f32 {
        t = (t + 1.0) % sample_rate;
        (t * freq * DOUBLE_PI / sample_rate).sin() * volume
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
    t: f32,
    v: f32,
    curve: CurveType,
    if_hold: bool,
}

// 包络生成器
pub struct Envelope {
    nodes: Vec<Node>,
    current_time: f32,
    current_value: f32,
    in_hold: bool,
    current_index: usize,
}

use super::approx_eq;
// use std::cmp::Ordering;

impl Envelope {
    // ...
    
    fn new() -> Self {
        Self {
            nodes: Vec::new(),
            current_time: 0.0,
            current_value: 0.0,
            in_hold: false,
            current_index: 0,
        }
    }

    fn from(nodes: Vec<Node>) -> Self {
        let mut envelope = Self {
            nodes,
            current_time: 0.0,
            current_value: 0.0,
            in_hold: false,
            current_index: 0,
        };
        envelope.sort();
        envelope
    }

    fn tick(&mut self) -> Option<f32> {
        if !self.in_hold {
            self.current_time += 1.0/44100.0;
        } else {
            return Some(self.current_value);
        }
        // 边界检查，到达最后一个节点时返回 None
        if self.current_index == self.nodes.len() {return None;}
        // 当前的节点，若存在下一个节点，则伺机等待
        let node = self.nodes[self.current_index];
        let node_2 = self.nodes[self.current_index+1];
        // 判断并计算插值
        self.current_value = Self::interpolation(&[node, node_2], self.current_time - node.t);
        // 判断是切换下一个节点
        if approx_eq(self.current_time, node_2.t) {
            if node_2.if_hold {self.in_hold = true;}
            self.current_index += 1;
        }

        Some(self.current_value)
    }

    // 由外部控制释放 hold
    pub fn release_hold(&mut self) {
        self.in_hold = false;
    }

    fn push(&mut self, node: Node) {
        self.nodes.push(node);
        self.sort();
    }

    fn sort(&mut self) {
        self.nodes.sort_by(|a, b| b.t.partial_cmp(&a.t).unwrap());
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
