use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::windows::io::AsRawHandle;

use serde_json::{Value, json};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Pipes::PeekNamedPipe;

const HANDSHAKE: u32 = 0;
const FRAME: u32 = 1;
const CLOSE: u32 = 2;

pub struct Discord {
    client_id: String,
    pipe: Option<File>,
    nonce: u64,
}

impl Discord {
    pub fn new(client_id: &str) -> Self {
        Self { client_id: client_id.to_string(), pipe: None, nonce: 0 }
    }

    pub fn connected(&mut self) -> bool {
        let Some(pipe) = &self.pipe else { return false };
        let mut available = 0u32;
        let alive = unsafe {
            PeekNamedPipe(HANDLE(pipe.as_raw_handle()), None, 0, None, Some(&mut available), None).is_ok()
        };
        if !alive {
            self.pipe = None;
            return false;
        }
        if available > 0 {
            let mut junk = vec![0u8; available as usize];
            if self.pipe.as_mut().map(|p| p.read_exact(&mut junk)).transpose().is_err() {
                self.pipe = None;
                return false;
            }
        }
        true
    }

    pub fn connect(&mut self) -> io::Result<()> {
        for i in 0..10 {
            let path = format!(r"\\.\pipe\discord-ipc-{i}");
            let Ok(pipe) = OpenOptions::new().read(true).write(true).open(&path) else { continue };
            self.pipe = Some(pipe);
            let hello = json!({ "v": 1, "client_id": self.client_id });
            match self.send(HANDSHAKE, &hello) {
                Ok(reply) if reply["evt"] == "READY" => return Ok(()),
                Ok(reply) => {
                    self.pipe = None;
                    return Err(io::Error::other(reply.to_string()));
                }
                Err(_) => self.pipe = None,
            }
        }
        Err(io::Error::new(io::ErrorKind::NotFound, "discord is not running"))
    }

    pub fn set_activity(&mut self, activity: Option<&Value>) -> io::Result<Value> {
        self.nonce += 1;
        let payload = json!({
            "cmd": "SET_ACTIVITY",
            "args": { "pid": std::process::id(), "activity": activity },
            "nonce": self.nonce.to_string(),
        });
        let reply = self.send(FRAME, &payload);
        if reply.is_err() {
            self.pipe = None;
        }
        reply
    }

    fn send(&mut self, op: u32, payload: &Value) -> io::Result<Value> {
        let pipe = self.pipe.as_mut().ok_or_else(|| io::Error::from(io::ErrorKind::NotConnected))?;
        let body = serde_json::to_vec(payload)?;
        let mut frame = Vec::with_capacity(body.len() + 8);
        frame.extend_from_slice(&op.to_le_bytes());
        frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
        frame.extend_from_slice(&body);
        pipe.write_all(&frame)?;
        pipe.flush()?;

        let mut header = [0u8; 8];
        pipe.read_exact(&mut header)?;
        let reply_op = u32::from_le_bytes(header[0..4].try_into().unwrap());
        let len = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
        let mut data = vec![0u8; len];
        pipe.read_exact(&mut data)?;
        let reply: Value = serde_json::from_slice(&data).unwrap_or(Value::Null);
        if reply_op == CLOSE {
            return Err(io::Error::new(io::ErrorKind::ConnectionAborted, reply.to_string()));
        }
        Ok(reply)
    }
}
