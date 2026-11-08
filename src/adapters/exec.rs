//! Bounded execution of external commands. Every host call goes through here.

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::domain::error::{ErrorKind, Result};

const POLL: Duration = Duration::from_millis(5);
/// How long to wait for output after exit. A background grandchild can hold a pipe open.
const DRAIN: Duration = Duration::from_millis(200);

#[derive(Debug, Clone)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    /// `None` waits indefinitely; pass it only where the caller has no limit.
    pub timeout: Option<Duration>,
    pub max_output: usize,
}

impl CommandSpec {
    pub fn new(
        program: impl Into<String>,
        args: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
            env: Vec::new(),
            timeout: Some(Duration::from_secs(10)),
            max_output: 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub exit: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl CommandOutput {
    pub fn success(&self) -> bool {
        self.exit == Some(0)
    }
}

/// Runs `spec` with stdin closed, killing it on timeout or excess output.
pub fn run(spec: &CommandSpec) -> Result<CommandOutput> {
    let mut child = Command::new(&spec.program)
        .args(&spec.args)
        .envs(spec.env.iter().map(|(key, value)| (key, value)))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| ErrorKind::Spawn {
            program: spec.program.clone(),
            source,
        })?;

    let overflow = Arc::new(AtomicBool::new(false));
    let open = Arc::new(AtomicUsize::new(2));
    let stdout = capture(child.stdout.take(), spec.max_output, &overflow, &open);
    let stderr = capture(child.stderr.take(), spec.max_output, &overflow, &open);

    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if overflow.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ErrorKind::OutputTooLarge {
                program: spec.program.clone(),
                limit: spec.max_output,
            }
            .into());
        }
        if let Some(after) = spec.timeout.filter(|limit| started.elapsed() >= *limit) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ErrorKind::CommandTimeout {
                program: spec.program.clone(),
                after,
            }
            .into());
        }
        thread::sleep(POLL);
    };

    let drained = Instant::now();
    while open.load(Ordering::Acquire) > 0 && drained.elapsed() < DRAIN {
        thread::sleep(POLL);
    }
    if overflow.load(Ordering::Relaxed) {
        return Err(ErrorKind::OutputTooLarge {
            program: spec.program.clone(),
            limit: spec.max_output,
        }
        .into());
    }

    Ok(CommandOutput {
        exit: status.code(),
        stdout: taken(&stdout),
        stderr: taken(&stderr),
    })
}

fn capture(
    stream: Option<impl Read + Send + 'static>,
    limit: usize,
    overflow: &Arc<AtomicBool>,
    open: &Arc<AtomicUsize>,
) -> Arc<Mutex<Vec<u8>>> {
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let (sink, overflow, open) = (buffer.clone(), overflow.clone(), open.clone());
    thread::spawn(move || {
        if let Some(mut stream) = stream {
            let mut chunk = [0_u8; 8192];
            while let Ok(read) = stream.read(&mut chunk) {
                if read == 0 {
                    break;
                }
                let Ok(mut sink) = sink.lock() else { break };
                if sink.len() + read > limit {
                    overflow.store(true, Ordering::Relaxed);
                    break;
                }
                sink.extend_from_slice(&chunk[..read]);
            }
        }
        open.fetch_sub(1, Ordering::Release);
    });
    buffer
}

fn taken(buffer: &Mutex<Vec<u8>>) -> Vec<u8> {
    buffer.lock().map(|bytes| bytes.clone()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shell(script: &str) -> CommandSpec {
        CommandSpec::new("/bin/sh", ["-c", script])
    }

    #[test]
    fn captures_stdout_stderr_and_exit() {
        let output = run(&shell("printf out; printf err >&2; exit 3")).unwrap();
        assert_eq!(output.exit, Some(3));
        assert_eq!(output.stdout, b"out");
        assert_eq!(output.stderr, b"err");
    }

    #[test]
    fn a_slow_command_is_killed_at_its_timeout() {
        let mut spec = shell("sleep 5");
        spec.timeout = Some(Duration::from_millis(50));
        let started = Instant::now();
        let error = run(&spec).unwrap_err();
        assert!(matches!(error.kind(), ErrorKind::CommandTimeout { .. }));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn excess_output_is_refused() {
        let mut spec = shell("yes");
        spec.max_output = 1024;
        let error = run(&spec).unwrap_err();
        assert!(matches!(error.kind(), ErrorKind::OutputTooLarge { .. }));
    }

    #[test]
    fn a_background_process_holding_the_pipe_does_not_block_return() {
        let started = Instant::now();
        let output = run(&shell("sleep 5 & printf done")).unwrap();
        assert!(output.success());
        assert_eq!(output.stdout, b"done");
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn a_missing_program_is_a_spawn_error() {
        let error = run(&CommandSpec::new("/no/such/program", [""; 0])).unwrap_err();
        assert!(matches!(error.kind(), ErrorKind::Spawn { .. }));
    }
}
