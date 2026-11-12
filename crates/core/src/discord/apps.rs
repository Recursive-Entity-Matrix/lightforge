//! Discord Application IDs and asset mappings for Blizzard game families.

use crate::product::ProductCode;

/// Discord Rich Presence identification for a game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscordApp {
    /// Discord Application Client ID (snowflake).
    pub client_id: String,
    /// Game title shown in presence.
    pub title: String,
    /// Default details string (e.g. expansion / branch name).
    pub default_details: String,
}

impl DiscordApp {
    pub fn new(
        client_id: impl Into<String>,
        title: impl Into<String>,
        default_details: impl Into<String>,
    ) -> Self {
        Self {
            client_id: client_id.into(),
            title: title.into(),
            default_details: default_details.into(),
        }
    }
}

// Discord-verified Application IDs from Discord's detectable applications database:
// https://discord.com/api/v10/applications/detectable
pub const APP_ID_WOW: &str = "356875762940379136";
pub const APP_ID_WOW_CLASSIC: &str = "615752773690064908";
pub const APP_ID_WOW_PTR: &str = "426525131276550144";
pub const APP_ID_WOW_FOREVER: &str = "1553234742529364018";
pub const APP_ID_WARCRAFT_1_REMASTERED: &str = "1442719732347441285";
pub const APP_ID_WARCRAFT_2_REMASTERED: &str = "1445095254217719879";
pub const APP_ID_WARCRAFT_3_REFORGED: &str = "1501073435047104573";
pub const APP_ID_WARCRAFT_3_CLASSIC: &str = "424719708785344525";

pub const APP_ID_HEARTHSTONE: &str = "356875890958925834";
pub const APP_ID_DIABLO_IV: &str = "1113966530531704943";
pub const APP_ID_OVERWATCH_2: &str = "356875221078245376";
pub const APP_ID_STARCRAFT_2: &str = "358425800766128128";

/// Resolve the Discord application info for a given [`ProductCode`] and optional custom App ID.
pub fn app_for_product(code: &ProductCode, custom_id: Option<&str>) -> DiscordApp {
    if let Some(id) = custom_id {
        let trimmed = id.trim();
        if !trimmed.is_empty() {
            let label = match code {
                ProductCode::Forever => "World of Warcraft: Forever".to_owned(),
                ProductCode::Classic => "World of Warcraft Classic".to_owned(),
                ProductCode::ClassicEra => "World of Warcraft Classic".to_owned(),
                ProductCode::Retail => "World of Warcraft".to_owned(),
                ProductCode::Ptr | ProductCode::Ptr2 => "World of Warcraft Public Test".to_owned(),
                _ => code.as_str().to_owned(),
            };
            return DiscordApp::new(trimmed, label.clone(), label);
        }
    }

    match code {
        ProductCode::Retail => DiscordApp::new(
            APP_ID_WOW,
            "World of Warcraft",
            "World of Warcraft: The War Within",
        ),
        ProductCode::Beta => {
            DiscordApp::new(APP_ID_WOW, "World of Warcraft", "World of Warcraft: Beta")
        }
        ProductCode::Ptr | ProductCode::Ptr2 => DiscordApp::new(
            APP_ID_WOW_PTR,
            "World of Warcraft Public Test",
            "World of Warcraft Public Test",
        ),
        ProductCode::Classic => DiscordApp::new(
            APP_ID_WOW_CLASSIC,
            "World of Warcraft Classic",
            "World of Warcraft Classic",
        ),
        ProductCode::ClassicPtr => DiscordApp::new(
            APP_ID_WOW_CLASSIC,
            "World of Warcraft Classic",
            "World of Warcraft Classic: PTR",
        ),
        ProductCode::ClassicBeta => DiscordApp::new(
            APP_ID_WOW_CLASSIC,
            "World of Warcraft Classic",
            "World of Warcraft Classic: Beta",
        ),
        ProductCode::ClassicEra => DiscordApp::new(
            APP_ID_WOW_CLASSIC,
            "World of Warcraft Classic",
            "World of Warcraft Classic: Era",
        ),
        ProductCode::EraPtr => DiscordApp::new(
            APP_ID_WOW_CLASSIC,
            "World of Warcraft Classic",
            "World of Warcraft Classic: Era PTR",
        ),
        ProductCode::Forever => DiscordApp::new(
            APP_ID_WOW_FOREVER,
            "World of Warcraft: Forever",
            "World of Warcraft: Forever",
        ),
        ProductCode::Custom(custom) => app_for_name(custom, None),
    }
}

