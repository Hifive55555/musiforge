use musiforge::config::stream_setup_for;
use musiforge::{musiblock, key::*};
use cpal::traits::StreamTrait;
// use std::f32::EPSILON;


fn main() -> anyhow::Result<()> {
    // let k_2 = Note12Key{key: ("A", 4), volumn: 255 ,base: 440.0};
    // p.send_key(k_2);
    let mut p = musiblock::Piano::new(1.0 / 3.0);
    // p.send_key(k_1);

    let stream = stream_setup_for(move |data: &mut [f32], num_channels, time_start| {
        
        if approx_eq(time_start, 1.0) {
            p.send_key(FreqKey{f: 440.0, volume: 1, duration: 0.2});
        } else if approx_eq(time_start, 2.0) {
            p.send_key(FreqKey{f: 550.0, volume: 1, duration: 0.3});
        } else if approx_eq(time_start, 4.0) {
            p.send_key(FreqKey{f: 330.0, volume: 1, duration: 0.4});
        } else if approx_eq(time_start, 4.5) {
            p.send_key(FreqKey{f: 371.0, volume: 1, duration: 0.4});
        }

        for frame in data.chunks_mut(num_channels) {
            let value = p.tick();
            for sample in frame.iter_mut() {
                *sample = value;
            }
        }
    })?;
    stream.play()?;
    std::thread::sleep(std::time::Duration::from_millis(8000));

    Ok(())
}

// 比较两个 f32 值
fn approx_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.004
}