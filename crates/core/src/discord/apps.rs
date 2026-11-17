//! Discord Application IDs and asset mappings for Blizzard game families.

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

    let mut app = resolve_builtin_app(code, &code_str, stem, clean_name);

    if let Some(id) = custom_id.map(str::trim).filter(|s| !s.is_empty()) {
        app.client_id = id.to_owned();
        if app.default_details == "Playing" {
            app.default_details = app.title.clone();
        }
    }

    app
}

/// Whether `code` / `name` refers to a World of Warcraft product (the only
/// family where `WTF` / `Client.log` character detection applies).
pub(crate) fn is_wow_app(app: &DiscordApp) -> bool {
    matches!(
        app.client_id.as_str(),
        APP_ID_WOW | APP_ID_WOW_CLASSIC | APP_ID_WOW_PTR | APP_ID_WOW_FOREVER
    )
}

fn matches_game(code_str: &str, stem: &str, exact: &[&str], substrings: &[&str]) -> bool {
    exact.iter().any(|&k| code_str == k || stem == k)
        || substrings
            .iter()
            .any(|&sub| code_str.contains(sub) || stem.contains(sub))
}

fn resolve_builtin_app(
    code: &ProductCode,
    code_str: &str,
    stem: &str,
    clean_name: &str,
) -> DiscordApp {
    // 1. Explicit WoW variants
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

    // 2. Match Blizzard game families by TACT product code or process stem.
    // Order matters where names overlap (e.g. StarCraft II and Remastered
    // before StarCraft; Warcraft I/II/III before World of Warcraft).
    if matches_game(
        code_str,
        stem,
        &["s2", "sc2"],
        &["sc2", "starcraft ii", "starcraft 2", "starcraft_2"],
    ) {
        return DiscordApp::new(APP_ID_STARCRAFT_2, "StarCraft II", "StarCraft II");
    }

    // s1 is the official Battle.net TACT code for StarCraft: Remastered.
    if matches_game(code_str, stem, &["s1", "scr"], &["remaster"]) {
        return DiscordApp::new(
            APP_ID_STARCRAFT_REMASTERED,
            "StarCraft: Remastered",
            "StarCraft: Remastered",
        );
    }

    if matches_game(
        code_str,
        stem,
        &["sc", "sc_x64", "starcraft", "starcraft_x64", "starcraft64"],
        &["starcraft"],
    ) {
        return DiscordApp::new(APP_ID_STARCRAFT, "StarCraft", "StarCraft");
    }

    if matches_game(
        code_str,
        stem,
        &["hsb", "hs_beta", "wtcg"],
        &["hearthstone"],
    ) {
        return DiscordApp::new(APP_ID_HEARTHSTONE, "Hearthstone", "Hearthstone");
    }

    if matches_game(
        code_str,
        stem,
        &["fenris", "fen", "d4"],
        &["diablo iv", "diablo 4", "diabloiv"],
    ) {
        return DiscordApp::new(APP_ID_DIABLO_IV, "Diablo IV", "Diablo IV");
    }

    if matches_game(
        code_str,
        stem,
        &["d3"],
        &["diablo iii", "diablo 3", "diabloiii"],
    ) {
        return DiscordApp::new(APP_ID_DIABLO_3, "Diablo III", "Diablo III");
    }

    if matches_game(
        code_str,
        stem,
        &["osi", "d2r"],
        &["d2r", "resurrected", "diablo ii", "diablo 2"],
    ) {
        return DiscordApp::new(
            APP_ID_DIABLO_2_RESURRECTED,
            "Diablo II: Resurrected",
            "Diablo II: Resurrected",
        );
    }

    if matches_game(code_str, stem, &["d2"], &[]) {
        return DiscordApp::new(APP_ID_DIABLO_2, "Diablo II", "Diablo II");
    }

    if matches_game(
        code_str,
        stem,
        &["anbs"],
        &["diablo immortal", "diabloimmortal"],
    ) {
        return DiscordApp::new(APP_ID_DIABLO_IMMORTAL, "Diablo Immortal", "Diablo Immortal");
    }

    if matches_game(code_str, stem, &["d1", "diablo"], &[]) {
        return DiscordApp::new(APP_ID_DIABLO_1, "Diablo", "Diablo");
    }

    if matches_game(code_str, stem, &["pro"], &["overwatch"]) {
        return DiscordApp::new(APP_ID_OVERWATCH_2, "Overwatch 2", "Overwatch 2");
    }

    if matches_game(
        code_str,
        stem,
        &["hero", "hots"],
        &["heroes", "heroesofthestorm"],
    ) {
        return DiscordApp::new(
            APP_ID_HEROES_OF_THE_STORM,
            "Heroes of the Storm",
            "Heroes of the Storm",
        );
    }

    if matches_game(
        code_str,
        stem,
        &["w1r", "w1", "warcraft", "warcraft i"],
        &["warcraft i:", "warcraft i ", "warcraft 1"],
    ) {
        return DiscordApp::new(
            APP_ID_WARCRAFT_1_REMASTERED,
            "Warcraft I: Remastered",
            "Warcraft I: Remastered",
        );
    }

    if matches_game(
        code_str,
        stem,
        &["w2r", "w2", "warcraft ii"],
        &["warcraft ii:", "warcraft ii ", "warcraft 2"],
    ) {
        return DiscordApp::new(
            APP_ID_WARCRAFT_2_REMASTERED,
            "Warcraft II: Remastered",
            "Warcraft II: Remastered",
        );
    }

    if matches_game(
        code_str,
        stem,
        &["war3", "wc3"],
        &["reign of chaos", "frozen throne"],
    ) {
        return DiscordApp::new(
            APP_ID_WARCRAFT_3_CLASSIC,
            "Warcraft III: Reign of Chaos",
            "Warcraft III: Reign of Chaos",
        );
    }

    if matches_game(
        code_str,
        stem,
        &["w3", "w3r", "warcraft iii"],
        &["reforged", "warcraft iii", "warcraft 3"],
    ) {
        return DiscordApp::new(
            APP_ID_WARCRAFT_3_REFORGED,
            "Warcraft III: Reforged",
            "Warcraft III: Reforged",
        );
    }

    if matches_game(code_str, stem, &["wowf"], &["forever"]) {
        return DiscordApp::new(
            APP_ID_WOW_FOREVER,
            "World of Warcraft: Forever",
            "World of Warcraft: Forever",
        );
    }

    if matches_game(code_str, stem, &["wowt", "wowxptr"], &["ptr"]) {
        return DiscordApp::new(
            APP_ID_WOW_PTR,
            "World of Warcraft Public Test",
            "World of Warcraft Public Test",
        );
    }

    if matches_game(
        code_str,
        stem,
        &["wowclassic", "wow_classic", "wow_classic_era"],
        &["classic", "anniversary"],
    ) {
        return DiscordApp::new(
            APP_ID_WOW_CLASSIC,
            "World of Warcraft Classic",
            "World of Warcraft Classic",
        );
    }

    if code_str.starts_with("wow")
        || code_str.contains("wow")
        || stem.starts_with("wow")
        || stem.contains("wow")
        || code_str.contains("world of warcraft")
        || stem.contains("world of warcraft")
    {
        return DiscordApp::new(APP_ID_WOW, "World of Warcraft", "World of Warcraft");
    }

    // 3. Unrecognized fallback: use code or clean name
    let title = if !code.as_str().is_empty() && code.as_str() != "custom" {
        code.as_str()
    } else if !clean_name.is_empty() {
        clean_name
    } else {
        "World of Warcraft"
    };
    DiscordApp::new(APP_ID_WOW, title, "Playing")
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

        let app_d4 = app_for_product(&ProductCode::Custom("fenris".into()), Some("111222333"));
        assert_eq!(app_d4.client_id, "111222333");
        assert_eq!(app_d4.title, "Diablo IV");
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
        assert_eq!(app_for_name("fenris", None).client_id, APP_ID_DIABLO_IV);
        assert_eq!(app_for_name("Diablo IV.exe", None).client_id, APP_ID_DIABLO_IV);
        assert_eq!(app_for_name("Diablo III.exe", None).client_id, APP_ID_DIABLO_3);
        assert_eq!(app_for_name("D2R.exe", None).client_id, APP_ID_DIABLO_2_RESURRECTED);
        assert_eq!(app_for_name("Overwatch.exe", None).client_id, APP_ID_OVERWATCH_2);
        assert_eq!(app_for_name("Heroes.exe", None).client_id, APP_ID_HEROES_OF_THE_STORM);
        assert_eq!(
            resolve_game_app(&ProductCode::Custom("hero".into()), "HeroesSwitcher_x64.exe", None).client_id,
            APP_ID_HEROES_OF_THE_STORM
        );
        assert_eq!(app_for_name("w1r", None).client_id, APP_ID_WARCRAFT_1_REMASTERED);
        assert_eq!(app_for_name("Warcraft.exe", None).client_id, APP_ID_WARCRAFT_1_REMASTERED);
        assert_eq!(app_for_name("w2r", None).client_id, APP_ID_WARCRAFT_2_REMASTERED);
        assert_eq!(app_for_name("Warcraft II.exe", None).client_id, APP_ID_WARCRAFT_2_REMASTERED);
        assert_eq!(app_for_name("w3", None).client_id, APP_ID_WARCRAFT_3_REFORGED);
        assert_eq!(app_for_name("Warcraft III.exe", None).client_id, APP_ID_WARCRAFT_3_REFORGED);
        assert_eq!(app_for_name("war3.exe", None).client_id, APP_ID_WARCRAFT_3_CLASSIC);
        assert_eq!(app_for_name("wowclassic.exe", None).client_id, APP_ID_WOW_CLASSIC);
        assert_eq!(app_for_name("wow_anniversary", None).client_id, APP_ID_WOW_CLASSIC);
        assert_eq!(app_for_name("wowt.exe", None).client_id, APP_ID_WOW_PTR);
        assert_eq!(app_for_name("wowf", None).client_id, APP_ID_WOW_FOREVER);
    }
}
