//! Discord IPC client over Unix domain sockets on Linux.
//!
//! Communicates with the local Discord desktop client via the standard
//! Discord IPC framing: 4-byte opcode (LE) + 4-byte length (LE) + JSON payload.

use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

use super::activity::Activity;

const OP_HANDSHAKE: u32 = 0;
const OP_FRAME: u32 = 1;
const OP_CLOSE: u32 = 2;

const TIMEOUT: Duration = Duration::from_millis(1000);
const MAX_PACKET_LEN: usize = 64 * 1024;

const FLATPAK_APPS: &[&str] = &[
    "com.discordapp.Discord",
    "com.discordapp.DiscordCanary",
    "com.discordapp.DiscordPTB",
    "dev.vencord.Vesktop",
    "io.github.shifteight.webcord",
];

/// Return all existing Discord IPC Unix socket candidates in preference order.
fn find_sockets() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    // 1. Check XDG_RUNTIME_DIR or /run/user/<uid>
    let runtime_dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            // SAFETY: getuid(2) is always safe to call and never fails.
            let uid = unsafe { libc::getuid() };
            let p = PathBuf::from(format!("/run/user/{uid}"));
            p.exists().then_some(p)
        });

    if let Some(ref dir) = runtime_dir {
        for i in 0..10 {
            candidates.push(dir.join(format!("discord-ipc-{i}")));
        }
        for app in FLATPAK_APPS {
            for i in 0..10 {
                candidates.push(dir.join(format!("app/{app}/discord-ipc-{i}")));
                candidates.push(dir.join(format!(".flatpak/{app}/xdg-run/discord-ipc-{i}")));
            }
        }
        for i in 0..10 {
            candidates.push(dir.join(format!("snap.discord/discord-ipc-{i}")));
        }
    }

    // 2. Check ~/.var/app (Flatpak home directory)
    if let Some(home) = std::env::var_os("HOME") {
        let home_dir = PathBuf::from(home);
        for app in FLATPAK_APPS {
            for i in 0..10 {
                candidates.push(home_dir.join(format!(".var/app/{app}/xdg-run/discord-ipc-{i}")));
            }
        }
    }

    // 3. Fallback: /tmp
    for i in 0..10 {
        candidates.push(PathBuf::from(format!("/tmp/discord-ipc-{i}")));
    }

    candidates.into_iter().filter(|p| p.exists()).collect()
}

/// Find the first available Discord IPC Unix socket.
pub fn find_socket() -> Option<PathBuf> {
    find_sockets().into_iter().next()
}

/// A connection to a local Discord client.
pub struct DiscordIpc {
    stream: UnixStream,
    client_id: String,
    closed: bool,
}

impl DiscordIpc {
    /// Connect to Discord using the given `client_id` (Discord Application ID).
    pub fn connect(client_id: &str) -> io::Result<Self> {
        let sockets = find_sockets();
        if sockets.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "no Discord IPC socket found",
            ));
        }

        let mut last_err = None;
        for path in sockets {
            tracing::debug!("trying Discord IPC at {}", path.display());
            match UnixStream::connect(&path).and_then(|s| Self::handshake(s, client_id)) {
                Ok(client) => {
                    tracing::info!("connected to Discord IPC at {}", path.display());
                    return Ok(client);
                }
                Err(e) => {
                    tracing::debug!("Discord IPC candidate {} failed: {e}", path.display());
                    last_err = Some(e);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "no Discord IPC socket found")
        }))
    }

    fn handshake(mut stream: UnixStream, client_id: &str) -> io::Result<Self> {
        stream.set_read_timeout(Some(TIMEOUT))?;
        stream.set_write_timeout(Some(TIMEOUT))?;

        let handshake = serde_json::json!({
            "v": 1,
            "client_id": client_id,
        });
        let payload = serde_json::to_vec(&handshake)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        send_packet(&mut stream, OP_HANDSHAKE, &payload)?;

        // Read the READY dispatch frame from Discord
        let (op, reply) = read_packet(&mut stream)?;
        if op != OP_FRAME {
            let msg = String::from_utf8_lossy(&reply);
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("expected READY frame (opcode 1), got opcode {op}: {msg}"),
            ));
        }

        let resp: serde_json::Value = serde_json::from_slice(&reply)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        if resp.get("evt").and_then(|v| v.as_str()) == Some("ERROR") {
            return Err(io::Error::other(format!("Discord handshake error: {resp:?}")));
        }
        if resp.get("evt").and_then(|v| v.as_str()) != Some("READY") {
            tracing::debug!("Discord handshake response: {resp:?}");
        }

        Ok(Self {
            stream,
            client_id: client_id.to_owned(),
            closed: false,
        })
    }

    /// The client_id this IPC connection is authenticated with.
    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    /// Set or clear the Discord Rich Presence activity.
    pub fn set_activity(&mut self, pid: u32, activity: Option<&Activity>) -> io::Result<()> {
        let payload = serde_json::json!({
            "cmd": "SET_ACTIVITY",
            "args": {
                "pid": pid,
                "activity": activity,
            },
            "nonce": "1",
        });
        let data = serde_json::to_vec(&payload)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        send_packet(&mut self.stream, OP_FRAME, &data)?;

        // Check the reply to verify whether Discord accepted the activity or closed the socket.
        match read_packet(&mut self.stream) {
            Ok((_op, reply)) => {
                let resp: serde_json::Value = serde_json::from_slice(&reply).unwrap_or_default();
                if resp.get("evt").and_then(|v| v.as_str()) == Some("ERROR") {
                    tracing::warn!("Discord SET_ACTIVITY error: {resp:?}");
                    return Err(io::Error::other(format!("Discord error: {resp:?}")));
                }
                tracing::debug!("Discord SET_ACTIVITY accepted: {resp:?}");
            }
            Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => {
                tracing::debug!("Discord SET_ACTIVITY reply timed out: {e}");
            }
            Err(e) => return Err(e),
        }
        Ok(())
    }

    /// Cleanly close the Discord IPC connection.
    pub fn close(&mut self) {
        if !self.closed {
            self.closed = true;
            let _ = send_packet(&mut self.stream, OP_CLOSE, b"{}");
            let _ = self.stream.shutdown(std::net::Shutdown::Both);
        }
    }
}

