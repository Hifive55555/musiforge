//! 效果器的初步测试实现

use core::num;

use rustfft::num_complex::Complex;
use rustfft::num_traits::Zero;
use rustfft::FftPlanner;

pub struct ConvolveReverb {
    ir_data: Vec<f32>,
    input_domain: Vec<f32>,
    sub_filters_fft: Vec<Vec<Complex<f32>>>,
    freq_delay_line: Vec<Vec<Complex<f32>>>,
    buffer_size: usize,
    sub_filter_size: usize,
    planner: FftPlanner<f32>,
}

impl ConvolveReverb {
    pub fn new(ir_data: Vec<f32>, buffer_size: usize, sub_filter_size: usize) -> Self {
        let num_sub_filters = (ir_data.len() + sub_filter_size - 1) / sub_filter_size;
        let fft_size = buffer_size + sub_filter_size;

        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(fft_size);

        let mut sub_filters_fft = Vec::with_capacity(num_sub_filters);
        let mut freq_delay_line = Vec::with_capacity(num_sub_filters);

        for i in 0..num_sub_filters {
            let start = i * sub_filter_size;
            let end = (i + 1) * sub_filter_size;
            let mut sub_filter = vec![Complex { re: 0.0, im: 0.0 }; fft_size];
            for j in 0..sub_filter_size {
                sub_filter[j] = if start + j < ir_data.len() {
                    Complex { re: ir_data[start + j], im: 0.0 }
                } else {
                    Complex { re: 0.0, im: 0.0 }
                };
            }
            fft.process(&mut sub_filter);
            sub_filters_fft.push(sub_filter);
            freq_delay_line.push(vec![Complex { re: 0.0, im: 0.0 }; fft_size]);
        }

        ConvolveReverb {
            ir_data,
            input_domain: vec![0.0; 2 * buffer_size], // 2 * B 的大小
            sub_filters_fft,
            freq_delay_line,
            buffer_size,
            sub_filter_size,
            planner,
        }
    }

    pub fn process(&mut self, input: &[f32]) -> Vec<f32> {
        let fft_size = self.buffer_size + self.sub_filter_size;
        let num_sub_filters = self.sub_filters_fft.len();
        let mut input_buffer = vec![Complex { re: 0.0, im: 0.0 }; fft_size];
        let mut output_buffer = vec![Complex { re: 0.0, im: 0.0 }; fft_size];
        let mut output = vec![0.0; self.buffer_size];

        // 将当前输入块和上一次缓冲区的数据合并到 input_domain
        for i in 0..self.buffer_size {
            self.input_domain[i] = self.input_domain[i + self.buffer_size];
            self.input_domain[i + self.buffer_size] = input[i];
        }

        // 计算输入缓冲区的 FFT
        for i in 0..fft_size {
            input_buffer[i] = Complex { re: self.input_domain[i], im: 0.0 };
        }
        let fft = self.planner.plan_fft_forward(fft_size);
        fft.process(&mut input_buffer);

        // 频域相乘
        for i in 0..num_sub_filters {
            for j in 0..fft_size {
                self.freq_delay_line[i][j] = input_buffer[j] * self.sub_filters_fft[i][j];
            }
        }

        // 计算逆 FFT
        let ifft = self.planner.plan_fft_inverse(fft_size);
        for i in 0..num_sub_filters {
            ifft.process(&mut self.freq_delay_line[i]);// 归一化逆 FFT 结果
            for j in 0..fft_size {
                self.freq_delay_line[i][j] /= fft_size as f32;
            }
        }

        // 拼接结果
        for i in 0..num_sub_filters {
            for j in 0..self.buffer_size {
                output[j] += self.freq_delay_line[i][j + self.sub_filter_size].re;
            }
        }

        output
    }
}

fn convolve(input_signal: &[f32], ir: &[f32]) -> Vec<f32> {
    let input_len = input_signal.len() / 2;
    let ir_len = ir.len();
    let output_len = input_len + ir_len - 1;

    // 扩展输入信号和 IR 到相同的长度，通常是两者长度之和减 1
    let mut input_padded = vec![Complex::zero(); output_len];
    let mut ir_padded = vec![Complex::zero(); output_len];

    for (i, &sample) in input_signal.iter().enumerate() {
        input_padded[i] = Complex::new(sample, 0.0);
    }
    for (i, &sample) in ir.iter().enumerate() {
        ir_padded[i] = Complex::new(sample, 0.0);
    }

    // 创建 FFT 规划器
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(output_len);

    // 对输入信号和 IR 进行 FFT
    let mut input_fft = input_padded.clone();
    let mut ir_fft = ir_padded.clone();
    fft.process(&mut input_fft);
    fft.process(&mut ir_fft);

    // 点乘 FFT 结果
    for (input, ir) in input_fft.iter_mut().zip(ir_fft.iter()) {
        *input *= ir;
    }

    // 创建 IFFT 规划器
    let ifft = planner.plan_fft_inverse(output_len);

    // 进行 IFFT
    let mut output_fft = input_fft;
    ifft.process(&mut output_fft);

    // 获取实部并转换为 f32，归一化输出信号
    let mut output_signal = vec![0.0f32; output_len];
    for (i, sample) in output_fft.iter().enumerate() {
        output_signal[i] = sample.re / output_len as f32;
    }

    output_signal
}

