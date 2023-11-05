use cpal::{self, traits::{HostTrait, DeviceTrait, StreamTrait}};
use std::{sync::{Arc, Mutex}, collections::HashMap};
// use std::collections::HashMap;
use crossterm::event::KeyCode;


use crate::input::{KeyboardBuffer, KeyboardHandler};

pub fn thread_audio(mtx_instrmnt: Arc<Mutex<Instrument>>) {
    let host: cpal::Host = cpal::default_host();
    let device = host.default_output_device().expect("No default output device found.");
    let cfg_output = device.supported_output_configs().expect("No supported output config.").next().expect("No supported output config.").with_max_sample_rate();
    let err_fn = |err| println!("error occurred on output stream: {}", err);
    println!("{:?}", cfg_output);
    println!("{:?}", device.name());

    {
        let mut instrument = mtx_instrmnt.lock().unwrap();
        instrument.set_sample_rate(cfg_output.sample_rate());
    }

    // might need to generalize data type depending on platform.
    fn generate_audio(data: &mut [f32], _: &cpal::OutputCallbackInfo, mti: Arc<Mutex<Instrument>>) {
        let mut instrmnt = mti.lock().unwrap();
        for (i, sample) in data.iter_mut().enumerate() {
            *sample = instrmnt.gen(i as u128);
        }
        instrmnt.advance_cursor(data.len() as u128);
    }

    let mtx_build_data = Arc::clone(&mtx_instrmnt);
    let stream = device.build_output_stream(
        &cfg_output.config(), 
        move |d, o| generate_audio(d, o, mtx_build_data.clone()), 
        err_fn, None)
    .expect("error building output stream");
    stream.play().unwrap();     
    loop {}
}


#[derive(PartialEq, Debug, Copy, Clone)]
pub struct EnvTimeAmp { time: f32, min: f32, max: f32 } 
impl EnvTimeAmp { pub fn new(time: f32, min: f32, max: f32) -> Self { Self { time, min, max } } }

#[derive(Debug)]
pub struct Envelope {
    pub attack: EnvTimeAmp,
    pub sustain: EnvTimeAmp,
    pub decay: EnvTimeAmp,
    pub release: EnvTimeAmp, 
}

impl Envelope {
    fn new() -> Envelope { 
        return Envelope {
            attack: EnvTimeAmp::new(1.0, 0.0, 1.0), 
            sustain: EnvTimeAmp::new(1.0, 1.0, 1.0), 
            decay: EnvTimeAmp::new(1.0, 1.0, 1.0), 
            release: EnvTimeAmp::new(2.0, 1.0, 0.0) 
        };
    }

    pub fn sample(&self, t: f32) -> f32 {
        let (env, relative_time) = self.select_env_time_amp(t);
        return env.min*(1.0-relative_time) + env.max*relative_time;
    }

    fn select_env_time_amp(&self, t: f32) -> (EnvTimeAmp, f32) {
        let envs: Vec<EnvTimeAmp> = vec![self.attack, self.sustain, self.decay, self.release];
        let mut time_until_last_envelope = 0.0;
        for env in envs[..envs.len()-1].iter() {
            let relative_time: f32 = t - time_until_last_envelope;
            time_until_last_envelope += env.time;
            if t <= time_until_last_envelope {
                return (*env, relative_time / env.time);
            }
        }
        (self.release, (t-time_until_last_envelope) / self.release.time)
    }
}

pub trait WaveGenerator { fn gen(&self, t: f32, freq: f32) -> f32; }

pub struct SinWave;
impl WaveGenerator for SinWave { fn gen(&self, t: f32, freq: f32) -> f32 { (t*std::f32::consts::FRAC_PI_2*freq).sin() }}

pub struct SquareWave;
impl WaveGenerator for SquareWave { fn gen(&self, t: f32, freq: f32) -> f32 { (t*freq).sin() }}

pub struct Instrument {
    sr: cpal::SampleRate,
    freq: f32,
    cursor: u128,
    wave_generator: Box<dyn WaveGenerator + Send>,
    keyboard_buffer: KeyboardBuffer,
    envelope: Envelope,
    key_to_freq: HashMap<KeyCode, f32>
}

impl std::fmt::Debug for Instrument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // self.sr.fmt(f);
        // self.freq.fmt(f);
        let _=self.cursor.fmt(f);
        // self.keyboard_buffer.fmt(f);
        // self.envelope.fmt(f);
        return std::fmt::Result::Ok(());
    }
}

