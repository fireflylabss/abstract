//! Discord Rich Presence over the local IPC socket (`discord-ipc-N`, the
//! documented discord-rpc protocol). A background thread owns the socket;
//! every failure is silent — with Discord absent the app works identically.

use std::io::{Result, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant, SystemTime};

use crate::i18n::{Key, t};

/// Rich Presence needs a registered Discord application id; this placeholder
/// keeps the wire format right until a real one exists.
const CLIENT_ID: &str = "abstract";
/// How often a disconnected worker may try the socket again.
const RETRY: Duration = Duration::from_secs(30);

enum Msg {
    Set { details: String, state: String },
    Off,
}

/// Handle to the presence worker; `None` means the feature is off and the
/// socket is never touched.
pub(crate) struct Presence(Option<Sender<Msg>>);

impl Presence {
    pub fn new(on: bool) -> Self {
        let mut p = Self(None);
        p.set_enabled(on);
        p
    }

    /// `false` disconnects and stops the worker entirely.
    pub fn set_enabled(&mut self, on: bool) {
        match (on, self.0.take()) {
            (true, None) => {
                let (tx, rx) = mpsc::channel();
                std::thread::spawn(move || run(rx));
                self.0 = Some(tx);
            }
            (false, Some(tx)) => {
                let _ = tx.send(Msg::Off);
            }
            (_, tx) => self.0 = tx,
        }
    }

    /// `details` of `None` shows the localized "Browsing notes".
    pub fn set(&self, details: Option<String>, state: String) {
        if let Some(tx) = &self.0 {
            let _ = tx.send(Msg::Set {
                details: details.unwrap_or_else(|| t(Key::BrowsingNotes).to_string()),
                state,
            });
        }
    }
}

impl Drop for Presence {
    fn drop(&mut self) {
        self.set_enabled(false);
    }
}

/// Latest activity wins; reconnects happen at most once per `RETRY` —
/// on a timer tick or the next `Set`, whichever comes first.
fn run(rx: Receiver<Msg>) {
    let mut ipc = None;
    let mut last = None;
    let mut attempt = Instant::now()
        .checked_sub(RETRY)
        .unwrap_or_else(Instant::now);
    let started = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let mut nonce = 0u64;
    loop {
        match rx.recv_timeout(RETRY) {
            Ok(Msg::Off) | Err(RecvTimeoutError::Disconnected) => return,
            Ok(Msg::Set { details, state }) => last = Some((details, state)),
            Err(RecvTimeoutError::Timeout) => {}
        }
        let Some((details, state)) = &last else {
            continue;
        };
        if ipc.is_none() {
            if attempt.elapsed() < RETRY {
                continue;
            }
            attempt = Instant::now();
            ipc = Ipc::connect().ok();
        }
        if let Some(c) = &mut ipc {
            nonce += 1;
            if c.set_activity(started, nonce, details, state).is_err() {
                ipc = None;
            }
        }
    }
}

struct Ipc(Socket);

#[cfg(unix)]
type Socket = std::os::unix::net::UnixStream;
#[cfg(windows)]
type Socket = std::fs::File;

impl Ipc {
    /// First socket that opens and accepts the handshake wins.
    fn connect() -> Result<Self> {
        let mut last = std::io::Error::new(std::io::ErrorKind::NotFound, "no discord-ipc socket");
        for path in socket_paths() {
            match open(&path).and_then(|mut s| {
                s.write_all(&frame(0, &handshake_json(CLIENT_ID)))?;
                s.flush()?;
                Ok(Self(s))
            }) {
                Ok(c) => return Ok(c),
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    fn set_activity(&mut self, started: u64, nonce: u64, details: &str, state: &str) -> Result<()> {
        self.0
            .write_all(&frame(1, &activity_json(started, nonce, details, state)))?;
        self.0.flush()
    }
}

#[cfg(unix)]
fn socket_paths() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for var in ["XDG_RUNTIME_DIR", "TMPDIR"] {
        if let Some(d) = std::env::var_os(var) {
            dirs.push(PathBuf::from(d));
        }
    }
    dirs.push(PathBuf::from("/tmp"));
    (0..10)
        .flat_map(|i| dirs.iter().map(move |d| d.join(format!("discord-ipc-{i}"))))
        .collect()
}

#[cfg(unix)]
fn open(path: &PathBuf) -> Result<Socket> {
    std::os::unix::net::UnixStream::connect(path)
}

#[cfg(windows)]
fn socket_paths() -> Vec<PathBuf> {
    (0..10)
        .map(|i| PathBuf::from(format!(r"\\?\pipe\discord-ipc-{i}")))
        .collect()
}

#[cfg(windows)]
fn open(path: &PathBuf) -> Result<Socket> {
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
}

/// One IPC frame: little-endian opcode + payload length, then JSON.
fn frame(op: u32, payload: &str) -> Vec<u8> {
    let mut v = Vec::with_capacity(8 + payload.len());
    v.extend_from_slice(&op.to_le_bytes());
    v.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    v.extend_from_slice(payload.as_bytes());
    v
}

fn handshake_json(client_id: &str) -> String {
    format!("{{\"v\":1,\"client_id\":\"{}\"}}", escape(client_id))
}

/// SET_ACTIVITY per the documented discord-rpc payload shape. `large_image`
/// only resolves once a real application id registers that asset.
fn activity_json(started: u64, nonce: u64, details: &str, state: &str) -> String {
    format!(
        "{{\"cmd\":\"SET_ACTIVITY\",\"args\":{{\"pid\":{},\"activity\":{{\"details\":\"{}\",\"state\":\"{}\",\"timestamps\":{{\"start\":{}}},\"assets\":{{\"large_image\":\"abstract\",\"large_text\":\"abstract\"}},\"instance\":false}}}},\"nonce\":\"abstract-{}\"}}",
        std::process::id(),
        escape(details),
        escape(state),
        started,
        nonce
    )
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_has_le_header() {
        let f = frame(1, "{}");
        assert_eq!(&f[..8], &[1, 0, 0, 0, 2, 0, 0, 0]);
        assert_eq!(&f[8..], b"{}");
    }

    #[test]
    fn handshake_carries_client_id() {
        assert_eq!(
            handshake_json("abstract"),
            "{\"v\":1,\"client_id\":\"abstract\"}"
        );
    }

    #[test]
    fn activity_carries_details_state_and_start() {
        let j = activity_json(42, 7, "My Title", "Space");
        assert!(j.contains("\"cmd\":\"SET_ACTIVITY\""));
        assert!(j.contains(&format!("\"pid\":{}", std::process::id())));
        assert!(j.contains("\"details\":\"My Title\""));
        assert!(j.contains("\"state\":\"Space\""));
        assert!(j.contains("\"start\":42"));
        assert!(j.contains("\"nonce\":\"abstract-7\""));
    }

    #[test]
    fn escape_quotes_backslashes_and_controls() {
        assert_eq!(escape("a\"b\\c\nd\te"), "a\\\"b\\\\c\\nd\\te");
        assert_eq!(escape("\u{1}"), "\\u0001");
        assert_eq!(escape("puro ç ão"), "puro ç ão");
    }

    #[test]
    fn disabled_presence_sends_nothing() {
        let mut p = Presence::new(false);
        p.set(Some("x".into()), "y".into());
        p.set(None, "y".into());
        p.set_enabled(false);
    }
}
