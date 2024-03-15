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
            let correspond_generator = self.key_generators.get_mut(&midi_message[1]).unwrap();
            correspond_generator.borrow_mut().off();
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
    env: Box<dyn FnMut() -> Option<f32>>,  // 总 ENV 函数
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
        // 生成 ENV 函数
        let nodes = vec![
            Node{t: 0.0, v: 1.0, style: Curves::Linear},
            Node{t: 1.0, v: 0.0, style: Curves::Linear}];
        let env = Box::new(envelop(nodes, 44100.0));
        Self {
            oscs,
            env,
            current_sample: 0.0,
        }
    }

    fn off(&mut self) {}

    fn tick(&mut self) -> Option<f32> {
        self.current_sample += 1.0 / 44100.0;
        let mut output: f32 = 0.0;
        for osc in &mut self.oscs {
            output += osc();
        }
        // 通过 envelop() 计算，不过这个用法有点炸裂。我在想要不要不要存函数而是结构体
        match self.env.as_mut()() {
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

// 包络生成器（就是一个函数，唯一棘手的需要知道现在的“t”的值）
// 关键点 (t,v)，曲线类型 curves
// 目前先搞线性的

pub enum Curves {
    Linear,
}

pub struct Node {
    t: f32,
    v: f32,
    style: Curves,
}

use std::cmp::Ordering;

/// A Function that return a closure of ENVELOP
/// 
/// # Examples
/// 
/// ```
/// let nodes = vec![
///     Node{t: 0.0, v: 1.0, style: Curves::Linear},
///     Node{t: 1.0, v: 0.0, style: Curves::Linear}]；
/// let env = envelop(nodes, 44100.0)
/// ```
pub fn envelop (
    mut nodes: Vec<Node>,
    sample_rate: f32
) -> impl FnMut() -> Option<f32> {
    let mut t = 0.0_f32;

    nodes.sort_by(|a, b| match b.t.partial_cmp(&a.t) {
        Some(Ordering::Greater) => Ordering::Greater,
        _ => Ordering::Less,
    });
    // 计算分段函数（不知必不必要）
    let mut func_vec = Vec::new();
    let node_len = nodes.len() - 1;
    for i in 0..node_len {
        match nodes[i].style {
            Curves::Linear => {
                let (v1, t1, v2, t2) = (nodes[i].v, nodes[i].t, nodes[i+1].v, nodes[i+1].t);
                let func_add = move |t: f32| {
                    // v = dv/dt * (t-t1) + v1
                    Some(v1 + (t - t1) * (v1 - v2)/(t1 - t2))
                };
                func_vec.push(Box::new(func_add));
            }
        }
    }
    // 将 t 重新搞一个列表
    let t_vec: Vec<f32> = nodes.iter().map(|node| node.t).collect();
    // 返回一个分段函数闭包
    move || -> Option<f32> {
        t = (t + 1.0) % sample_rate;
        let mut return_v = None;
        // 分段检测
        for i in 0..node_len {
            if t_vec[i] <= t && t < t_vec[i+1] {
                return_v = func_vec[i](t);
                break;
            }
        }
        return_v
    }
}
