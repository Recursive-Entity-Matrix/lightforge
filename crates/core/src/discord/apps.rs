//! Discord Application IDs for Blizzard game families.

use crate::product::ProductCode;

/// Discord Rich Presence identification for a game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscordApp {
    /// Discord Application Client ID (snowflake).
    pub client_id: String,
    /// Game title shown in presence.
    pub title: String,
    /// Default details string (e.g. branch name).
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
pub const APP_ID_DIABLO_1: &str = "496571953147150354";
pub const APP_ID_DIABLO_2: &str = "496572658255790080";
pub const APP_ID_DIABLO_2_RESURRECTED: &str = "1471657242729255086";
pub const APP_ID_DIABLO_3: &str = "358425751197974528";
pub const APP_ID_DIABLO_IV: &str = "1113966530531704943";
pub const APP_ID_DIABLO_IMMORTAL: &str = "1124353329640783962";
pub const APP_ID_OVERWATCH_2: &str = "356875221078245376";
pub const APP_ID_HEROES_OF_THE_STORM: &str = "356878860190613504";
pub const APP_ID_STARCRAFT: &str = "363409793119158272";
pub const APP_ID_STARCRAFT_REMASTERED: &str = "1496717666659078205";
pub const APP_ID_STARCRAFT_2: &str = "358425800766128128";

/// TACT product codes and Battle.net install UIDs mapped to Discord App IDs.
const GAME_APPS: &[(&[&str], &str, &str)] = &[
    (&["s2", "sc2"], APP_ID_STARCRAFT_2, "StarCraft II"),
    (&["s1", "scr"], APP_ID_STARCRAFT_REMASTERED, "StarCraft: Remastered"),
    (&["sc"], APP_ID_STARCRAFT, "StarCraft"),
    (&["hsb", "hs_beta", "wtcg"], APP_ID_HEARTHSTONE, "Hearthstone"),
    (&["fenris", "fen", "d4"], APP_ID_DIABLO_IV, "Diablo IV"),
    (&["d3"], APP_ID_DIABLO_3, "Diablo III"),
    (&["osi", "d2r"], APP_ID_DIABLO_2_RESURRECTED, "Diablo II: Resurrected"),
    (&["d2"], APP_ID_DIABLO_2, "Diablo II"),
    (&["anbs"], APP_ID_DIABLO_IMMORTAL, "Diablo Immortal"),
    (&["d1"], APP_ID_DIABLO_1, "Diablo"),
    (&["pro"], APP_ID_OVERWATCH_2, "Overwatch 2"),
    (&["hero", "hots"], APP_ID_HEROES_OF_THE_STORM, "Heroes of the Storm"),
    (&["w1r", "w1"], APP_ID_WARCRAFT_1_REMASTERED, "Warcraft I: Remastered"),
    (&["w2r", "w2"], APP_ID_WARCRAFT_2_REMASTERED, "Warcraft II: Remastered"),
    (&["w3", "w3r"], APP_ID_WARCRAFT_3_REFORGED, "Warcraft III: Reforged"),
    (&["war3", "wc3"], APP_ID_WARCRAFT_3_CLASSIC, "Warcraft III: Reign of Chaos"),
];

/// Resolve the Discord application info for a given [`ProductCode`].
pub fn app_for_product(code: &ProductCode) -> DiscordApp {
    resolve_game_app(code, code.as_str())
}

/// Resolve the Discord application info from a [`ProductCode`] and fallback executable name.
pub fn resolve_game_app(code: &ProductCode, exe_name: &str) -> DiscordApp {
    match code {
        ProductCode::Retail => {
            return DiscordApp::new(APP_ID_WOW, "World of Warcraft", "World of Warcraft");
        }
        ProductCode::Beta => {
            return DiscordApp::new(APP_ID_WOW, "World of Warcraft", "World of Warcraft: Beta");
        }
        ProductCode::Ptr | ProductCode::Ptr2 => {
            return DiscordApp::new(
                APP_ID_WOW_PTR,
                "World of Warcraft Public Test",
                "World of Warcraft Public Test",
            );
        }
        ProductCode::Classic => {
            return DiscordApp::new(
                APP_ID_WOW_CLASSIC,
                "World of Warcraft Classic",
                "World of Warcraft Classic",
            );
        }
        ProductCode::ClassicPtr => {
            return DiscordApp::new(
                APP_ID_WOW_CLASSIC,
                "World of Warcraft Classic",
                "World of Warcraft Classic: PTR",
            );
        }
        ProductCode::ClassicBeta => {
            return DiscordApp::new(
                APP_ID_WOW_CLASSIC,
                "World of Warcraft Classic",
                "World of Warcraft Classic: Beta",
            );
        }
        ProductCode::ClassicEra => {
            return DiscordApp::new(
                APP_ID_WOW_CLASSIC,
                "World of Warcraft Classic",
                "World of Warcraft Classic: Era",
            );
        }
        ProductCode::EraPtr => {
            return DiscordApp::new(
                APP_ID_WOW_CLASSIC,
                "World of Warcraft Classic",
                "World of Warcraft Classic: Era PTR",
            );
        }
        ProductCode::Forever => {
            return DiscordApp::new(
                APP_ID_WOW_FOREVER,
                "World of Warcraft: Forever",
                "World of Warcraft: Forever",
            );
        }
        ProductCode::Custom(_) => {}
    }

    let code_str = code.as_str().trim().to_ascii_lowercase();

    for &(codes, client_id, title) in GAME_APPS {
        if codes.contains(&code_str.as_str()) {
            return DiscordApp::new(client_id, title, title);
        }
    }

    // Custom / manifest-supplied WoW product codes (e.g. wow_anniversary, wow_midnight_ptr, turtle_wow).
    if code_str.starts_with("wow") || code_str.ends_with("_wow") {
        if code_str.contains("ptr") {
            return DiscordApp::new(
                APP_ID_WOW_PTR,
                "World of Warcraft Public Test",
                "World of Warcraft Public Test",
            );
        }
        if code_str.contains("classic") || code_str.contains("anniversary") {
            return DiscordApp::new(
                APP_ID_WOW_CLASSIC,
                "World of Warcraft Classic",
                "World of Warcraft Classic",
            );
        }
        return DiscordApp::new(APP_ID_WOW, "World of Warcraft", "World of Warcraft");
    }

    let clean_exe = exe_name.rsplit(['/', '\\']).next().unwrap_or(exe_name).trim();
    let title = if !code.as_str().is_empty() && code.as_str() != "custom" {
        code.as_str()
    } else if !clean_exe.is_empty() {
        clean_exe
    } else {
        "World of Warcraft"
    };
    DiscordApp::new(APP_ID_WOW, title, "Playing")
}

