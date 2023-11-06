use std::sync::{Arc, Mutex};
use crossterm::ExecutableCommand;
use crossterm::terminal::Clear;
use crossterm::cursor::MoveTo;
use audio::instrument::{Instrument, thread_audio};
use input::{KeyboardHandler, thread_input};


pub mod audio;
pub mod input;

// ====================
//      AUDIO

fn thread_debug(m: Arc<Mutex<Instrument>>) {
    let mut stdout = std::io::stdout();
    loop {
        {
            let i = &mut m.lock().unwrap();
            stdout.execute(Clear(crossterm::terminal::ClearType::All)).unwrap();
            stdout.execute(MoveTo(0, 0)).unwrap();
            println!("{:?}", i.keyboard_buffer().event_buffer());
            // println!("🔥");
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}

struct DebugKeyboardHandler;
impl KeyboardHandler for DebugKeyboardHandler {
    fn handle_key_event(&mut self, event: crossterm::event::KeyEvent, timestamp: f32) {
        match event.kind {
            crossterm::event::KeyEventKind::Press => { println!("press"); },
            crossterm::event::KeyEventKind::Release => { println!("release"); },
            _ => ()
        }
    }

}


fn main() {
    let instr = Instrument::new();
    let debug = DebugKeyboardHandler {};

    let mtx_instrmnt = Arc::new(Mutex::<Instrument>::new(instr));
    let mtx_debug = Arc::new(Mutex::<DebugKeyboardHandler>::new(debug));

    let mtx_inst_debug = mtx_instrmnt.clone();
    std::thread::spawn(|| thread_debug(mtx_inst_debug));

    let mtx_inst_audio= mtx_instrmnt.clone();
    std::thread::spawn(|| thread_audio(mtx_inst_audio));

    let mtx_inst_input = mtx_instrmnt.clone();
    let mtx_debug_input = mtx_debug.clone();
    let event_handlers: Vec<Arc<Mutex<dyn KeyboardHandler + Send>>> = vec![
        (mtx_inst_input as Arc<Mutex<dyn KeyboardHandler + Send>>).clone(),
        (mtx_debug_input as Arc<Mutex<dyn KeyboardHandler + Send>>).clone(),

    ];
    if let Err(e) = std::thread::spawn(|| thread_input(event_handlers)).join() {
        eprintln!("Failed to join thread: {:?}", e);
    }
}
