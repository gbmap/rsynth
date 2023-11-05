use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use crossterm::event::{read, Event, KeyCode, KeyEventKind, KeyEvent, poll};

pub fn thread_input(mut handlers: Vec<Arc<Mutex<dyn KeyboardHandler + Send>>>) -> Result<(), std::io::Error>  {
    // let mut stdout = std::io::stdout();
    loop {
        if poll(Duration::from_millis(25))? {
            match read()? {
                Event::Key(crossterm::event::KeyEvent {
                    code: KeyCode::Char('q'),
                    kind: KeyEventKind::Release,
                    ..
                }) => break,
                crossterm::event::Event::Key(event) => handlers.iter_mut().for_each(|handler| handler.lock().unwrap().handle_key_event(event)),
                _ => ()
            }
        } else {
            handlers.iter_mut().for_each(|h| h.lock().unwrap().cleanup_events());
        }
    }
    Ok(())
}

pub trait KeyboardHandler {
    fn handle_key_event(&mut self, event: crossterm::event::KeyEvent);
    fn cleanup_events(&mut self) {}
}

#[derive(Debug)]
pub struct KeyboardBufferEvent {
    pub key: KeyCode,
    pub time_press: std::time::SystemTime,
    pub time_release: Option<std::time::SystemTime>,
}

#[derive(Debug)]
pub struct KeyboardBuffer {
    pub event_buffer : std::collections::HashMap<KeyCode, KeyboardBufferEvent>
}

impl KeyboardBuffer {
    pub fn new() -> KeyboardBuffer { KeyboardBuffer { event_buffer: std::collections::HashMap::<KeyCode, KeyboardBufferEvent>::default() } }
    pub fn clean_stale_events(&mut self) {
        let now = std::time::SystemTime::now();
        self.event_buffer.retain(|_, v| {
            match v.time_release {
                Some(t) => now.duration_since(t).unwrap().as_secs_f32() < 2.0,
                None => true
            }
        });
    }

    pub fn event_buffer(&mut self) -> &mut HashMap<KeyCode, KeyboardBufferEvent> {
        &mut self.event_buffer
    }
}

impl KeyboardHandler for KeyboardBuffer {
    fn handle_key_event(&mut self, event: KeyEvent) {
        match event {
            KeyEvent {kind: KeyEventKind::Press, ..} => {
                self.event_buffer().entry(event.code).or_insert(KeyboardBufferEvent {
                    key: event.code,
                    time_press: std::time::SystemTime::now(),
                    time_release: None,
                });
            },
            KeyEvent {kind: KeyEventKind::Release, ..} => {
                if let Some(buffer_event) = self.event_buffer().get_mut(&event.code) {
                    buffer_event.time_release = Some(std::time::SystemTime::now());
                } 
            },
            _ => ()
        }
    }

    fn cleanup_events(&mut self) {
        self.clean_stale_events();
    }
}