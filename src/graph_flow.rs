use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::sync::mpsc;
use std::thread;

use petgraph::graph::DiGraph;
use petgraph::algo::toposort;

use crate::block::*;


pub struct  GraphFlowBuilder {
    pub sample_rate: u32,
    pub buffer_size: u32,
}

impl GraphFlowBuilder {
    pub fn build(&self) -> GraphFlow {
        let (result_tx, result_rx) = mpsc::channel();

        GraphFlow {
            time: Time::new(self.sample_rate),
            blocks: HashMap::new(),

            buffer_size: self.buffer_size,
            outputs: HashSet::new(),

            graph: DiGraph::new(),
            block_map: HashMap::new(),
            node_map: HashMap::new(),
            thread_pool: ThreadPool::new(20, result_tx.clone()),
            result_rx,
        }
    }
}

pub struct GraphFlow {
    time: Time,
    blocks: HashMap<BlockId, Block>,
    buffer_size: u32,
    outputs: HashSet<BlockId>,
    graph: DiGraph<f32, ()>,
    block_map: HashMap<BlockId, petgraph::graph::NodeIndex>,
    node_map: HashMap<petgraph::graph::NodeIndex, BlockId>,
    thread_pool: ThreadPool,
    result_rx: mpsc::Receiver<ResultData>,
}

impl GraphFlow {
    pub fn to_output(&mut self, from: Port) {
        self.outputs.insert(from.block_id);
    }

    pub fn add_block(&mut self, block: impl Into<Block>) -> BlockId {
        let id = BlockId::new();
        let block = block.into();
        // add to blocks
        self.blocks.insert(id, block);
        // add to graph
        let node = self.graph.add_node(1.0);
        self.block_map.insert(id, node);
        self.node_map.insert(node, id);

        id
    }

    pub fn get_block(&self, block_id: &BlockId) -> &Block {
        self.blocks.get(block_id).unwrap()
    }

    pub fn get_block_mut(&mut self, block_id: &BlockId) -> &mut Block {
        self.blocks.get_mut(block_id).unwrap()
    }

    pub fn connect(&mut self, from: Port, to: Port) {
        // 更新图结构
        let from_node = self.block_map[&from.block_id];
        let to_node = self.block_map[&to.block_id];
        self.graph.add_edge(from_node, to_node, ());

        // 修改 Block
        let to_block = self.get_block_mut(&to.block_id);
        to_block.inputs.entry(from.block_id).and_modify(|port_match| {
            port_match.insert((from.port, to.port));
        }).or_insert({
            let mut set = HashSet::new();
            set.insert((from.port, to.port));
            set
        });
        to_block.d_in += 1;
    }

    fn update_block_inputs(&mut self, block_id: &BlockId, node_id: petgraph::graph::NodeIndex) {
        let mut inputs = IOData::new(128, self.buffer_size as usize);
        let block = self.get_block(block_id);

        for pre_node in self.graph.neighbors_directed(node_id, petgraph::Direction::Incoming) {
            let pre_block_id = self.node_map.get(&pre_node).unwrap();
            let pre_block = self.get_block(pre_block_id);
            let pre_ports = block.inputs.get(pre_block_id).unwrap();
            for (from_port, to_port) in pre_ports {
                inputs[*to_port].iter_mut().zip(pre_block.data[*from_port].iter()).for_each(|(input, from_data)| *input = *from_data);
            }
        }

        let block = self.get_block_mut(block_id);
        block.data = inputs;
    }

    fn process_node(&mut self, node_id: petgraph::graph::NodeIndex) {
        let block_id = *self.node_map.get(&node_id).unwrap();
        self.update_block_inputs(&block_id, node_id);

        let block_data = self.get_block(&block_id).data.clone();
        let block_process = Arc::clone(&self.get_block(&block_id).process);
        let time = self.time.clone();
        let mut outputs = IOData::new(128, self.buffer_size as usize);

        self.thread_pool.execute(move || {
            (block_process)(time, &block_data, &mut outputs);
            (block_id, outputs)
        });
    }

    fn reset_block(&mut self) {
        for (_, block) in self.blocks.iter_mut() {
            block.d_in_cur = block.d_in;
        }
    }

    fn process(&mut self) {
        if let Ok(sorted) = toposort(&self.graph, None) {
            self.process_node(sorted[0]);

            let mut i = 1;
            while i < sorted.len() {
                if let Ok(ResultData {block_id, result_data}) = self.result_rx.recv() {
                    // 更新节点的数据
                    let block = self.get_block_mut(&block_id);
                    block.data = result_data;
                    let node_id = self.block_map.get(&block_id).unwrap();
                    let post_nodes = self.graph.neighbors_directed(*node_id, petgraph::Direction::Outgoing).collect::<Vec<_>>();
                    for post_node in post_nodes.iter() {
                        let post_block_id = *self.node_map.get(&post_node).unwrap();
                        let post_block = self.get_block_mut(&post_block_id);
                        post_block.d_in_cur -= 1;
                    }

                    // 持续判断下一个队首是否可以运行
                    while i < sorted.len() {
                        let next_node_id = sorted[i];
                        let next_block_id = self.node_map.get(&next_node_id).unwrap();
                        let next_block = self.get_block(next_block_id);
                        if next_block.d_in_cur == 0 {
                            self.process_node(next_node_id);
                            i += 1;
                        } else {
                            break;
                        }
                    }
                }
            }
        }
    }

    pub fn run(&mut self, buffer_size: u32, output: &mut [f32]) {
        self.buffer_size = buffer_size;

        self.reset_block();
        self.time.tick_by_buffer(buffer_size as u64);
        self.process();

        // 收集输出
        for block_id in self.outputs.iter() {
            let block = self.get_block(block_id);
            output.iter_mut().zip(block.data[0].iter()).for_each(|(sample, block_sample)| *sample += *block_sample);
        }
    }
}

impl std::fmt::Debug for GraphFlow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "* Graph Flow\n{:?}", self.graph)
    }
}

struct ResultData {
    pub block_id: BlockId,
    pub result_data: IOData,
}

type Job = Box<dyn FnOnce() -> (BlockId, IOData) + Send + 'static>;

struct ThreadPool {
    workers: Vec<Worker>,
    sender: mpsc::Sender<Job>,
}

impl ThreadPool {
    fn new(size: usize, result_sender: mpsc::Sender<ResultData>) -> ThreadPool {
        assert!(size > 0);

        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            workers.push(Worker::new(
                id,
                Arc::clone(&receiver),
                result_sender.clone()),
            );
        }

        ThreadPool { workers, sender }
    }

    fn execute<F>(&self, f: F)
    where
        F: FnOnce() -> (BlockId, IOData) + Send + 'static,
    {
        let job = Box::new(f);
        self.sender.send(job).unwrap();
    }
}

struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
    result_sender: mpsc::Sender<ResultData>,
}

impl Worker {
    fn new(
        id: usize,
        receiver: Arc<Mutex<mpsc::Receiver<Job>>>,
        result_sender: mpsc::Sender<ResultData>,
    ) -> Worker {
        let sender_clone = result_sender.clone();
        let thread = thread::spawn(move || loop {
            let job = receiver.lock().unwrap().recv();

            match job {
                Ok(job) => {
                    // println!("Worker {} got a job; executing.", id);
                    let (block_id, result_data) = job();
                    sender_clone.send(ResultData {
                        block_id,
                        result_data,
                    }).unwrap();
                }
                Err(_) => {
                    // println!("Worker {} disconnected; shutting down.", id);
                    break;
                }
            }
        });

        Worker {
            id,
            thread: Some(thread),
            result_sender,
        }
    }
}