fn save_output_file(
    output_signal: &[f32],
    output_file_path: &str,
    sample_rate: u32,
    num_channels: usize,
) -> Result<(), hound::Error> {
    let spec = hound::WavSpec {
        channels: num_channels as u16,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(output_file_path, spec)?;

    for chunk in output_signal.chunks(num_channels) {
        for &sample in chunk {
            writer.write_sample(sample)?;
        }
    }

    writer.finalize()?;
    Ok(())
}

fn convert_to_mono(input_signal: &[f32], num_channels: usize) -> Vec<f32> {
    input_signal
        .chunks(num_channels)
        .map(|chunk| chunk.iter().sum::<f32>() / num_channels as f32)
        .collect()
}

struct ConvolveChannels {
    num_channels: usize,
    convolvers: Vec<ConvolveReverb>,
}

impl ConvolveChannels {
    fn new(num_channels: usize, ir_data: Vec<f32>, buffer_size: usize) -> Self {
        let buffer_size = buffer_size / 2;
        ConvolveChannels {
            num_channels,
            convolvers: (0..num_channels).map(|_| ConvolveReverb::new(ir_data.clone(), buffer_size, buffer_size)).collect()
        }
    }

    fn process(&mut self, input_signal: &[f32]) -> Vec<f32> {
        let input_len = input_signal.len() / self.num_channels;
    
        let mut output_signals = vec![vec![0.0f32; input_len]; self.num_channels];
    
        for channel in 0..self.num_channels {
            let channel_input = input_signal
                .iter()
                .skip(channel)
                .step_by(self.num_channels)
                .cloned()
                .collect::<Vec<f32>>();
    
            let channel_output = self.convolvers[channel].process(&channel_input);
            output_signals[channel] = channel_output;
        }
    
        // 交错排列每个通道的样本
        let mut interleaved_output = vec![0.0f32; input_len * self.num_channels];
        for (channel, channel_output) in output_signals.iter().enumerate() {
            for (i, &sample) in channel_output.iter().enumerate() {
                interleaved_output[i * self.num_channels + channel] = sample;
            }
        }
    
        interleaved_output
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use hound;

    #[test]
    fn test_convolve() -> Result<(), Box<dyn std::error::Error>> {
        // 加载 IR 文件
        let reader = hound::WavReader::open("test_data/ir_f.wav")?;
        let ir = reader
            .into_samples::<f32>()
            .collect::<hound::Result<Vec<_>>>()?;

        // 获取输入信号流
        let reader = hound::WavReader::open("test_data/test.wav")?;
        let num_channels = reader.spec().channels as usize;

        let ir = convert_to_mono(&ir, num_channels);
        let input_signal = reader
            .into_samples::<f32>()
            .collect::<hound::Result<Vec<_>>>()?;

        // 执行卷积运算
        // let output_signal = convolve_channels(&input_signal, num_channels, &ir);

        // // 保存输出文件
        // save_output_file(
        //     &output_signal,
        //     "test_data/output_file.wav",
        //     48000,
        //     num_channels,
        // )?;

        Ok(())
    }

    #[test]
    fn test_convolve_reverb() -> Result<(), Box<dyn std::error::Error>> {
        // 加载 IR 文件
        let reader = hound::WavReader::open("test_data/ir_f.wav")?;
        let ir_data = reader.into_samples::<f32>().collect::<hound::Result<Vec<_>>>()?;
        let buffer_size = 4096; // 选择合适的缓冲区大小

        // 创建 ConvolveReverb 实例
        // let mut reverb = ConvolveReverb::new(ir_data, buffer_size, buffer_size);
        let mut reverb = ConvolveChannels::new(2, ir_data, buffer_size);

        // 加载输入信号
        let reader = hound::WavReader::open("test_data/test.wav")?;
        let input_signal = reader.into_samples::<f32>().collect::<hound::Result<Vec<_>>>()?;

        // 执行卷积
        let mut output_signal = Vec::new();
        for i in 0..input_signal.len() / buffer_size {
            let slice = (i * buffer_size)..(i * buffer_size + buffer_size);
            output_signal.extend(reverb.process(&input_signal[slice]));
        }

        // 保存输出文件
        save_output_file(&output_signal, "test_data/output_file.wav", 48000, 2)?;

        Ok(())
    }
}
