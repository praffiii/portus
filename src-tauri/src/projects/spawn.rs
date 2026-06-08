use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, Child, CommandBuilder, PtySize};
use serde::Serialize;
use specta::Type;
use tauri::ipc::Channel;

use super::env::load_project_env;
use crate::logs::ansi::{LogBatch, LogBatcher, LogLine};

pub const RING_CAPACITY: usize = 200;

#[derive(Debug)]
pub struct RingBuffer {
    capacity: usize,
    lines: VecDeque<String>,
}

impl RingBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            lines: VecDeque::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, line: String) {
        if self.lines.len() == self.capacity {
            self.lines.pop_front();
        }
        self.lines.push_back(line);
    }

    pub fn lines(&self) -> Vec<String> {
        self.lines.iter().cloned().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn last_non_empty(&self) -> Option<String> {
        self.lines
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
            .cloned()
    }
}

pub struct SpawnedProcess {
    pid: u32,
    pgid: u32,
    child: Box<dyn Child + Send + Sync>,
    output: Arc<Mutex<RingBuffer>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    stream: Arc<Mutex<LogStream>>,
    last_output_at: Arc<Mutex<Instant>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessExitStatus {
    code: Option<i32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum InputStatus {
    Sent,
    Ignored,
}

impl ProcessExitStatus {
    pub fn code(&self) -> Option<i32> {
        self.code
    }
}

/// Spawn `command` through a login shell in `cwd`, inside a PTY-backed process
/// group. The PTY merges stdout and stderr, and its bytes feed the capped
/// collapsed-glance ring buffer.
pub fn spawn_task(shell: &str, command: &str, cwd: &Path) -> io::Result<SpawnedProcess> {
    let output = Arc::new(Mutex::new(RingBuffer::new(RING_CAPACITY)));
    let stream = Arc::new(Mutex::new(LogStream::new()));
    let last_output_at = Arc::new(Mutex::new(Instant::now()));
    let env_overlay = load_project_env(cwd);

    if let Some(notice) = env_overlay.notice {
        output
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(notice);
    }

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(to_io_error)?;

    let mut cmd = CommandBuilder::new(shell);
    cmd.arg("-l");
    cmd.arg("-c");
    cmd.arg(command);
    cmd.cwd(cwd);
    for (key, value) in env_overlay.vars {
        cmd.env(key, value);
    }

    let child = pair.slave.spawn_command(cmd).map_err(to_io_error)?;
    let pid = child
        .process_id()
        .ok_or_else(|| io::Error::other("PTY child did not report a pid"))?;
    let reader = pair.master.try_clone_reader().map_err(to_io_error)?;
    let writer = pair.master.take_writer().map_err(to_io_error)?;

    spawn_reader(
        reader,
        output.clone(),
        stream.clone(),
        last_output_at.clone(),
    );

    Ok(SpawnedProcess {
        pid,
        pgid: pid,
        child,
        output,
        writer: Arc::new(Mutex::new(writer)),
        stream,
        last_output_at,
    })
}

fn to_io_error(error: impl std::fmt::Display) -> io::Error {
    io::Error::other(error.to_string())
}

fn spawn_reader<R: Read + Send + 'static>(
    mut reader: R,
    buffer: Arc<Mutex<RingBuffer>>,
    stream: Arc<Mutex<LogStream>>,
    last_output_at: Arc<Mutex<Instant>>,
) {
    thread::spawn(move || {
        let mut pending = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            match reader.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    *last_output_at.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
                    pending.extend_from_slice(&chunk[..n]);
                    drain_lines(&mut pending, &buffer);
                    stream
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push_bytes(&chunk[..n]);
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
        if !pending.is_empty() {
            push_output_line(&buffer, bytes_to_line(&pending));
        }
    });
}

fn drain_lines(pending: &mut Vec<u8>, buffer: &Arc<Mutex<RingBuffer>>) {
    while let Some(pos) = pending.iter().position(|b| *b == b'\n' || *b == b'\r') {
        let raw: Vec<u8> = pending.drain(..=pos).collect();
        push_output_line(buffer, bytes_to_line(&raw));
    }
}

fn bytes_to_line(raw: &[u8]) -> String {
    String::from_utf8_lossy(raw)
        .trim_end_matches(['\r', '\n'])
        .to_string()
}

fn push_output_line(buffer: &Arc<Mutex<RingBuffer>>, line: String) {
    buffer.lock().unwrap_or_else(|e| e.into_inner()).push(line);
}

impl SpawnedProcess {
    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn pgid(&self) -> u32 {
        self.pgid
    }

    /// Non-blocking exit check. `Some(status)` once the shell child has exited.
    pub fn try_status(&mut self) -> io::Result<Option<ProcessExitStatus>> {
        self.child.try_wait().map(|status| {
            status.map(|status| ProcessExitStatus {
                code: status
                    .signal()
                    .is_none()
                    .then_some(status.exit_code() as i32),
            })
        })
    }

    pub fn write_input(&self, data: &[u8]) -> io::Result<()> {
        self.writer
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .write_all(data)
    }

    pub fn send_input_if_running(&mut self, data: &[u8]) -> io::Result<InputStatus> {
        if self.try_status()?.is_some() {
            return Ok(InputStatus::Ignored);
        }
        self.write_input(data)?;
        Ok(InputStatus::Sent)
    }

    pub fn recent_output(&self) -> Vec<String> {
        self.output
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .lines()
    }

    pub fn subscribe_logs(&self, channel: Channel<LogBatch>) {
        let replay = self.recent_output();
        self.stream
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .subscribe(channel, replay);
    }

    pub fn unsubscribe_logs(&self) {
        self.stream
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .unsubscribe();
    }

    pub fn has_output(&self) -> bool {
        !self
            .output
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_empty()
    }

    pub fn output_idle(&self) -> Duration {
        self.last_output_at
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .elapsed()
    }

    pub fn last_non_empty_output(&self) -> Option<String> {
        self.output
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .last_non_empty()
    }
}

struct LogStream {
    batcher: LogBatcher,
    channel: Option<Channel<LogBatch>>,
}

impl LogStream {
    fn new() -> Self {
        Self {
            batcher: LogBatcher::new(RING_CAPACITY, 128 * 1024),
            channel: None,
        }
    }

    fn subscribe(&mut self, channel: Channel<LogBatch>, replay: Vec<String>) {
        let replay = LogBatch {
            lines: replay
                .into_iter()
                .map(|line| LogLine {
                    html: crate::logs::ansi::sanitize_chunk(line.as_bytes()),
                })
                .collect(),
        };
        let _ = channel.send(replay);
        self.channel = Some(channel);
    }

    fn unsubscribe(&mut self) {
        self.channel = None;
    }

    fn push_bytes(&mut self, bytes: &[u8]) {
        let batch = self.batcher.push_bytes(bytes);
        if batch.lines.is_empty() {
            return;
        }
        if self
            .channel
            .as_ref()
            .is_some_and(|channel| channel.send(batch).is_err())
        {
            self.channel = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_buffer_keeps_only_the_last_n_lines() {
        let mut buffer = RingBuffer::new(2);
        buffer.push("one".to_string());
        buffer.push("two".to_string());
        buffer.push("three".to_string());

        assert_eq!(buffer.lines(), vec!["two".to_string(), "three".to_string()]);
    }

    #[test]
    fn ring_buffer_reports_emptiness() {
        let mut buffer = RingBuffer::new(4);
        assert!(buffer.is_empty());
        buffer.push("hi".to_string());
        assert!(!buffer.is_empty());
    }
}
