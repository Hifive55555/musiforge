# Musiforge

## 目标

一个音乐制造原型机，希望实现以下目标：

1. 前后端分离。后端框架为**生产图流**。
2. 提供与前端框架交互的接口。
3. 提供同步的音乐创作体验。

### 生产图流

我们将一个音乐创作过程转化为一个图流生产线，这启发于 Bitwig Studio，抛弃传统的机架模式（卷轴、混音台、通道）。我们发现所有的生产概念都可以用一种图流代替：

1. **Pattern**：模板式图流（发生音频信号）
2. **Envelop**：模板式图流（发生控制信号）
3. **The Mixer**：提供**分配器**的图流，且终端信号接入声卡入口
4. **Piano Roll**：这其实是一个UI概念，其提供对Pattern的接口
5. **Browser**：编码了模板式图流的实例化时刻
6. Bitwig 提供的对 MIDI 音符的灵活编辑手段也是基于将控制信号视作可操控的信号

上述的音频信号和控制信号在我们看来是同一种东西，也就是信号流。

### 信号流

在生产图流中，信号流作为唯一的传输介质，提供了对信号的统一编码。

- **MIDI 信号**：由三个字节构成，第一个字节为 STATUS byte，表示信号类型或信道的改变，其后的信息表示音高和速率（力度）等。特殊的编码例如 NOTE ON、NOTE OFF、All Notes Off 等。设计 MIDI 协议的最大利家还是生产 MIDI 乐器的商家，孤以为对于实验型的 Musiforge 只需要将信号按字节量分类即可。所有字节量对应的接口就可以连接，对于稀疏的 MIDI 信号，通常实时触发即可。
- **音频信号**，不同采样率和比特率的音频信号的传输规格都不同，通常采用成组发送，再在接收端依据采样率连接即可。
- **相互转换**：将稀疏信号转换为稠密信号的方式就是对其进行“填补采样空缺”。理论上在语法层面抽象成组发送的概念就可以让使用者不关心这二者的区别。将稠密信号可以“直译”为稀疏信号，只相当于提前接收了一组信号不过延迟处理罢。
  > 但通常需要整体提前处理稠密信号形成一个音频缓冲区，如何解决稀疏信号的实时性问题仍然有待解决。

### 后端语法

通常后端创作不关心具体的生产技术细节，参考 Typst，开发一套 Musiforge 创作语法，该语法可以实现：

1. **开发插件**：现有的插件大多都基于日本的 Steinberg 公司提出的 VST 协议，提供了新窗口句柄与 MIDI 信号、音频信号的传输通道。而 Musiforge 生态采用前后端分离和信号统一化的设计模式，因此提供一种新的插件开发模式。此时已经不叫插件了，因为这样的设计模式不依赖于插件的编译与动态链接，而是叫“import”更为合适。也就是新的插件模式就相当于动态解释引用的代码。
2. **图流编写**：可以简明地表征一个图流，并灵活地嵌入到或链接到其他的图流中。
3. **实例序列**：可以简明表征一个基于时刻的实例序列，可谓是时间的艺术。

### 前后端分离

参考 Tauri，实现前后端通信需要开发前端库与后端库，例如开发一个网页应用程序。

## 开发进度

- [ ] 图流开发
  - [ ] 基本图流
  - [ ] 信号流
  - [ ] 模板式图流
  - [ ] 发生器
  - [ ] 分配器
- [ ] 插件模式
- [ ] 同步模式
- [ ] 语法解析器（bindings）
- [ ] 前端开发

### 小目标

- [ ] 一个简单的振荡器（Oscillator） + 合并节点（Merger） + 监听器（Listener） + Pattern

## 技术栈

相比于开发一个新的后端语法，一个更方便的做法是采用 Python 作为脚本语言。

借鉴 safetensors，原生项目主体就在根目录中开发，在 `bindings/` 中开发与其他脚本语言的链接库。例如在 `python/` 中，本质是一个 Rust 项目，其引用了 `../../musiforge`，并使用 `pyo3` 开发 Python 库，最终在 `py_src/musiforge/` 中开发 Python 库。

主项目采用 `cpal` 作为与音频硬件交互的接口，`midir` 作为 MIDI 信号解析的底层库。

## 快速上手

在 `bin/` 中给出了两个示例，其中 `song.rs` 为播放一段乐曲，`synth.rs` 为连接外部 MIDI 信道通信。

首先导入（之后再弄 `prelude`）

```rust
use musiforge::{
    create_stream, init_logger,
    musiblock::{listen, AdditiveSynth, AdditiveUnit, MidiRack},
    ClockTime,
};
```

一切开始于创建一个音频流：

```rust
let gf = create_stream(1024, data_callback());
gf();
std::thread::park();
```

你可以用任意方式阻断进程结束。其中 `gf` 为一个函数，调用它就可以启动整个音频流。为了创建它，你需要传输两个参数，一个是 `buffer_size`，另一个是 `data_callback`，分别表示缓存大小和每次对缓存的修改。

`data_callback()` 返回对缓存修改的闭包，并携带一个计时器 `ClockTime`：

```rust
fn data_callback() -> impl FnMut(&mut [f32], &cpal::OutputCallbackInfo) + Send + 'static {
    let mut global_clock: ClockTime<SAMPLE_RATE> = ClockTime::new();
    let mut graph_flow = create_graph_flow();

    move |data: &mut [f32], _info: &cpal::OutputCallbackInfo| {
        for frame in data.chunks_mut(2) {
            global_clock.tick();
            let vol = graph_flow();
            for sample in frame.iter_mut() {
                *sample = vol;
            }
        }
    }
}
```

它对每一帧的数据进行修改。其中 `graph_flow` 是音频生成的图流，每次调用它就生成新一帧的数据，它通过 `create_graph_flow` 生成。

```rust
fn create_graph_flow() -> impl FnMut() -> f32 {
    let additive_synth = Arc::new(Mutex::new(AdditiveSynth::new(5, SAMPLE_RATE as f32)));
    let synth_rack = Arc::new(Mutex::new(MidiRack::new(additive_synth)));
    let synth_rack_2 = Arc::clone(&synth_rack);
    std::thread::spawn(move || {
        listen::<SAMPLE_RATE, AdditiveSynth, AdditiveUnit>(synth_rack_2).unwrap();
    });

    move || -> f32 { synth_rack.lock().unwrap().tick() }
}
```

上面是一个通过 `listen` 监听外部 MIDI 信号并发送给 MIDI 机架 `synth_rack` 的例子。你可以任意编辑这个函数，只要它最后返回的是一个 `FnMut() -> f32` 的闭包即可。

## 相关项目

- Ai 辅助创作（可能）
