

pub trait WaveGenerator { fn gen(&mut self, t: f32) -> f32; }
pub trait Randomize { fn randomize(&mut self); }

unsafe impl Send for Oscillator {}
unsafe impl Send for LinearTransform {}
unsafe impl Send for ConstantWave {}
unsafe impl Send for NullWave {}
unsafe impl Send for RandomWave {}

pub struct NullWave;
impl WaveGenerator for NullWave { fn gen(&mut self, _: f32) -> f32 { 0.0 }}

pub struct IdentityWave;
impl WaveGenerator for IdentityWave { fn gen(&mut self, t: f32) -> f32 { 1.0 }}

pub struct ConstantWave;
impl WaveGenerator for ConstantWave { fn gen(&mut self, t: f32) -> f32 { t }}

pub struct SinWave;
impl WaveGenerator for SinWave { fn gen(&mut self, t: f32) -> f32 { (t*std::f32::consts::FRAC_PI_2).sin() }}

pub struct SquareWave;
impl WaveGenerator for SquareWave { fn gen(&mut self, t: f32) -> f32 {  if (t as i32) % 2 == 0 { 1.0 } else { -1.0 } }}

pub struct TriWave;
impl WaveGenerator for TriWave { fn gen(&mut self, t: f32) -> f32 { (t % 2.0)-1.0 }}

fn random_wave_generator() -> Box<dyn WaveGenerator> {
    let mut rng = rand::thread_rng();
    let index = rng.gen_range(1..7);

    if index == 0 {
        let mut lt = LinearTransform::default();
        lt.randomize();
        return Box::new(lt);
    }
    if index == 1 {
        Box::new(IdentityWave)
    } else if index == 2 {
        Box::new(ConstantWave)
    } else if index == 3 {
        Box::new(SinWave)
    } else if index == 4 {
        Box::new(SquareWave)
    } else if index == 5 {
        Box::new(TriWave)
    } else {
        Box::new(RandomWave::new())
    }
}

pub struct RandomWave { rng: ThreadRng  }
impl RandomWave { pub fn new() -> RandomWave { RandomWave { rng: thread_rng() }} }
impl WaveGenerator for RandomWave {  fn gen(&mut self, _: f32) -> f32 { self.rng.gen() } }

pub struct LinearTransform { pub alpha: Box<dyn WaveGenerator>, pub beta: Box<dyn WaveGenerator> }
impl LinearTransform {  fn default() -> LinearTransform { LinearTransform { alpha: Box::new(IdentityWave), beta: Box::new(NullWave) } } }
impl WaveGenerator for LinearTransform { fn gen(&mut self, t: f32) -> f32 { self.alpha.gen(t)*t + self.beta.gen(t) } }

impl Randomize for LinearTransform {
    fn randomize(&mut self) {
        self.alpha = random_wave_generator();
        self.beta = random_wave_generator();
    }
}

pub trait Voicing { fn gen(&self, osc: &mut Vec<&mut Oscillator>, t: f32, freq: f32) -> f32; }

pub struct MeanVoicing;
impl Voicing for MeanVoicing { fn gen(&self, oscs: &mut Vec<&mut Oscillator>, t: f32, freq: f32) -> f32 { 
    oscs.iter_mut().map(|o| o.gen(t, freq)).sum::<f32>() / (oscs.len() as f32)
}}

pub struct RepeatedVoicing(pub f32, pub f32, pub u16);
impl Voicing for RepeatedVoicing { 
     fn gen(&self, oscs: &mut Vec<&mut Oscillator>, t: f32, freq: f32) -> f32 { 
        let mut total = 0.0;
        for osc in oscs {
            total += osc.gen(t, freq);
            for i in 1..self.2 {
                let i_f = i as f32;
                total += osc.gen(t-(self.0*i_f), freq*(self.1*i_f));
                total += osc.gen(t+(self.0*i_f), freq*(1.0/(self.1*i_f)));
            }
        }
        total / (self.2 as f32) / 2.0
    }
}

