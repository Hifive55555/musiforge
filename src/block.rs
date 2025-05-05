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

    pub fn with_port(&self, port: usize) -> Port {
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

    pub fn tick_by_buffer(&mut self, buffer_size: u64) {
        self.sample += buffer_size / 2;  // chunk
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

    pub fn port_len(&self) -> usize {
        self.0.len()
    }

    pub fn buffer_size(&self) -> usize {
        self.0.first().unwrap().len()
    }
}

pub struct Block {
    pub(crate) process: Arc<dyn Fn(Time, &IOData, &mut IOData) + Send + Sync>,
    pub port_len: usize,
    pub inputs: HashMap<BlockId, HashSet<(usize, usize)>>,  // block_id -> (pre_port, self_port)
    pub(crate) data: IOData,
    pub(crate) d_in: usize,
    pub(crate) d_in_cur: usize,
}

unsafe impl Sync for Block {}

impl Block {
    pub fn new(
        func: impl Fn(Time, &IOData, &mut IOData) + Send + Sync + 'static,
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

impl<F: Fn(Time, &IOData, &mut IOData) + Send + Sync + 'static> From<F> for Block {
    fn from(value: F) -> Self {
        Block::new(value, 256, 512)
    }
}

pub trait BlockExt {
}

impl<T> BlockExt for T
where
    T: Into<Block> + Clone,
{

}
