

pub trait WaveGenerator { fn gen(&mut self, t: f32, freq: f32) -> f32; }

pub struct SinWave;
impl WaveGenerator for SinWave { fn gen(&mut self, t: f32, freq: f32) -> f32 { (t*std::f32::consts::FRAC_PI_2*freq).sin() }}

pub struct SquareWave;
impl WaveGenerator for SquareWave { fn gen(&mut self, t: f32, freq: f32) -> f32 { 
    let step = (t * freq) as i32;
    if step % 2 == 0 {
        1.0
    } else {
        -1.0
    }
}}

pub struct TriWave;
impl WaveGenerator for TriWave { fn gen(&mut self, t: f32, freq: f32) -> f32 { ((t*freq) % 2.0)-1.0 }}

use rand::{thread_rng, Rng};
use rand::rngs::ThreadRng;

pub struct RandomWave { rng: ThreadRng  }
unsafe impl Send for RandomWave {}
impl RandomWave { pub fn new() -> RandomWave { RandomWave { rng: thread_rng() }} }
impl WaveGenerator for RandomWave { 
    fn gen(&mut self, t: f32, freq: f32) -> f32 { self.rng.gen() }
}


#[derive(PartialEq, Debug, Copy, Clone)]
pub struct EnvTimeAmp { time: f32, min: f32, max: f32 } 
impl EnvTimeAmp { pub fn new(time: f32, min: f32, max: f32) -> Self { Self { time, min, max } } }

#[derive(Debug)]
pub struct Envelope(pub f32, pub f32, pub f32, pub f32);

impl Envelope {
    pub fn new() -> Envelope {  return Envelope(1.0, 1.0, 0.2, 1.0); }

    pub fn sample(&self, t: f32, t0: f32, t1: Option<f32>) -> f32 {
        macro_rules! lerp { ($t:expr, $a:expr, $b:expr) => ($a*(1.0-$t) + $b*$t) }
        macro_rules! lt { ($a:expr, $b:expr) => ( ($a.clamp(0.0, $b)/$b) ) }
        match t1 {
            Some(t1_) => { lerp!(lt!(t-t1_, self.3), self.2, 0.0) },
            None => { 
                lerp!(lt!(t-t0-self.0, self.1), lerp!(lt!(t-t0, self.0), 0.0, 1.0), self.2) 
            }
        }
    }
}