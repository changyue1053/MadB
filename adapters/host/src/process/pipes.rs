use super::io_error;
use madb_app::process::{CapturedOutput, ProcessPolicy, ProcessResult, StopReason};
use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use std::{
    fs::File,
    io::{Read, Write},
    os::fd::AsFd,
};

pub(super) fn nonblocking(pipe: &impl AsFd) -> ProcessResult<()> {
    let flags = fcntl_getfl(pipe).map_err(|e| io_error("pipe", e.into()))?;
    fcntl_setfl(pipe, flags | OFlags::NONBLOCK).map_err(|e| io_error("pipe", e.into()))
}

pub(super) struct Capture<R> {
    stream: Option<R>,
    pub output: CapturedOutput,
    pub eof: bool,
    limit: usize,
}
impl<R: Read> Capture<R> {
    pub(super) fn new(stream: R, limit: usize) -> Self {
        Self {
            stream: Some(stream),
            output: CapturedOutput::default(),
            eof: false,
            limit,
        }
    }
    pub(super) fn pump(&mut self) -> ProcessResult<()> {
        let Some(stream) = self.stream.as_mut() else {
            return Ok(());
        };
        let mut buffer = [0; 4096];
        // Fairness matters: a flooding stream must not starve cancellation,
        // the other pipe, the child's stdin or the execution deadline.
        for _ in 0..8 {
            match stream.read(&mut buffer) {
                Ok(0) => {
                    self.eof = true;
                    self.stream.take();
                    break;
                }
                Ok(size) => {
                    self.output.observed_bytes =
                        self.output.observed_bytes.saturating_add(size as u64);
                    let keep = size.min(self.limit - self.output.bytes.len());
                    self.output.bytes.extend_from_slice(&buffer[..keep]);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => {
                    self.stream.take();
                    return Err(io_error("pipe-read", e));
                }
            }
        }
        Ok(())
    }
}

pub(super) struct Pipes {
    input: Option<File>,
    pub written: usize,
    pub out: Capture<File>,
    pub err: Capture<File>,
}
impl Pipes {
    pub fn new(child: &mut subprocess::Job, policy: &ProcessPolicy) -> ProcessResult<Self> {
        let input = child.stdin.take().expect("piped stdin");
        let out = child.stdout.take().expect("piped stdout");
        let err = child.stderr.take().expect("piped stderr");
        nonblocking(&input)?;
        nonblocking(&out)?;
        nonblocking(&err)?;
        Ok(Self {
            input: Some(input),
            written: 0,
            out: Capture::new(out, policy.stdout_limit),
            err: Capture::new(err, policy.stderr_limit),
        })
    }
    pub fn close_input(&mut self) {
        self.input.take();
    }
    pub fn pump(&mut self, bytes: &[u8]) -> ProcessResult<Option<StopReason>> {
        let mut stop = None;
        if let Some(input) = &mut self.input {
            if self.written < bytes.len() {
                let end = bytes.len().min(self.written + 16 * 1024);
                match input.write(&bytes[self.written..end]) {
                    Ok(0) => {
                        stop = Some(StopReason::StdinClosed);
                        self.input.take();
                    }
                    Ok(size) => self.written += size,
                    Err(e)
                        if [
                            std::io::ErrorKind::WouldBlock,
                            std::io::ErrorKind::Interrupted,
                        ]
                        .contains(&e.kind()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {
                        stop = Some(StopReason::StdinClosed);
                        self.input.take();
                    }
                    Err(e) => {
                        self.input.take();
                        return Err(io_error("pipe-write", e));
                    }
                }
            }
            if self.written == bytes.len() {
                self.input.take();
            }
        }
        self.out.pump()?;
        self.err.pump()?;
        if self.out.output.truncated() {
            stop = Some(StopReason::StdoutLimit);
        }
        Ok(stop)
    }
    pub fn eof(&self) -> bool {
        self.out.eof && self.err.eof
    }
}
