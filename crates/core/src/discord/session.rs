//! Background manager for Discord Rich Presence lifecycle.
//!
//! Maintains connection to Discord, handles reconnect retries gracefully,
//! tracks multiple running games with last-launched priority, and ensures
//! presence only fires for games (never Battle.net).

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::activity::Activity;
use super::apps::{self, DiscordApp};
use super::character;
use super::ipc::DiscordIpc;
use crate::product::ProductCode;

const RECONNECT_INTERVAL: Duration = Duration::from_secs(10);
const LOOP_POLL_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone)]
struct ActiveGame {
    code: ProductCode,
    name: String,
    pid: u32,
    start_secs: u64,
    product_dir: Option<PathBuf>,
    custom_app_id: Option<String>,
}

enum Command {
    GameStarted(ActiveGame),
    GameExited(ProductCode),
    ConfigChanged {
        enabled: bool,
        include_character: bool,
    },
    Shutdown,
}

struct SessionInner {
    tx: Sender<Command>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl Drop for SessionInner {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Shutdown);
        if let Ok(mut guard) = self.handle.lock() {
            if let Some(handle) = guard.take() {
                if handle.thread().id() != std::thread::current().id() {
                    let _ = handle.join();
                }
            }
        }
    }
}

/// A handle to the background Discord RPC manager.
#[derive(Clone)]
pub struct DiscordRpcSession {
    inner: Arc<SessionInner>,
}

impl DiscordRpcSession {
    /// Start the background Discord RPC manager thread.
    pub fn start(enabled: bool, include_character: bool) -> Self {
        let (tx, rx) = mpsc::channel();

        let handle = std::thread::Builder::new()
            .name("discord-rpc".into())
            .spawn(move || run_worker(rx, enabled, include_character))
            .expect("failed to spawn discord-rpc thread");

        Self {
            inner: Arc::new(SessionInner {
                tx,
                handle: Mutex::new(Some(handle)),
            }),
        }
    }

    /// Notify that a game has started running.
    pub fn game_started(
        &self,
        code: ProductCode,
        name: String,
        pid: u32,
        start_secs: u64,
        product_dir: Option<PathBuf>,
        custom_app_id: Option<String>,
    ) {
        let _ = self.inner.tx.send(Command::GameStarted(ActiveGame {
            code,
            name,
            pid,
            start_secs,
            product_dir,
            custom_app_id,
        }));
    }

    /// Notify that a game has exited.
    pub fn game_exited(&self, code: ProductCode) {
        let _ = self.inner.tx.send(Command::GameExited(code));
    }

    /// Update configuration toggles (e.g. from Settings).
    pub fn update_config(&self, enabled: bool, include_character: bool) {
        let _ = self.inner.tx.send(Command::ConfigChanged {
            enabled,
            include_character,
        });
    }

    /// Stop the background worker.
    pub fn shutdown(&self) {
        let _ = self.inner.tx.send(Command::Shutdown);
    }
}

fn run_worker(rx: Receiver<Command>, mut enabled: bool, mut include_character: bool) {
    let mut games: Vec<ActiveGame> = Vec::new();
    let mut ipc: Option<DiscordIpc> = None;
    let mut last_attempt = Instant::now() - RECONNECT_INTERVAL;
    let mut dirty = false;
    let mut last_activity: Option<Activity> = None;

    loop {
        // Block without polling when RPC is disabled or no game is running.
        let recv = if enabled && !games.is_empty() {
            rx.recv_timeout(LOOP_POLL_TIMEOUT)
        } else {
            rx.recv().map_err(|_| RecvTimeoutError::Disconnected)
        };

        match recv {
            Ok(Command::GameStarted(game)) => {
                tracing::info!(
                    "Discord RPC received GameStarted for {} (pid {})",
                    game.name,
                    game.pid
                );
                games.retain(|g| g.code != game.code);
                games.push(game);
                dirty = true;
                last_attempt = Instant::now() - RECONNECT_INTERVAL;
            }
            Ok(Command::GameExited(code)) => {
                tracing::info!("Discord RPC received GameExited for {}", code.as_str());
                games.retain(|g| g.code != code);
                dirty = true;
                last_attempt = Instant::now() - RECONNECT_INTERVAL;
            }
            Ok(Command::ConfigChanged {
                enabled: new_enabled,
                include_character: new_include_char,
            }) => {
                if enabled != new_enabled || include_character != new_include_char {
                    tracing::info!(
                        "Discord RPC config updated: enabled={new_enabled}, include_character={new_include_char}"
                    );
                    enabled = new_enabled;
                    include_character = new_include_char;
                    dirty = true;
                    if enabled {
                        last_attempt = Instant::now() - RECONNECT_INTERVAL;
                    }
                }
            }
            Ok(Command::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {
                // Periodic loop: check if character info or game activity changed
            }
        }

        if !enabled || games.is_empty() {
            if let Some(mut client) = ipc.take() {
                client.close();
            }
            dirty = false;
            last_activity = None;
            continue;
        }

        let Some(active) = games.last() else {
            continue;
        };

        let app = resolve_app(active);

        // Check if existing IPC matches the target application
        if let Some(client) = &ipc {
            if client.client_id() != app.client_id.as_str() {
                ipc = None;
                dirty = true;
                last_activity = None;
                last_attempt = Instant::now() - RECONNECT_INTERVAL;
            }
        }

        // Attempt connection if not connected and reconnect interval elapsed
        if ipc.is_none() && last_attempt.elapsed() >= RECONNECT_INTERVAL {
            last_attempt = Instant::now();
            match DiscordIpc::connect(&app.client_id) {
                Ok(client) => {
                    tracing::info!("connected to Discord IPC for {}", app.title);
                    ipc = Some(client);
                    dirty = true;
                }
                Err(e) => {
                    tracing::debug!("Discord IPC not connected: {e}");
                }
            }
        }

        // Update presence if connected and state changed
        if let Some(client) = ipc.as_mut() {
            let activity = build_activity(active, &app, include_character);
            if dirty || last_activity.as_ref() != Some(&activity) {
                tracing::debug!("setting Discord activity: {activity:?}");
                if let Err(e) = client.set_activity(std::process::id(), Some(&activity)) {
                    tracing::warn!("failed to send activity to Discord: {e}");
                    ipc = None; // Connection broken, retry later
                    last_activity = None;
                } else {
                    last_activity = Some(activity);
                    dirty = false;
                }
            }
        }
    }

    if let Some(mut client) = ipc.take() {
        client.close();
    }
}

fn resolve_app(game: &ActiveGame) -> DiscordApp {
    apps::resolve_game_app(&game.code, &game.name, game.custom_app_id.as_deref())
}

fn build_activity(game: &ActiveGame, app: &DiscordApp, include_character: bool) -> Activity {
    let char_info = if include_character && apps::is_wow_app(app) {
        game.product_dir
            .as_deref()
            .and_then(|p| character::detect_character_with_start_time(p, Some(game.start_secs)))
    } else {
        None
    };

    let (details, state) = match char_info {
        Some(info) => (Some(info.format_details()), Some(info.format_state())),
        None => {
            if app.title == app.default_details {
                (Some("In Game".to_owned()), None)
            } else {
                (
                    Some(app.default_details.to_string()),
                    Some("In Game".to_owned()),
                )
            }
        }
    };

    let mut activity = Activity::new(details, state, Some(game.start_secs));
    activity.assets = Some(super::activity::Assets {
        large_image: None,
        large_text: Some(app.title.clone()),
        small_image: None,
        small_text: None,
    });
    activity
}
