# 预期使用方法

## 作为rust生态库的musiforge

- 可以融合进rust程序开发

```rust
fn osc(time: Time, _inputs: &IOData, outputs: &mut IOData) {
    let freq = 440.0;
    let mut time = time;
    for frame in outputs[0].chunks_mut(2) {
        let val = time.get_phase_by_freq(freq).sin();
        for sample in frame.iter_mut() {
            *sample = val;
        }
        time.tick();
    }
}

fn filter(_time: Time, inputs: &IOData, outputs: &mut IOData) {
    for i in 0..inputs.len() {
        outputs[0][i] = inputs[0][i];
    }
}

let builder = GraphFlowBuilder {
    sample_rate: 48000,
    buffer_size: 512,
};
let mut gf = builder.build();

let osc_id = gf.add_block(osc);
let filter_id = gf.add_block(filter);

gf.connect(osc_id.with_port(0), filter_id.with_port(0));
gf.to_output(filter_id.with_port(0));

let stream = create_stream(10240, 48000, move |data: &mut [f32], _| {
    gf.run(data.len() as u32, data);
});
stream();
```

### 动态修改音频块及其输出

将 `gf` 作为程序主生命周期句柄.

```rust
// 动态添加音频块
gf.add_block(...);

// 动态修改某音频块的输出
gf.connect(block_id.with_port(0), other_id.with_port(0));
gf.disconnect(block_id.with_port(0), other_id.with_port(0));

// 动态删除某音频块
gf.remove_block(block_id).unwrap();
```

### 动态保存和加载音频块

```rust
// 保存
fn block(_time: Time, inputs: &[Vec<f32>], outputs: &mut [Vec<f32>]) {
    ...
}
let block_binary = bincode::serialize(&block).unwrap();

// 加载
let block: BlockFn = bincode::deserialize(&data).unwrap();
```

## 图流的实现

### Block 保存节点配置

```rust
pub struct IOData(Vec<Vec<f32>>);

pub struct Block {
    pub process: Box<dyn Fn(Time, &IOData, &mut IOData) + Send + Sync>,
    pub port_len: usize,
    // pub inputs: HashSet<Port>,
    data: IOData,  // 缓存字段
    d_in: usize,  // 入度
    d_in_cur: usize,  // 当前的计算入度
}
```

### GraphFlow 构建有向无权图

1. `blocks: HashMap<BlockId, Block>` 存储了所有节点配置, 并为每个节点分配一个唯一 id.
2. `graph: DiGraph<f32, ()>` 存储了该图.
3. `thread_pool: ThreadPool` 维护了一个用于计算的线程池.

### process()

1. 获取节点的拓扑排序
2. 从队首开始, 检查该节点的入度是否为 0, 若是, 则向线程池加入新的任务: `type Job = Box<dyn FnOnce(&mut IOData) + Send + 'static>`
3. 当任务结束后，会向主线程发送信号
