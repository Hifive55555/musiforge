use std::ops::{Index, IndexMut};
use std::sync::{mpsc, Arc};
use std::collections::{HashMap, HashSet};

use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(Uuid);

impl BlockId {
    pub fn new() -> Self {
        BlockId(Uuid::new_v4())
    }

    pub fn port(&self, port: usize) -> Port {
        Port { block_id: *self, port }
    }
}

#[derive(Clone, Copy)]
pub struct Time {
    sample_rate: u32,
    sample: u64,
}

impl Time {
    pub const RATE_44100: Time = Time {
        sample_rate: 44100,
        sample: 0,
    };

    pub const RATE_48000: Time = Time {
        sample_rate: 48000,
        sample: 0,
    };

    pub fn new(sample_rate: u32) -> Self {
        assert!(sample_rate > 0, "Sample rate must be greater than 0");

        Time {
            sample_rate,
            sample: 0,
        }
    }

    pub fn tick(&mut self) {
        self.sample += 1;
    }

    pub fn tick_by_buffer(&mut self, buffer_size: u64, num_channels: usize) {
        self.sample += buffer_size / num_channels as u64;  // chunk
    }

    pub fn sample(&self) -> u64 {
        self.sample
    }

    pub fn as_secs_f32(&self) -> f32 {
        self.sample as f32 / self.sample_rate as f32
    }

    pub fn get_phase_by_freq(&self, freq: f32) -> f32 {
        const TWO_PI: f32 = 6.283185307179586;
        TWO_PI * self.as_secs_f32() * freq
    }
}

impl Default for Time {
    fn default() -> Self {
        Time::RATE_48000
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Port {
    pub block_id: BlockId,
    pub port: usize,
}

#[derive(Clone)]
pub struct IOData(Vec<Vec<f32>>);

impl Index<usize> for IOData {
    type Output = Vec<f32>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl IndexMut<usize> for IOData {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}

impl IOData {
    pub fn new(port_len: usize, buffer_size: usize) -> Self {
        IOData(vec![vec![0.0; buffer_size]; port_len])
    }

    pub fn from_raw(data: Vec<Vec<f32>>) -> Self {
        IOData(data)
    }

    pub fn port_len(&self) -> usize {
        self.0.len()
    }

    pub fn buffer_size(&self) -> usize {
        self.0.first().unwrap().len()
    }
}

impl IntoIterator for IOData {
    type Item = Vec<f32>;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

pub struct Block {
    pub(crate) process: Arc<dyn Fn(Time, &IOData, &mut IOData, usize) + Send + Sync>,
    pub port_len: usize,
    pub inputs: HashMap<BlockId, HashSet<(usize, usize)>>,  // block_id -> (pre_port, self_port)
    pub(crate) data: IOData,
    pub(crate) d_in: usize,
    pub(crate) d_in_cur: usize,
}

unsafe impl Sync for Block {}

impl Block {
    pub fn new(
        func: impl Fn(Time, &IOData, &mut IOData, usize) + Send + Sync + 'static,
        port_len: usize,
        buffer_size: usize,
    ) -> Self {
        Block {
            process: Arc::new(func),
            port_len,
            inputs: HashMap::new(),
            data: IOData::new(port_len, buffer_size),
            d_in: 0,
            d_in_cur: 0,
        }
    }
}

pub trait BlockMarker {}
pub struct WithInput;
pub struct WithoutInput;
impl BlockMarker for WithInput {}
impl BlockMarker for WithoutInput {}

pub trait IntoBlock<M: BlockMarker> {
    fn into_block(self) -> Block;
}

impl<F> IntoBlock<WithInput> for F
where
    F: Fn(Time, &IOData, &mut IOData, usize) + Send + Sync + 'static,
{
    fn into_block(self) -> Block {
        Block::new(self, 256, 512)
    }
}
impl<F> IntoBlock<WithoutInput> for F
where
    F: Fn(Time, &mut IOData, usize) + Send + Sync + 'static,
{
    fn into_block(self) -> Block {
        let func = move |time: Time, inputs: &IOData, outputs: &mut IOData, num_channels: usize| {
            self(time, outputs, num_channels)
        };
        Block::new(func, 256, 512)
    }
}


#[macro_export]
macro_rules! frame_block {
    ($block_fn:ident) => {
        |mut time: Time, inputs: &IOData, outputs: &mut IOData, num_channels: usize| {
            let buffer_size = inputs.buffer_size();
            let num_frames = buffer_size / num_channels;
            let port_count = inputs.port_len();

            // 创建帧输入/输出的临时存储
            let mut frame_inputs = IOData::new(port_count, num_channels);
            let mut frame_outputs = IOData::new(outputs.port_len(), num_channels);
            
            // 按帧处理数据
            for frame_idx in 0..num_frames {
                let start = frame_idx * num_channels;
                let end = start + num_channels;

                // 构建当前帧输入 - 直接使用切片引用
                for port_idx in 0..port_count {
                    frame_inputs[port_idx].copy_from_slice(&inputs[port_idx][start..end]);
                }

                // 处理当前帧
                $block_fn(time, &frame_inputs, &mut frame_outputs, num_channels);

                // 直接写入输出 - 避免中间缓冲
                for port_idx in 0..outputs.port_len() {
                    outputs[port_idx][start..end].copy_from_slice(&frame_outputs[port_idx]);
                    frame_outputs[port_idx].fill(0.0); // 重置输出缓冲区
                }

                time.tick();
            }
        }
    };
}