unsafe impl Send for MeanVoicing {}
unsafe impl Send for RepeatedVoicing {}


pub struct Test {
    pub osc: Oscillator,
    pub voicing: Box<dyn Voicing>
}

impl Test {
    fn gen(&mut self, t: f32, freq: f32) -> f32 { 
        self.voicing.gen(&mut vec![&mut self.osc], t, freq)
    }
}

unsafe impl Send for Test {}

// wave generation on steroids
pub struct Oscillator {
    pub ttf : LinearTransform,
    pub wtf : LinearTransform,
    pub otf : Box<dyn WaveGenerator>,
}

impl Oscillator { 
    pub fn gen(&mut self, t: f32, freq: f32) -> f32 {  
        self.otf.gen(self.ttf.gen(t)*self.wtf.gen(freq)) 
    } 
}

impl Randomize for Oscillator {
    fn randomize(&mut self) {
        self.ttf.randomize();
        self.wtf.randomize();
        self.otf = random_wave_generator();
    }
}

use rand::{thread_rng, Rng};
use rand::rngs::ThreadRng;


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

impl Randomize for Envelope {
    fn randomize(&mut self) {
        let mut rng = thread_rng();
        self.0 = rng.gen();
        self.1 = rng.gen();
        self.2 = rng.gen();
        self.3 = rng.gen();
    }
}


mod wave_tests {
    use rand::Rng;

    use crate::audio::waves::{EnvTimeAmp, Envelope, Oscillator, LinearTransform, ConstantWave, NullWave, SinWave, WaveGenerator};

    use super::IdentityWave;

    macro_rules! assert_approx_eq {
        ($a:expr, $b:expr) => {{
            assert!(($a - $b).abs() < 1e-6, "Expected {} to be approximately equal to {}", $a, $b);
        }};
    }

    #[test]
    fn test_envelope() {
        let e = Envelope(1.0, 1.0, 0.5, 1.0);

        assert_approx_eq!(e.sample(-100.0, 0.0, None), 0.0);
        assert_approx_eq!(e.sample(-0.1, 0.0, None), 0.0);
        assert_approx_eq!(e.sample(0.0, 0.0, None), 0.0);
        assert_approx_eq!(e.sample(0.1, 0.0, None), 0.1);
        assert_approx_eq!(e.sample(1.0, 0.0, None), 1.0);
        assert_approx_eq!(e.sample(2.0, 0.0, None), 0.5);
        assert_approx_eq!(e.sample(100.0, 0.0, None), 0.5);
        assert_approx_eq!(e.sample(3.0, 0.0, Some(2.0)), 0.0);

    }

    #[test]
    fn test_constant_wave() {
        let mut g = ConstantWave;
        let mut rng = rand::thread_rng();
        for _ in 0..100 {
            let y = rng.gen();
            assert_approx_eq!(g.gen(y), y);
        }
    }

    #[test]
    fn test_constant_linear_transform() {
        let mut lt = LinearTransform { alpha: Box::new(IdentityWave), beta: Box::new(NullWave) };
        let mut rng = rand::thread_rng();
        for _ in 0..100 {
            let y = rng.gen();
            assert_approx_eq!(lt.gen(y), y);
        }
    }

    #[test]
    fn test_simple_sin_wave() {
        let mut test_generator = Oscillator { 
            ttf: LinearTransform { alpha: Box::new(IdentityWave), beta: Box::new(NullWave) },
            wtf: LinearTransform { alpha: Box::new(IdentityWave), beta: Box::new(NullWave) },
            otf: Box::new(SinWave)
        };

        let mut control_generator = SinWave;

        for i in 0..100 {
            let f = (i as f32) / 10.0;
            assert_approx_eq!(test_generator.gen(f, 1.0), control_generator.gen(f));
        }
    }

}