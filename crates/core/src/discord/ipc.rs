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

/// Find an available Discord IPC Unix socket.
pub fn find_socket() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    // 1. Check XDG_RUNTIME_DIR or /run/user/<uid>
    let runtime_dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            let uid = unsafe { libc::getuid() };
            let p = PathBuf::from(format!("/run/user/{uid}"));
            p.exists().then_some(p)
        });

    if let Some(ref dir) = runtime_dir {
        // Direct sockets
        for i in 0..10 {
            candidates.push(dir.join(format!("discord-ipc-{i}")));
        }
        // Flatpak app runtimes
        for app in &[
            "com.discordapp.Discord",
            "com.discordapp.DiscordCanary",
            "com.discordapp.DiscordPTB",
            "dev.vencord.Vesktop",
            "io.github.shifteight.webcord",
        ] {
            for i in 0..10 {
                candidates.push(dir.join(format!("app/{app}/discord-ipc-{i}")));
                candidates.push(dir.join(format!(".flatpak/{app}/xdg-run/discord-ipc-{i}")));
            }
        }
        // Snap
        for i in 0..10 {
            candidates.push(dir.join(format!("snap.discord/discord-ipc-{i}")));
        }
    }

    // 2. Check ~/.var/app (Flatpak home directory)
    if let Ok(home) = std::env::var("HOME") {
        let home_dir = PathBuf::from(home);
        for app in &[
            "com.discordapp.Discord",
            "com.discordapp.DiscordCanary",
            "com.discordapp.DiscordPTB",
            "dev.vencord.Vesktop",
            "io.github.shifteight.webcord",
        ] {
            for i in 0..10 {
                candidates.push(home_dir.join(format!(".var/app/{app}/xdg-run/discord-ipc-{i}")));
            }
        }
    }

    // 3. Fallback: /tmp
    for i in 0..10 {
        candidates.push(PathBuf::from(format!("/tmp/discord-ipc-{i}")));
    }

    candidates.into_iter().find(|p| p.exists())
}

/// A connection to a local Discord client.
pub struct DiscordIpc {
    stream: UnixStream,
    client_id: String,
}

impl DiscordIpc {
    /// Connect to Discord using the given `client_id` (Discord Application ID).
    pub fn connect(client_id: &str) -> io::Result<Self> {
        let path = find_socket().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "no Discord IPC socket found")
        })?;

        tracing::info!("connecting to Discord IPC at {}", path.display());
        let mut stream = UnixStream::connect(&path)?;
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
        if resp.get("evt").and_then(|v| v.as_str()) != Some("READY") {
            tracing::info!("Discord handshake response: {resp:?}");
        }

        Ok(Self {
            stream,
            client_id: client_id.to_owned(),
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

        // Check the reply to verify whether Discord accepted the activity
        match read_packet(&mut self.stream) {
            Ok((_op, reply)) => {
                let resp: serde_json::Value = serde_json::from_slice(&reply).unwrap_or_default();
                if resp.get("evt").and_then(|v| v.as_str()) == Some("ERROR") {
                    tracing::warn!("Discord SET_ACTIVITY error: {resp:?}");
                    return Err(io::Error::other(format!("Discord error: {resp:?}")));
                }
                tracing::info!("Discord SET_ACTIVITY accepted: {resp:?}");
            }
            Err(e) => {
                tracing::debug!("Discord SET_ACTIVITY reply read note: {e}");
            }
        }
        Ok(())
    }

    /// Cleanly close the Discord IPC connection.
    pub fn close(&mut self) {
        let _ = send_packet(&mut self.stream, OP_CLOSE, b"{}");
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
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
}