/// Resolve the Discord application info for an arbitrary game or process name and optional custom App ID.
pub fn app_for_name(name: &str, custom_id: Option<&str>) -> DiscordApp {
    if let Some(id) = custom_id {
        let trimmed = id.trim();
        if !trimmed.is_empty() {
            return DiscordApp::new(trimmed, name, name);
        }
    }

    let lower = name.to_ascii_lowercase();
    let stem = lower.trim_end_matches(".exe");

    if stem.contains("hearthstone") || stem == "wtcg" || stem == "hsb" {
        DiscordApp::new(APP_ID_HEARTHSTONE, "Hearthstone", "Hearthstone")
    } else if stem.contains("diablo iv") || stem == "diabloiv" || stem == "fen" {
        DiscordApp::new(APP_ID_DIABLO_IV, "Diablo IV", "Diablo IV")
    } else if stem.contains("overwatch") || stem == "pro" {
        DiscordApp::new(APP_ID_OVERWATCH_2, "Overwatch 2", "Overwatch 2")
    } else if stem.contains("sc2") || stem.contains("starcraft ii") || stem == "s2" {
        DiscordApp::new(APP_ID_STARCRAFT_2, "StarCraft II", "StarCraft II")
    } else if stem.contains("starcraft") || stem == "s1" {
        DiscordApp::new(
            APP_ID_STARCRAFT_2,
            "StarCraft: Remastered",
            "StarCraft: Remastered",
        )
    } else if stem == "w1r"
        || stem == "w1"
        || stem.contains("warcraft i:")
        || stem.contains("warcraft i ")
        || stem.contains("warcraft 1")
    {
        DiscordApp::new(
            APP_ID_WARCRAFT_1_REMASTERED,
            "Warcraft I: Remastered",
            "Warcraft I: Remastered",
        )
    } else if stem == "w2r"
        || stem == "w2"
        || stem.contains("warcraft ii:")
        || stem.contains("warcraft ii ")
        || stem.contains("warcraft 2")
    {
        DiscordApp::new(
            APP_ID_WARCRAFT_2_REMASTERED,
            "Warcraft II: Remastered",
            "Warcraft II: Remastered",
        )
    } else if stem == "w3" || stem == "w3r" || stem.contains("reforged") {
        DiscordApp::new(
            APP_ID_WARCRAFT_3_REFORGED,
            "Warcraft III: Reforged",
            "Warcraft III: Reforged",
        )
    } else if stem == "war3"
        || stem == "wc3"
        || stem.contains("reign of chaos")
        || stem.contains("frozen throne")
    {
        DiscordApp::new(
            APP_ID_WARCRAFT_3_CLASSIC,
            "Warcraft III: Reign of Chaos",
            "Warcraft III: Reign of Chaos",
        )
    } else if stem == "wowf" || stem.contains("forever") {
        app_for_product(&ProductCode::Forever, None)
    } else if stem.contains("ptr") || stem == "wowt" || stem == "wowxptr" {
        app_for_product(&ProductCode::Ptr, None)
    } else if stem.contains("classic")
        || stem == "wowclassic"
        || stem == "wow_classic"
        || stem == "wow_classic_era"
        || stem.contains("anniversary")
    {
        app_for_product(&ProductCode::Classic, None)
    } else if stem.starts_with("wow") {
        app_for_product(&ProductCode::Retail, None)
    } else {
        DiscordApp::new(APP_ID_WOW, "World of Warcraft", "Playing")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn official_products_map_to_dedicated_apps() {
        assert_eq!(app_for_product(&ProductCode::Retail, None).client_id, APP_ID_WOW);
        assert_eq!(app_for_product(&ProductCode::Retail, None).title, "World of Warcraft");

        assert_eq!(app_for_product(&ProductCode::Classic, None).client_id, APP_ID_WOW_CLASSIC);
        assert_eq!(app_for_product(&ProductCode::Classic, None).title, "World of Warcraft Classic");

        assert_eq!(app_for_product(&ProductCode::ClassicEra, None).client_id, APP_ID_WOW_CLASSIC);
        assert_eq!(app_for_product(&ProductCode::ClassicEra, None).title, "World of Warcraft Classic");

        assert_eq!(app_for_product(&ProductCode::Ptr, None).client_id, APP_ID_WOW_PTR);
        assert_eq!(app_for_product(&ProductCode::Ptr, None).title, "World of Warcraft Public Test");

        assert_eq!(app_for_product(&ProductCode::Forever, None).client_id, APP_ID_WOW_FOREVER);
        assert_eq!(app_for_product(&ProductCode::Forever, None).title, "World of Warcraft: Forever");
        assert_eq!(
            app_for_product(&ProductCode::Forever, None).default_details,
            "World of Warcraft: Forever"
        );
    }

    #[test]
    fn custom_app_id_overrides_builtin() {
        let app = app_for_product(&ProductCode::Forever, Some("123456789"));
        assert_eq!(app.client_id, "123456789");
        assert_eq!(app.title, "World of Warcraft: Forever");
    }

    #[test]
    fn hand_off_and_warcraft_games_resolve() {
        assert_eq!(app_for_name("Hearthstone.exe", None).client_id, APP_ID_HEARTHSTONE);
        assert_eq!(app_for_name("Diablo IV.exe", None).client_id, APP_ID_DIABLO_IV);
        assert_eq!(app_for_name("Overwatch.exe", None).client_id, APP_ID_OVERWATCH_2);
        assert_eq!(app_for_name("SC2_x64.exe", None).client_id, APP_ID_STARCRAFT_2);
        assert_eq!(app_for_name("w1r", None).client_id, APP_ID_WARCRAFT_1_REMASTERED);
        assert_eq!(app_for_name("w2r", None).client_id, APP_ID_WARCRAFT_2_REMASTERED);
        assert_eq!(app_for_name("w3", None).client_id, APP_ID_WARCRAFT_3_REFORGED);
        assert_eq!(app_for_name("war3.exe", None).client_id, APP_ID_WARCRAFT_3_CLASSIC);
        assert_eq!(app_for_name("wowclassic.exe", None).client_id, APP_ID_WOW_CLASSIC);
        assert_eq!(app_for_name("wowt.exe", None).client_id, APP_ID_WOW_PTR);
        assert_eq!(app_for_name("wowf", None).client_id, APP_ID_WOW_FOREVER);
    }
}
