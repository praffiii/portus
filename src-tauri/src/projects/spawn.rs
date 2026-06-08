use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

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
}

pub struct SpawnedProcess {
    pid: u32,
    pgid: u32,
    child: Child,
    output: Arc<Mutex<RingBuffer>>,
}

/// Spawn `command` through a login shell in `cwd`, in its own process group,
/// capturing stdout+stderr into a capped ring buffer. No PTY (Layer 3).
pub fn spawn_task(shell: &str, command: &str, cwd: &Path) -> std::io::Result<SpawnedProcess> {
    let mut cmd = Command::new(shell);
    cmd.arg("-l")
        .arg("-c")
        .arg(command)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    let mut child = cmd.spawn()?;
    let pid = child.id();
    let output = Arc::new(Mutex::new(RingBuffer::new(RING_CAPACITY)));

    if let Some(stdout) = child.stdout.take() {
        spawn_reader(stdout, output.clone());
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_reader(stderr, output.clone());
    }

    Ok(SpawnedProcess {
        pid,
        pgid: pid,
        child,
        output,
    })
}

fn spawn_reader<R: std::io::Read + Send + 'static>(reader: R, buffer: Arc<Mutex<RingBuffer>>) {
    thread::spawn(move || {
        let reader = BufReader::new(reader);
        for line in reader.lines().map_while(Result::ok) {
            buffer.lock().unwrap_or_else(|e| e.into_inner()).push(line);
        }
    });
}

impl SpawnedProcess {
    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn pgid(&self) -> u32 {
        self.pgid
    }

    /// Non-blocking exit check. `Some(status)` once the shell child has exited.
    pub fn try_status(&mut self) -> std::io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }

    pub fn recent_output(&self) -> Vec<String> {
        self.output
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .lines()
    }

    pub fn has_output(&self) -> bool {
        !self
            .output
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_empty()
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
