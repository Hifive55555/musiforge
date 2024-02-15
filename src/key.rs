// 之后考虑加入“加法”

pub trait Key {
    fn to_freqkey(&self) -> FreqKey;

    fn get_type(&self) -> KeyType;
}

pub enum KeyType {
    Freq,
    Note12,
    // Tuned12,
}

pub struct FreqKey{
    pub f: f32,
    pub volume: u8,
    pub duration: f32,
}

impl Key for FreqKey {
    fn to_freqkey(&self) -> FreqKey {
        Self {
            f: self.f,
            volume: self.volume,
            duration: self.duration,
        }
    }

    fn get_type(&self) -> KeyType {
        KeyType::Freq
    }
}

pub struct Note12Key<'a> {
    pub key: (&'a str, u8),
    pub volume: u8,
    pub base: f32,
}

impl Key for Note12Key<'_> {
    fn to_freqkey(&self) -> FreqKey {
        const RANK: [&str; 12] = ["A", "A#", "B", "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#"];
        // const RATIO: f32 = 1.059463094359; // 2 ^ (1/12)
        let base_bias_freq = if self.key.0 == "A" {440.0} else 
        {self.base.powf(RANK.iter().position(|&r| r == self.key.0).unwrap() as f32)};  // 440.0 ^ (x/12)
        FreqKey {
            f: base_bias_freq * 2.0_f32.powf(((4 - self.key.1) as f32).abs()),  // * 2 ^ |4 - x|
            volume: self.volume,
            duration: 1.0,
        }
    }

    fn get_type(&self) -> KeyType {
        KeyType::Note12
    }
}

// impl Tuned12Key<'_> {
//     pub fn new<'a>(key: &'a str) -> Self {
//         Self {
//             key: (key, 4),
//             base: 440.0,
//         }
//     }
// }
