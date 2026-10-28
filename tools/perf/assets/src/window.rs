//! Measurement window controlled over `perf stat --control` FIFOs (std only).
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

use crate::Failure;

/// Without FIFOs the window is a no-op: allocation counts and digests still work.
pub struct Window {
    fifos: Option<(File, File)>,
}

impl Window {
    pub fn open(control: Option<&Path>, ack: Option<&Path>) -> Result<Self, Failure> {
        let fifos = match (control, ack) {
            (Some(control), Some(ack)) => {
                let open = |path: &Path, write: bool| {
                    OpenOptions::new()
                        .read(!write)
                        .write(write)
                        .open(path)
                        .map_err(|e| format!("{}: {e}", path.display()))
                };
                Some((open(control, true)?, open(ack, false)?))
            }
            (None, None) => None,
            _ => return Err("--control and --ack go together".into()),
        };
        Ok(Self { fifos })
    }

    fn command(&mut self, word: &str) -> Result<(), Failure> {
        let Some((control, ack)) = self.fifos.as_mut() else {
            return Ok(());
        };
        control
            .write_all(format!("{word}\n").as_bytes())
            .map_err(|e| format!("control write: {e}"))?;
        // perf terminates each ack with "\n" and a NUL; read to the newline, ignoring NULs.
        let mut reply = Vec::new();
        let mut byte = [0u8; 1];
        while byte[0] != b'\n' {
            ack.read_exact(&mut byte)
                .map_err(|e| format!("control ack: {e}"))?;
            if byte[0] != 0 {
                reply.push(byte[0]);
            }
        }
        if reply != b"ack\n" {
            return Err(format!("control ack was {reply:?}"));
        }
        Ok(())
    }

    pub fn enable(&mut self) -> Result<(), Failure> {
        self.command("enable")
    }

    pub fn disable(&mut self) -> Result<(), Failure> {
        self.command("disable")
    }
}
