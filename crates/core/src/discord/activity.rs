//! Discord Rich Presence activity model.

use serde::Serialize;

/// A Discord Rich Presence activity payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Activity {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamps: Option<Timestamps>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assets: Option<Assets>,
}

/// Timestamps displayed as elapsed or remaining time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Timestamps {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<u64>,
}

/// Image and text assets shown in the Rich Presence box.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Assets {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub large_image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub large_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub small_image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub small_text: Option<String>,
}

impl Activity {
    /// Create a new activity for a game with a start timestamp.
    pub fn new(details: Option<String>, state: Option<String>, start_secs: Option<u64>) -> Self {
        Self {
            details,
            state,
            timestamps: start_secs.map(|start| Timestamps { start: Some(start) }),
            assets: None,
        }
    }
}
