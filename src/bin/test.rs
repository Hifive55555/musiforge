use rustfft::{FftPlanner, num_complex::Complex};
use rustfft::FftDirection;

struct ConvolveReverb {
    ir_data: Vec<f32>,
    input_buffer: Vec<f32>,
    output_buffer: Vec<f32>,
    sub_filters_fft: Vec<Vec<Complex<f32>>>,
    freq_delay_line: Vec<Vec<Complex<f32>>>,
    buffer_size: usize,
    planner: FftPlanner<f32>,
    sub_filter_size: usize,
    fft_size: usize,
    input_write_pointer: usize,
    output_read_pointer: usize,
    counter: usize,
}

impl ConvolveReverb {
    fn new(buffer_size: usize, sub_filter_size: usize) -> Self {
        let fft_size = buffer_size.next_power_of_two() * 2;

        ConvolveReverb {
            ir_data: Vec::new(),
            input_buffer: vec![0.0; buffer_size],
            output_buffer: vec![0.0; buffer_size],
            sub_filters_fft: Vec::new(),
            freq_delay_line: Vec::new(),
            buffer_size,
            planner: FftPlanner::<f32>::new(),
            sub_filter_size,
            fft_size,
            input_write_pointer: 0,
            output_read_pointer: 0,
            counter: 0,
        }
    }

    fn prepare(&mut self, ir_data: Vec<f32>) {
        self.ir_data = ir_data;
        let num_sub_filters = (self.ir_data.len() + self.buffer_size - 1) / self.buffer_size;
        let fft = self.planner.plan_fft_forward(self.fft_size);

        self.sub_filters_fft = Vec::with_capacity(num_sub_filters);
        self.freq_delay_line = Vec::with_capacity(num_sub_filters);

        for i in 0..num_sub_filters {
            let start = i * self.buffer_size;
            let end = (i + 1) * self.buffer_size;
            let mut sub_filter = vec![Complex { re: 0.0, im: 0.0 }; self.fft_size];
            for j in 0..self.buffer_size {
                sub_filter[j] = if start + j < self.ir_data.len() {
                    Complex { re: self.ir_data[start + j], im: 0.0 }
                } else {
                    Complex { re: 0.0, im: 0.0 }
                };
            }
            fft.process(&mut sub_filter);
            self.sub_filters_fft.push(sub_filter);
            self.freq_delay_line.push(vec![Complex { re: 0.0, im: 0.0 }; self.fft_size]);
        }
    }

    fn process_sample(&mut self, sample: f32) -> f32 {
        self.input_buffer[self.input_write_pointer] = sample;
        self.input_write_pointer = (self.input_write_pointer + 1) % self.buffer_size;

        let output = self.output_buffer[self.output_read_pointer];
        self.output_read_pointer = (self.output_read_pointer + 1) % self.buffer_size;

        self.counter += 1;
        if self.counter >= self.buffer_size {
            self.process_block();
            self.counter = 0;
        }

        output
    }

    fn process_block(&mut self) {
        let mut input_buffer = vec![Complex { re: 0.0, im: 0.0 }; self.fft_size];
        let mut output_buffer = vec![Complex { re: 0.0, im: 0.0 }; self.fft_size];
        let mut fft_summing = vec![Complex { re: 0.0, im: 0.0 }; self.fft_size / 2 + 1];

        // 将输入缓冲区的数据复制到滑动块
        for i in 0..self.buffer_size {
            input_buffer[i] = Complex { re: self.input_buffer[i], im: 0.0 };
        }

        // 计算输入缓冲区的 FFT
        let fft = self.planner.plan_fft_forward(self.fft_size);
        fft.process(&mut input_buffer);

        // 频域相乘
        for i in 0..self.sub_filters_fft.len() {
            for j in 0..self.fft_size / 2 + 1 {
                self.freq_delay_line[i][j] = input_buffer[j] * self.sub_filters_fft[i][j];
            }
        }

        // 求和
        for i in 0..self.sub_filters_fft.len() {
            for j in 0..self.fft_size / 2 + 1 {
                fft_summing[j] += self.freq_delay_line[i][j];
            }
        }

        // 计算逆 FFT
        let ifft = self.planner.plan_fft_inverse(self.fft_size);
        // ifft.process(&mut fft_summing, &mut output_buffer);

        // 归一化逆 FFT 结果
        for i in 0..self.fft_size {
            output_buffer[i] /= self.fft_size as f32;
        }

        // 将结果复制到输出缓冲区
        for i in 0..self.buffer_size {
            self.output_buffer[i] = output_buffer[self.fft_size - self.buffer_size + i].re;
        }
    }
}

fn main() {
    // 示例输入和滤波器
    let inputs: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 0.0];
    let h: Vec<f32> = vec![0.1, 0.2, 0.3];
    let buffer_size = 2; // 每个输入块的大小
    let sub_filter_size = 1024; // 子滤波器的大小

    // 初始化卷积器
    let mut convolver = ConvolveReverb::new(buffer_size, sub_filter_size);
    convolver.prepare(h);

    // 执行卷积
    let mut output = Vec::new();
    for &sample in inputs.iter() {
        output.push(convolver.process_sample(sample));
    }

    // 打印结果
    println!("卷积结果: {:?}", output);
}