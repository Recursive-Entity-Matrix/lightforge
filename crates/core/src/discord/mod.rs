//! Discord Rich Presence (RPC) integration for Lightforge.
//!
//! Provides game-only rich presence over local Unix domain sockets without
//! touching game process memory or violating zero-injection constraints.

pub mod activity;
pub mod apps;
pub mod character;
pub mod ipc;
pub mod session;

pub use activity::Activity;
pub use apps::{app_for_name, app_for_product, resolve_game_app, DiscordApp};
pub use character::{detect_character, detect_character_with_start_time};
pub use ipc::DiscordIpc;
pub use session::DiscordRpcSession;
