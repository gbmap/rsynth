use cpal::{self, traits::{HostTrait, DeviceTrait, StreamTrait}};
use std::sync::{Arc, Mutex};
use crossterm::event::{read, Event, KeyCode, KeyEventKind};



// ======================
//        INPUT

fn thread_input(mut handlers: Vec<Arc<Mutex<dyn KeyHandler + Send>>>) -> Result<(), std::io::Error>  {
    // let mut stdout = std::io::stdout();
    loop {
        match read()? {
            Event::Key(crossterm::event::KeyEvent {
                code: KeyCode::Char('q'),
                kind: KeyEventKind::Release,
                ..
            }) => break,
            crossterm::event::Event::Key(event) => handlers.iter_mut().for_each(|handler| handler.lock().unwrap().handle_key_event(event)),
            _ => ()
        }
    }

    Ok(())
}

pub trait KeyHandler {
    fn handle_key_event(&mut self, event: crossterm::event::KeyEvent);
}

// ====================
//      AUDIO


#[derive(PartialEq, Debug, Copy, Clone)]
pub struct EnvTimeAmp { time: f32, min: f32, max: f32 } 
impl EnvTimeAmp { fn new(time: f32, min: f32, max: f32) -> Self { Self { time, min, max } } }


pub struct Envelope {
    attack: EnvTimeAmp,
    sustain: EnvTimeAmp,
    decay: EnvTimeAmp,
    release: EnvTimeAmp, 
}

impl Envelope {
    pub fn sample(&self, t: f32) -> f32 {
        let (env, relative_time) = self.select_env_time_amp(t);
        return env.min + (env.max - env.min) * relative_time;
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

pub struct Oscillator {
    sr: cpal::SampleRate,
    freq: f32,
    cursor: u128
}

impl Oscillator {
    pub fn new() -> Oscillator { 
        Oscillator { 
            cursor: 0, 
            freq: 220., 
            sr: cpal::SampleRate(0),
        }
    }
    
    // the cursor is used to advance through buffer samples and prevent
    // the wave from repeating on each buffer request from the sound card.
    pub fn advance_cursor(&mut self, n: u128) { self.cursor = (self.cursor + n) % u128::MAX }
    pub fn cursor(&self) -> u128 { self.cursor }
    pub fn set_sample_rate(&mut self, sr: cpal::SampleRate) { self.sr = sr }
    pub fn set_frequency(&mut self, f: f32) { self.freq = f }
    pub fn sample_rate(&self) -> u128 { self.sr.0 as u128 }
    pub fn gen(&self, i: u128) -> f32 {  self.x(i).sin()*self.amplitude() }

    // get t for wave generation fn 
    fn amplitude(&self) -> f32 { 1.0 }
    fn x(&self, i: u128) -> f32 { return (((self.cursor+i) as f32)/(self.sample_rate() as f32))*std::f32::consts::FRAC_PI_2*self.freq; }
}

impl WaveGenerator for Oscillator {
    fn gen(&self, t: f32, freq: f32) -> f32 {
        return (t*std::f32::consts::FRAC_2_PI*freq).sin();
    }
}

unsafe impl Sync for Oscillator { }
impl KeyHandler for Oscillator {
    fn handle_key_event(&mut self, event: crossterm::event::KeyEvent) {
        let mut key_to_freq = std::collections::HashMap::<char, f32>::new();
        key_to_freq.insert('z', 130.81); // C
        key_to_freq.insert('x', 146.83);
        key_to_freq.insert('c', 164.81);
        key_to_freq.insert('v', 174.61);
        key_to_freq.insert('b', 196.00);
        key_to_freq.insert('n', 220.00);
        key_to_freq.insert('m', 246.94);
        

        match event {
           crossterm::event::KeyEvent { kind: KeyEventKind::Press, .. } => {
                println!("{:?}", event.code);
                if let KeyCode::Char(c) = event.code {
                    if let Some(&freq) = key_to_freq.get(&c) {
                        self.set_frequency(freq);
                    }
                }
            },
            _ => ()
        }
    }
}

fn thread_audio(mosc: Arc<Mutex<Oscillator>>) {
    let host: cpal::Host = cpal::default_host();
    let device = host.default_output_device().expect("No default output device found.");
    let cfg_output = device.supported_output_configs().expect("No supported output config.").next().expect("No supported output config.").with_max_sample_rate();
    let err_fn = |err| eprintln!("error occurred on output stream: {}", err);
    {
        let mut osc = mosc.lock().unwrap();
        osc.set_sample_rate(cfg_output.sample_rate());
    }

    // might need to generalize data type depending on platform.
    fn generate_audio(data: &mut [f32], _: &cpal::OutputCallbackInfo, mosc: Arc<Mutex<Oscillator>>) {
        let mut osc = mosc.lock().unwrap();
        for (i, sample) in data.iter_mut().enumerate() {
            *sample = osc.gen(i as u128);
        }
        osc.advance_cursor(data.len() as u128);
    }

    let stream = device.build_output_stream(
        &cfg_output.config(), 
        move |d, o| generate_audio(d, o, Arc::clone(&mosc)), 
        err_fn, None)
    .expect("error building output stream");

    stream.play().unwrap();
    loop { }
}


fn main() {
    let osc = Oscillator::new();
    let mosc = Arc::new(Mutex::<Oscillator>::new(osc));
    let event_handlers = vec![
        Arc::clone(&mosc) as Arc<Mutex<dyn KeyHandler + Send>>
    ];

    {
        let a1 = Arc::clone(&mosc);
        let a2 = Arc::clone(&mosc);
        std::thread::spawn(|| thread_audio(a1));
        let _ = std::thread::spawn(|| thread_input(event_handlers)).join().expect("Error on input thread.");
    }
}


mod tests {
    use crate::{EnvTimeAmp, Envelope};

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