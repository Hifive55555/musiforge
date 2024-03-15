use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use midir::MidiInput;

enum MidiMessage {
    NoteOn {
        channel: u8,
        note: u8,
        velocity: u8,
    },
    NoteOff {
        channel: u8,
        note: u8,
        velocity: u8,
    },
}


#[derive(Clone, Copy)]
struct Note {
    note_number: u8,
    velocity: u8,
    channel: u8,
    start_time: Instant,
    end_time: Option<Instant>,
    amp_envelope: AmpEnvelope,
}

struct MidiSynthesizer {
    notes: HashMap<u8, Note>,
    current_time: Instant,
}

impl MidiSynthesizer {
    fn new() -> Self {
        MidiSynthesizer {
            notes: HashMap::new(),
            current_time: Instant::now(),
        }
    }

    fn handle_message(&mut self, message: &MidiMessage) {
        let now = Instant::now();
        match message {
            MidiMessage::NoteOn { channel, note, velocity } => {
                let end_time = if *velocity > 0 {
                    Some(now + Duration::from_millis(500)) // 假设音符持续500ms
                } else {
                    None
                };
                let amp_envelope = AmpEnvelope::new(*velocity);
                self.notes.insert(
                    *note,
                    Note {
                        note_number: *note,
                        velocity: *velocity,
                        channel: *channel,
                        start_time: now,
                        end_time,
                        amp_envelope,
                    },
                );
            }
            MidiMessage::NoteOff { channel, note, .. } => {
                if let Some(note) = self.notes.get_mut(note) {
                    if note.channel == *channel {
                        note.end_time = Some(now);
                    }
                }
            }
            _ => (),
        }
    }

    fn update(&mut self) {
        self.current_time = Instant::now();
        self.notes.retain(|_, v| {
            if let Some(end_time) = v.end_time {
                return self.current_time < end_time;
            }
            true
        });
        // 更新音量衰减
        for note in self.notes.values_mut() {
            note.amp_envelope.update(self.current_time);
        }
    }
}

#[derive(Clone, Copy)]
struct AmpEnvelope {
    attack_time: Duration,
    decay_time: Duration,
    sustain_level: f32,
    current_level: f32,
    last_update_time: Instant,
}

impl AmpEnvelope {
    fn new(velocity: u8) -> Self {
        let attack_time = Duration::from_millis(50); // 假设攻击时间为50ms
        let decay_time = Duration::from_millis(250); // 假设衰减时间为250ms
        let sustain_level = velocity as f32 / 127.0;
        AmpEnvelope {
            attack_time,
            decay_time,
            sustain_level,
            current_level: 0.0,
            last_update_time: Instant::now(),
        }
    }

    fn update(&mut self, now: Instant) {
        let elapsed_time = now.duration_since(self.last_update_time);
        self.last_update_time = now;

        match elapsed_time {
            d if d < self.attack_time => {
                self.current_level = (d.as_secs_f32() / self.attack_time.as_secs_f32()).min(1.0);
            }
            d if d < self.attack_time + self.decay_time => {
                self.current_level = (1.0 - ((d - self.attack_time).as_secs_f32() / self.decay_time.as_secs_f32()))
                    * self.sustain_level;
            }
            _ => {
                self.current_level = self.sustain_level;
            }
        }
    }
}

fn main(){
    let (tx, rx): (SyncSender<bool>, Receiver<bool>) = sync_channel(100);
    // midi_input.open_virtual_port(&midi_port).unwrap();

    let synthesizer = Arc::new(Mutex::new(MidiSynthesizer::new()));
    let synthesizer_clone = Arc::clone(&synthesizer);

    let handle_thread = thread::spawn(move || {
        let midi_input = MidiInput::new("MidiSynthesizer").unwrap();
        let midi_port = &midi_input.ports()[0];
        let _ = midi_input.connect(
            midi_port,
            "test-port",
            move |stamp, message, _| {
                if let Ok(_) = rx.try_recv() {
                    // break; // 接收到关闭信号，退出循环
                }
                let mut synthesizer = synthesizer_clone.lock().unwrap();
                synthesizer.handle_message(&MidiMessage::NoteOn {channel: 1, note: 1, velocity: 127});
            },
            "data"
        );
    });

    handle_thread.join().unwrap();
}