impl Instrument {
    pub fn new() -> Instrument { 
        let mut k2f = HashMap::<KeyCode, f32>::new();
        k2f.insert(KeyCode::Char('z'), 130.81); // C
        k2f.insert(KeyCode::Char('x'), 146.83);
        k2f.insert(KeyCode::Char('c'), 164.81);
        k2f.insert(KeyCode::Char('v'), 174.61);
        k2f.insert(KeyCode::Char('b'), 196.00);
        k2f.insert(KeyCode::Char('n'), 220.00);
        k2f.insert(KeyCode::Char('m'), 246.94);
        let i = Instrument { 
            cursor: 0, 
            freq: 220., 
            sr: cpal::SampleRate(0),
            wave_generator: Box::new(SinWave),
            keyboard_buffer: KeyboardBuffer::new(),
            envelope: Envelope::new(),
            key_to_freq: k2f
        };
        return i;
    }

    pub fn keyboard_buffer(&mut self) -> &mut KeyboardBuffer { &mut self.keyboard_buffer }
    
    // the cursor is used to advance through buffer samples and prevent
    // the wave from repeating on each buffer request from the sound card.
    pub fn advance_cursor(&mut self, n: u128) { self.cursor = (self.cursor + n) % u128::MAX }
    pub fn cursor(&self) -> u128 { self.cursor }
    pub fn set_sample_rate(&mut self, sr: cpal::SampleRate) { self.sr = sr }
    pub fn set_frequency(&mut self, f: f32) { self.freq = f }
    pub fn sample_rate(&self) -> u128 { self.sr.0 as u128 }

    fn t(&self, i: u128) -> f32 { return ((self.cursor+i) as f32)/(self.sample_rate() as f32); }

    pub fn gen(&self, i: u128) -> f32 {  
        let t = self.t(i);
        let now = std::time::SystemTime::now();

        self.keyboard_buffer.event_buffer.iter()
            .map(|event| {
                let env_t = now.duration_since(event.1.time_press).unwrap().as_secs_f32();
                let freq = self.key_to_freq.get(event.0).unwrap_or(&0.0);
                self.wave_generator.gen(t, *freq) * self.envelope.sample(env_t)
            })
            .sum()
    }

    // get t for wave generation fn 
}


unsafe impl Sync for Instrument { }
impl KeyboardHandler for Instrument {
    fn handle_key_event(&mut self, event: crossterm::event::KeyEvent) {
        self.keyboard_buffer.handle_key_event(event);
    }

    fn cleanup_events(&mut self) {
        self.keyboard_buffer.cleanup_events();
    }
}


mod audio_tests {
    use crate::audio::{EnvTimeAmp, Envelope};

    fn default_envelope() -> Envelope {
        return Envelope { 
            attack: EnvTimeAmp::new(1.0, 0.0, 1.0), 
            sustain: EnvTimeAmp::new(1.0, 2.0, 3.0), 
            decay: EnvTimeAmp::new(1.0, 4.0, 5.0), 
            release: EnvTimeAmp::new(1.0, 6.0, 7.0) 
        };
    }

    macro_rules! assert_approx_eq {
        ($a:expr, $b:expr) => {{
            assert!(($a - $b).abs() < 1e-6, "Expected {} to be approximately equal to {}", $a, $b);
        }};
    }

    #[test]
    fn test_envelope() {
        let e = default_envelope();

        assert_approx_eq!(e.sample(0.0), 0.0);
        assert_approx_eq!(e.sample(1.0-f32::EPSILON), 1.0);
        assert_approx_eq!(e.sample(1.0+f32::EPSILON), 2.0);
        // assert_eq!(e.sample(0.45), 4.0);
        // assert_eq!(e.sample(0.0), 0.0);
    }

    #[test]
    fn test_select_envelope() {
        let e = default_envelope(); 
        assert_eq!(e.select_env_time_amp(0.0).0, e.attack);
        assert_eq!(e.select_env_time_amp(1.0-f32::EPSILON).0, e.attack);
        assert_eq!(e.select_env_time_amp(1.0+f32::EPSILON).0, e.sustain);
        assert_eq!(e.select_env_time_amp(2.0-f32::EPSILON*1.0001).0, e.sustain); // why
        assert_eq!(e.select_env_time_amp(2.0+f32::EPSILON*1.0001).0, e.decay); // why
        assert_eq!(e.select_env_time_amp(3.0-f32::EPSILON*1.0001).0, e.decay); // whywhy
        assert_eq!(e.select_env_time_amp(3.0+f32::EPSILON*1.0001).0, e.release); // whywhy
    }

}