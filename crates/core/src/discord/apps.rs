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

/// Resolve the Discord application info for a given [`ProductCode`] and optional custom App ID.
pub fn app_for_product(code: &ProductCode, custom_id: Option<&str>) -> DiscordApp {
    resolve_game_app(code, code.as_str(), custom_id)
}

/// Resolve the Discord application info for an arbitrary game or process name and optional custom App ID.
pub fn app_for_name(name: &str, custom_id: Option<&str>) -> DiscordApp {
    resolve_game_app(&ProductCode::from(name.to_owned()), name, custom_id)
}

/// Primary resolution function taking both [`ProductCode`] and the process/executable name.
pub fn resolve_game_app(code: &ProductCode, name: &str, custom_id: Option<&str>) -> DiscordApp {
    let clean_name = name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(name)
        .trim();
    let lower_name = clean_name.to_ascii_lowercase();
    let stem = lower_name.trim_end_matches(".exe");
    let code_str = code.as_str().trim().to_ascii_lowercase();

    // 1. User-specified custom app ID override
    if let Some(id) = custom_id {
        let trimmed = id.trim();
        if !trimmed.is_empty() {
            let label = title_for_code_or_stem(code, &code_str, stem, clean_name);
            return DiscordApp::new(trimmed, label.clone(), label);
        }
    }

    // 2. Explicit WoW variants
    match code {
        ProductCode::Retail => {
            return DiscordApp::new(
                APP_ID_WOW,
                "World of Warcraft",
                "World of Warcraft: The War Within",
            );
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

    // 3. Match game family by either product code or process stem

    // StarCraft II
    if code_str == "s2"
        || code_str == "sc2"
        || code_str.contains("starcraft ii")
        || code_str.contains("starcraft 2")
        || code_str.contains("starcraft_2")
        || stem == "s2"
        || stem == "sc2"
        || stem.contains("sc2")
        || stem.contains("starcraft ii")
        || stem.contains("starcraft 2")
    {
        return DiscordApp::new(APP_ID_STARCRAFT_2, "StarCraft II", "StarCraft II");
    }

    // StarCraft: Remastered (s1 is the official Battle.net TACT code for Remastered)
    if code_str == "s1"
        || code_str == "scr"
        || code_str.contains("remaster")
        || stem == "s1"
        || stem.contains("remaster")
    {
        return DiscordApp::new(
            APP_ID_STARCRAFT_REMASTERED,
            "StarCraft: Remastered",
            "StarCraft: Remastered",
        );
    }

    // StarCraft (Classic / original, e.g. sc.exe, starcraft.exe)
    if code_str == "sc"
        || code_str == "starcraft"
        || stem == "sc"
        || stem == "sc_x64"
        || stem == "starcraft"
        || stem == "starcraft_x64"
        || stem == "starcraft64"
        || stem.contains("starcraft")
    {
        return DiscordApp::new(APP_ID_STARCRAFT, "StarCraft", "StarCraft");
    }

    // Hearthstone
    if code_str == "hsb"
        || code_str == "hs_beta"
        || code_str == "wtcg"
        || code_str.contains("hearthstone")
        || stem == "hsb"
        || stem == "wtcg"
        || stem.contains("hearthstone")
    {
        return DiscordApp::new(APP_ID_HEARTHSTONE, "Hearthstone", "Hearthstone");
    }

    // Diablo IV
    if code_str == "fen"
        || code_str == "d4"
        || code_str.contains("diablo iv")
        || code_str.contains("diablo 4")
        || code_str.contains("diabloiv")
        || stem == "fen"
        || stem.contains("diablo iv")
        || stem.contains("diablo 4")
        || stem.contains("diabloiv")
    {
        return DiscordApp::new(APP_ID_DIABLO_IV, "Diablo IV", "Diablo IV");
    }

    // Diablo III
    if code_str == "d3"
        || code_str.contains("diablo iii")
        || code_str.contains("diablo 3")
        || code_str.contains("diabloiii")
        || stem == "d3"
        || stem.contains("diablo iii")
        || stem.contains("diablo 3")
        || stem.contains("diabloiii")
    {
        return DiscordApp::new(APP_ID_DIABLO_3, "Diablo III", "Diablo III");
    }

    // Diablo II: Resurrected
    if code_str == "osi"
        || code_str == "d2r"
        || code_str.contains("diablo ii")
        || code_str.contains("diablo 2")
        || code_str.contains("resurrected")
        || stem == "osi"
        || stem == "d2r"
        || stem.contains("d2r")
        || stem.contains("diablo ii")
    {
        return DiscordApp::new(
            APP_ID_DIABLO_2_RESURRECTED,
            "Diablo II: Resurrected",
            "Diablo II: Resurrected",
        );
    }

    // Diablo Immortal
    if code_str == "anbs"
        || code_str.contains("diablo immortal")
        || code_str.contains("diabloimmortal")
        || stem.contains("diablo immortal")
        || stem.contains("diabloimmortal")
    {
        return DiscordApp::new(APP_ID_DIABLO_IMMORTAL, "Diablo Immortal", "Diablo Immortal");
    }

    // Overwatch 2
    if code_str == "pro"
        || code_str.contains("overwatch")
        || stem == "pro"
        || stem.contains("overwatch")
    {
        return DiscordApp::new(APP_ID_OVERWATCH_2, "Overwatch 2", "Overwatch 2");
    }

    // Heroes of the Storm
    if code_str == "hero"
        || code_str.contains("heroes")
        || code_str == "hots"
        || stem == "hero"
        || stem.contains("heroes of the storm")
        || stem.contains("heroesofthestorm")
    {
        return DiscordApp::new(
            APP_ID_HEROES_OF_THE_STORM,
            "Heroes of the Storm",
            "Heroes of the Storm",
        );
    }

    // Warcraft I: Remastered
    if code_str == "w1r"
        || code_str == "w1"
        || code_str.contains("warcraft i:")
        || code_str.contains("warcraft i ")
        || code_str.contains("warcraft 1")
        || stem == "w1r"
        || stem == "w1"
        || stem.contains("warcraft i:")
        || stem.contains("warcraft i ")
        || stem.contains("warcraft 1")
    {
        return DiscordApp::new(
            APP_ID_WARCRAFT_1_REMASTERED,
            "Warcraft I: Remastered",
            "Warcraft I: Remastered",
        );
    }

    // Warcraft II: Remastered
    if code_str == "w2r"
        || code_str == "w2"
        || code_str.contains("warcraft ii:")
        || code_str.contains("warcraft ii ")
        || code_str.contains("warcraft 2")
        || stem == "w2r"
        || stem == "w2"
        || stem.contains("warcraft ii:")
        || stem.contains("warcraft ii ")
        || stem.contains("warcraft 2")
    {
        return DiscordApp::new(
            APP_ID_WARCRAFT_2_REMASTERED,
            "Warcraft II: Remastered",
            "Warcraft II: Remastered",
        );
    }

    // Warcraft III: Reforged
    if code_str == "w3"
        || code_str == "w3r"
        || code_str.contains("reforged")
        || stem == "w3"
        || stem == "w3r"
        || stem.contains("reforged")
    {
        return DiscordApp::new(
            APP_ID_WARCRAFT_3_REFORGED,
            "Warcraft III: Reforged",
            "Warcraft III: Reforged",
        );
    }

    // Warcraft III: Classic
    if code_str == "war3"
        || code_str == "wc3"
        || code_str.contains("reign of chaos")
        || code_str.contains("frozen throne")
        || stem == "war3"
        || stem == "wc3"
        || stem.contains("reign of chaos")
        || stem.contains("frozen throne")
    {
        return DiscordApp::new(
            APP_ID_WARCRAFT_3_CLASSIC,
            "Warcraft III: Reign of Chaos",
            "Warcraft III: Reign of Chaos",
        );
    }

    // WoW Forever
    if code_str == "wowf"
        || code_str.contains("forever")
        || stem == "wowf"
        || stem.contains("forever")
    {
        return DiscordApp::new(
            APP_ID_WOW_FOREVER,
            "World of Warcraft: Forever",
            "World of Warcraft: Forever",
        );
    }

    // WoW PTR
    if code_str.contains("ptr")
        || code_str == "wowt"
        || code_str == "wowxptr"
        || stem.contains("ptr")
        || stem == "wowt"
        || stem == "wowxptr"
    {
        return DiscordApp::new(
            APP_ID_WOW_PTR,
            "World of Warcraft Public Test",
            "World of Warcraft Public Test",
        );
    }

    // WoW Classic / Era / Anniversary
    if code_str.contains("classic")
        || code_str == "wowclassic"
        || code_str.contains("anniversary")
        || stem.contains("classic")
        || stem == "wowclassic"
        || stem == "wow_classic"
        || stem == "wow_classic_era"
        || stem.contains("anniversary")
    {
        return DiscordApp::new(
            APP_ID_WOW_CLASSIC,
            "World of Warcraft Classic",
            "World of Warcraft Classic",
        );
    }

    // WoW Retail / Other WoW (e.g. turtle_wow)
    if code_str.starts_with("wow")
        || code_str.contains("wow")
        || stem.starts_with("wow")
        || stem.contains("wow")
        || stem.contains("warcraft")
    {
        return DiscordApp::new(
            APP_ID_WOW,
            "World of Warcraft",
            "World of Warcraft",
        );
    }

    // Unrecognized fallback: use code or clean name
    let title = if !code.as_str().is_empty() && code.as_str() != "custom" {
        code.as_str()
    } else if !clean_name.is_empty() {
        clean_name
    } else {
        "World of Warcraft"
    };
    DiscordApp::new(APP_ID_WOW, title, "Playing")
}

fn title_for_code_or_stem(
    code: &ProductCode,
    code_str: &str,
    stem: &str,
    clean_name: &str,
) -> String {
    match code {
        ProductCode::Forever => "World of Warcraft: Forever".to_owned(),
        ProductCode::Classic | ProductCode::ClassicEra => "World of Warcraft Classic".to_owned(),
        ProductCode::Retail => "World of Warcraft".to_owned(),
        ProductCode::Ptr | ProductCode::Ptr2 => "World of Warcraft Public Test".to_owned(),
        _ => {
            if code_str == "s2" || stem.contains("sc2") || stem.contains("starcraft ii") {
                "StarCraft II".to_owned()
            } else if code_str == "s1" || stem.contains("remaster") {
                "StarCraft: Remastered".to_owned()
            } else if code_str == "sc" || stem == "sc" || stem.contains("starcraft") {
                "StarCraft".to_owned()
            } else if code_str == "hsb" || stem.contains("hearthstone") {
                "Hearthstone".to_owned()
            } else if code_str == "fen" || stem.contains("diablo iv") {
                "Diablo IV".to_owned()
            } else if !code.as_str().is_empty() && code.as_str() != "custom" {
                code.as_str().to_owned()
            } else {
                clean_name.to_owned()
            }
        }
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

        let app_sc = resolve_game_app(
            &ProductCode::Custom("s1".into()),
            "StarCraft.exe",
            Some("987654321"),
        );
        assert_eq!(app_sc.client_id, "987654321");
        assert_eq!(app_sc.title, "StarCraft: Remastered");
    }

    #[test]
    fn starcraft_resolves_correctly() {
        // StarCraft Remastered via TACT code s1
        let s1_app = resolve_game_app(&ProductCode::Custom("s1".into()), "StarCraft.exe", None);
        assert_eq!(s1_app.client_id, APP_ID_STARCRAFT_REMASTERED);
        assert_eq!(s1_app.title, "StarCraft: Remastered");

        // StarCraft Remastered via TACT code s1 with generic/unrecognized process name
        let s1_preloader = resolve_game_app(&ProductCode::Custom("s1".into()), "wine64-preloader", None);
        assert_eq!(s1_preloader.client_id, APP_ID_STARCRAFT_REMASTERED);
        assert_eq!(s1_preloader.title, "StarCraft: Remastered");

        // StarCraft Remastered via process name
        let scr_proc = app_for_name("StarCraft Remastered.exe", None);
        assert_eq!(scr_proc.client_id, APP_ID_STARCRAFT_REMASTERED);
        assert_eq!(scr_proc.title, "StarCraft: Remastered");

        // Classic StarCraft via SC.exe
        let sc_classic = app_for_name("SC.exe", None);
        assert_eq!(sc_classic.client_id, APP_ID_STARCRAFT);
        assert_eq!(sc_classic.title, "StarCraft");

        // Classic StarCraft via StarCraft.exe
        let sc_exe = app_for_name("StarCraft.exe", None);
        assert_eq!(sc_exe.client_id, APP_ID_STARCRAFT);
        assert_eq!(sc_exe.title, "StarCraft");

        // StarCraft II via TACT code s2
        let s2_app = resolve_game_app(&ProductCode::Custom("s2".into()), "SC2_x64.exe", None);
        assert_eq!(s2_app.client_id, APP_ID_STARCRAFT_2);
        assert_eq!(s2_app.title, "StarCraft II");

        // StarCraft II via process name
        let sc2_proc = app_for_name("SC2_x64.exe", None);
        assert_eq!(sc2_proc.client_id, APP_ID_STARCRAFT_2);
        assert_eq!(sc2_proc.title, "StarCraft II");
    }

    #[test]
    fn hand_off_and_warcraft_games_resolve() {
        assert_eq!(app_for_name("Hearthstone.exe", None).client_id, APP_ID_HEARTHSTONE);
        assert_eq!(app_for_name("Diablo IV.exe", None).client_id, APP_ID_DIABLO_IV);
        assert_eq!(app_for_name("Diablo III.exe", None).client_id, APP_ID_DIABLO_3);
        assert_eq!(app_for_name("D2R.exe", None).client_id, APP_ID_DIABLO_2_RESURRECTED);
        assert_eq!(app_for_name("Overwatch.exe", None).client_id, APP_ID_OVERWATCH_2);
        assert_eq!(app_for_name("Heroes.exe", None).client_id, APP_ID_HEROES_OF_THE_STORM);
        assert_eq!(app_for_name("w1r", None).client_id, APP_ID_WARCRAFT_1_REMASTERED);
        assert_eq!(app_for_name("w2r", None).client_id, APP_ID_WARCRAFT_2_REMASTERED);
        assert_eq!(app_for_name("w3", None).client_id, APP_ID_WARCRAFT_3_REFORGED);
        assert_eq!(app_for_name("war3.exe", None).client_id, APP_ID_WARCRAFT_3_CLASSIC);
        assert_eq!(app_for_name("wowclassic.exe", None).client_id, APP_ID_WOW_CLASSIC);
        assert_eq!(app_for_name("wowt.exe", None).client_id, APP_ID_WOW_PTR);
        assert_eq!(app_for_name("wowf", None).client_id, APP_ID_WOW_FOREVER);
    }
}
