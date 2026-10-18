//! Ordered asynchronous streams and events.
//!
//! A [`Stream`] owns one worker thread. Work submitted to a stream runs in
//! submission order. An [`Event`] is recorded on a stream and completes when
//! every item submitted before it has finished; another stream can wait on it,
//! which expresses a cross-stream (and cross-device) dependency.

use anyhow::{anyhow, Result};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

type Work = Box<dyn FnOnce() -> Result<()> + Send + 'static>;

enum Command {
    Run(Work),
    Signal(Event),
    Wait(Event),
}

#[derive(Debug, Default)]
struct EventState {
    complete: bool,
    error: Option<String>,
}

/// One-shot completion flag shared between streams and the host.
#[derive(Clone, Debug, Default)]
pub struct Event {
    inner: Arc<(Mutex<EventState>, Condvar)>,
}

impl Event {
    pub fn new() -> Self {
        Self::default()
    }

    fn complete(&self, error: Option<String>) {
        let (lock, cvar) = &*self.inner;
        let mut state = lock.lock().expect("event lock poisoned");
        state.complete = true;
        state.error = error;
        cvar.notify_all();
    }

    /// True once the event has been reached by its stream.
    pub fn query(&self) -> bool {
        self.inner.0.lock().expect("event lock poisoned").complete
    }

    /// Block until complete. Returns the first error seen by the recording
    /// stream before this event, if any.
    pub fn synchronize(&self) -> Result<()> {
        let (lock, cvar) = &*self.inner;
        let mut state = lock.lock().expect("event lock poisoned");
        while !state.complete {
            state = cvar.wait(state).expect("event lock poisoned");
        }
        match &state.error {
            Some(message) => Err(anyhow!(message.clone())),
            None => Ok(()),
        }
    }

    /// Block for at most `timeout`; returns whether the event completed.
    pub fn wait_timeout(&self, timeout: Duration) -> bool {
        let (lock, cvar) = &*self.inner;
        let state = lock.lock().expect("event lock poisoned");
        let (state, _) = cvar
            .wait_timeout_while(state, timeout, |state| !state.complete)
            .expect("event lock poisoned");
        state.complete
    }
}

/// An ordered work queue bound to one device id.
pub struct Stream {
    device: String,
    sender: Option<Sender<Command>>,
    worker: Option<JoinHandle<()>>,
}

impl Stream {
    pub fn new(device: impl Into<String>) -> Self {
        let (sender, receiver) = channel::<Command>();
        let worker = std::thread::spawn(move || {
            // The first failure is sticky: later work is skipped and every later
            // event reports it, matching common accelerator stream semantics.
            let mut sticky: Option<String> = None;
            for command in receiver {
                match command {
                    Command::Run(work) => {
                        if sticky.is_none() {
                            if let Err(error) = work() {
                                sticky = Some(error.to_string());
                            }
                        }
                    }
                    Command::Signal(event) => event.complete(sticky.clone()),
                    Command::Wait(event) => {
                        if let Err(error) = event.synchronize() {
                            sticky.get_or_insert(error.to_string());
                        }
                    }
                }
            }
        });
        Self {
            device: device.into(),
            sender: Some(sender),
            worker: Some(worker),
        }
    }

    pub fn device(&self) -> &str {
        &self.device
    }

    fn send(&self, command: Command) -> Result<()> {
        self.sender
            .as_ref()
            .ok_or_else(|| anyhow!("stream is closed"))?
            .send(command)
            .map_err(|_| anyhow!("stream worker has exited"))
    }

    /// Enqueue work; returns immediately.
    pub fn submit<F>(&self, work: F) -> Result<()>
    where
        F: FnOnce() -> Result<()> + Send + 'static,
    {
        self.send(Command::Run(Box::new(work)))
    }

    /// Record an event that completes after all previously submitted work.
    pub fn record_event(&self) -> Result<Event> {
        let event = Event::new();
        self.send(Command::Signal(event.clone()))?;
        Ok(event)
    }

    /// Make all later work on this stream wait until `event` completes.
    pub fn wait_event(&self, event: &Event) -> Result<()> {
        self.send(Command::Wait(event.clone()))
    }

    /// Block until all submitted work has finished.
    pub fn synchronize(&self) -> Result<()> {
        self.record_event()?.synchronize()
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