/// Whether `app` is a World of Warcraft application (where `WTF` character
/// detection applies).
pub(crate) fn is_wow_app(app: &DiscordApp) -> bool {
    matches!(
        app.client_id.as_str(),
        APP_ID_WOW | APP_ID_WOW_CLASSIC | APP_ID_WOW_PTR | APP_ID_WOW_FOREVER
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn official_products_map_to_dedicated_apps() {
        assert_eq!(app_for_product(&ProductCode::Retail).client_id, APP_ID_WOW);
        assert_eq!(app_for_product(&ProductCode::Retail).title, "World of Warcraft");

        assert_eq!(app_for_product(&ProductCode::Classic).client_id, APP_ID_WOW_CLASSIC);
        assert_eq!(app_for_product(&ProductCode::Classic).title, "World of Warcraft Classic");

        assert_eq!(app_for_product(&ProductCode::ClassicEra).client_id, APP_ID_WOW_CLASSIC);
        assert_eq!(app_for_product(&ProductCode::ClassicEra).title, "World of Warcraft Classic");

        assert_eq!(app_for_product(&ProductCode::Ptr).client_id, APP_ID_WOW_PTR);
        assert_eq!(app_for_product(&ProductCode::Ptr).title, "World of Warcraft Public Test");

        assert_eq!(app_for_product(&ProductCode::Forever).client_id, APP_ID_WOW_FOREVER);
        assert_eq!(app_for_product(&ProductCode::Forever).title, "World of Warcraft: Forever");
    }

    #[test]
    fn hand_off_tact_and_install_codes_resolve() {
        let cases = [
            ("s1", APP_ID_STARCRAFT_REMASTERED, "StarCraft: Remastered"),
            ("s2", APP_ID_STARCRAFT_2, "StarCraft II"),
            ("sc", APP_ID_STARCRAFT, "StarCraft"),
            ("hsb", APP_ID_HEARTHSTONE, "Hearthstone"),
            ("hs_beta", APP_ID_HEARTHSTONE, "Hearthstone"),
            ("fenris", APP_ID_DIABLO_IV, "Diablo IV"),
            ("d3", APP_ID_DIABLO_3, "Diablo III"),
            ("osi", APP_ID_DIABLO_2_RESURRECTED, "Diablo II: Resurrected"),
            ("anbs", APP_ID_DIABLO_IMMORTAL, "Diablo Immortal"),
            ("pro", APP_ID_OVERWATCH_2, "Overwatch 2"),
            ("hero", APP_ID_HEROES_OF_THE_STORM, "Heroes of the Storm"),
            ("w1r", APP_ID_WARCRAFT_1_REMASTERED, "Warcraft I: Remastered"),
            ("w2r", APP_ID_WARCRAFT_2_REMASTERED, "Warcraft II: Remastered"),
            ("w3", APP_ID_WARCRAFT_3_REFORGED, "Warcraft III: Reforged"),
            ("war3", APP_ID_WARCRAFT_3_CLASSIC, "Warcraft III: Reign of Chaos"),
            ("wow_anniversary", APP_ID_WOW_CLASSIC, "World of Warcraft Classic"),
            ("wow_midnight_ptr", APP_ID_WOW_PTR, "World of Warcraft Public Test"),
            ("turtle_wow", APP_ID_WOW, "World of Warcraft"),
        ];

        for (code, expected_id, expected_title) in cases {
            let app = resolve_game_app(&ProductCode::Custom(code.into()), "wine64-preloader");
            assert_eq!(app.client_id, expected_id, "code {code}");
            assert_eq!(app.title, expected_title, "code {code}");
        }
    }
}