impl Drop for DiscordIpc {
    fn drop(&mut self) {
        self.close();
    }
}

fn send_packet(stream: &mut UnixStream, opcode: u32, payload: &[u8]) -> io::Result<()> {
    let mut header = [0u8; 8];
    header[0..4].copy_from_slice(&opcode.to_le_bytes());
    let len = u32::try_from(payload.len())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    header[4..8].copy_from_slice(&len.to_le_bytes());
    stream.write_all(&header)?;
    stream.write_all(payload)?;
    stream.flush()
}

fn read_packet(stream: &mut UnixStream) -> io::Result<(u32, Vec<u8>)> {
    let mut header = [0u8; 8];
    stream.read_exact(&mut header)?;
    let opcode = u32::from_le_bytes(
        header[0..4]
            .try_into()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid opcode header"))?,
    );
    let length = u32::from_le_bytes(
        header[4..8]
            .try_into()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid length header"))?,
    ) as usize;
    if length > MAX_PACKET_LEN {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "packet too large"));
    }
    let mut buf = vec![0u8; length];
    stream.read_exact(&mut buf)?;
    Ok((opcode, buf))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_framing_round_trip() {
        let (mut client, mut server) = UnixStream::pair().unwrap();
        let payload = b"{\"test\":true}";
        send_packet(&mut client, OP_FRAME, payload).unwrap();

        let (op, received) = read_packet(&mut server).unwrap();
        assert_eq!(op, OP_FRAME);
        assert_eq!(received, payload);
    }

    #[test]
    fn handshake_and_set_activity_round_trip() {
        let (client_stream, mut server_stream) = UnixStream::pair().unwrap();

        let server = std::thread::spawn(move || {
            let (op, body) = read_packet(&mut server_stream).unwrap();
            assert_eq!(op, OP_HANDSHAKE);
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(req["client_id"], "12345");
            send_packet(&mut server_stream, OP_FRAME, br#"{"evt":"READY"}"#).unwrap();

            let (op, body) = read_packet(&mut server_stream).unwrap();
            assert_eq!(op, OP_FRAME);
            let act: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(act["cmd"], "SET_ACTIVITY");
            send_packet(&mut server_stream, OP_FRAME, br#"{"cmd":"SET_ACTIVITY"}"#).unwrap();

            let (op, _) = read_packet(&mut server_stream).unwrap();
            assert_eq!(op, OP_CLOSE);
            // Closing twice / dropping must not send a second OP_CLOSE.
            assert!(read_packet(&mut server_stream).is_err());
        });

        let mut ipc = DiscordIpc::handshake(client_stream, "12345").unwrap();
        let activity = Activity::new(Some("Arthas".into()), Some("In World".into()), Some(100));
        ipc.set_activity(42, Some(&activity)).unwrap();
        ipc.close();
        drop(ipc);

        server.join().unwrap();
    }

    #[test]
    fn set_activity_returns_err_when_peer_closes() {
        let (client_stream, mut server_stream) = UnixStream::pair().unwrap();
        let server = std::thread::spawn(move || {
            let _ = read_packet(&mut server_stream).unwrap();
            send_packet(&mut server_stream, OP_FRAME, br#"{"evt":"READY"}"#).unwrap();
            let _ = read_packet(&mut server_stream).unwrap();
            drop(server_stream);
        });

        let mut ipc = DiscordIpc::handshake(client_stream, "12345").unwrap();
        let activity = Activity::new(Some("Arthas".into()), None, None);
        assert!(ipc.set_activity(42, Some(&activity)).is_err());

        server.join().unwrap();
    }
}
