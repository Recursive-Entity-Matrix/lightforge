//! Non-invasive character and realm detection for World of Warcraft.
//!
//! Scans `Logs/Client.log`, `Logs/Connection.log`, `WTF/Config.wtf`, and the `WTF/Account`
//! folder without touching process memory, injecting code, or interfering with game files.

use std::fs;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Detected World of Warcraft character presence information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterInfo {
    /// Character name, formatted for display (hyphens replaced by spaces).
    /// `None` when the player is on the character selection screen.
    pub character: Option<String>,
    /// Realm name (e.g. "Silvermoon") or ruleset for realm-less editions like Forever (e.g. "PvE").
    pub realm: String,
    /// Whether the player is actively in the game world (vs Character Select / loading).
    pub in_world: bool,
}

impl CharacterInfo {
    /// Formatted status string suitable for Discord Rich Presence state or details.
    pub fn format_state(&self) -> String {
        if self.in_world {
            match &self.character {
                Some(char_name) => {
                    if self.realm.is_empty() {
                        char_name.clone()
                    } else {
                        format!("{char_name} - {}", self.realm)
                    }
                }
                None => {
                    if self.realm.is_empty() {
                        "In World".to_owned()
                    } else {
                        format!("In World - {}", self.realm)
                    }
                }
            }
        } else if self.realm.is_empty() {
            "Character Select".to_owned()
        } else {
            format!("Character Select - {}", self.realm)
        }
    }
}

/// Locate the `WTF` directory within `product_dir` or its immediate parent.
pub fn find_wtf_dir(product_dir: &Path) -> Option<PathBuf> {
    for candidate_name in &["WTF", "wtf", "Wtf"] {
        let p = product_dir.join(candidate_name);
        if p.is_dir() {
            return Some(p);
        }
    }
    if let Some(parent) = product_dir.parent() {
        for candidate_name in &["WTF", "wtf"] {
            let p = parent.join(candidate_name);
            if p.is_dir() {
                return Some(p);
            }
        }
    }
    None
}

/// Locate the `Logs` directory within `product_dir` or its immediate parent.
pub fn find_logs_dir(product_dir: &Path) -> Option<PathBuf> {
    for candidate_name in &["Logs", "logs", "LOGS"] {
        let p = product_dir.join(candidate_name);
        if p.is_dir() {
            return Some(p);
        }
    }
    if let Some(parent) = product_dir.parent() {
        for candidate_name in &["Logs", "logs"] {
            let p = parent.join(candidate_name);
            if p.is_dir() {
                return Some(p);
            }
        }
    }
    None
}

/// Formats a character folder name into an in-game display name (replacing hyphens with spaces).
pub fn format_character_name(raw: &str) -> String {
    raw.replace('-', " ")
}

/// Extract ruleset keyword (PvE, PvP, RP-PvP, Hardcore, Normal, RP) from a string.
pub fn extract_ruleset(text: &str) -> Option<&'static str> {
    let lower = text.to_ascii_lowercase();
    if lower.contains("rp-pvp") || lower.contains("rppvp") {
        Some("RP-PvP")
    } else if lower.contains("rp-pve") || lower.contains("rppve") {
        Some("RP-PvE")
    } else if lower.contains("hardcore") {
        Some("Hardcore")
    } else if lower.contains("pvp") {
        Some("PvP")
    } else if lower.contains("pve") {
        Some("PvE")
    } else if lower.contains("normal") {
        Some("Normal")
    } else if lower.contains("rp")
        && (lower.contains(" rp") || lower.contains("rp ") || lower.ends_with("rp"))
    {
        Some("RP")
    } else {
        None
    }
}

/// Read up to `max_bytes` from the tail of a file.
fn read_log_tail(path: &Path, max_bytes: u64) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let len = file.metadata()?.len();
    if len > max_bytes {
        file.seek(SeekFrom::Start(len - max_bytes))?;
    }
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivePlayerLog {
    pub in_world: bool,
    pub guid_hex: Option<String>,
    pub realm_id: Option<String>,
}

fn extract_player_guid(line: &str) -> Option<(String, String)> {
    let tag = "Player-";
    let start_idx = line.find(tag)? + tag.len();
    let rest = &line[start_idx..];

    let end_idx = rest
        .find(|c: char| c.is_whitespace() || c == ':' || c == '"' || c == ',')
        .unwrap_or(rest.len());
    let token = &rest[..end_idx];

    let parts: Vec<&str> = token.split('-').collect();
    if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
        Some((parts[0].to_string(), parts[1].to_string()))
    } else {
        None
    }
}

/// Parse the tail of `Logs/Client.log` to determine if the player is in-world and their active GUID.
/// Returns `None` if the log file is missing, empty, stale from a previous session,
/// or contains no definitive login/logout events.
pub fn parse_active_player_from_client_log(logs_dir: &Path) -> Option<ActivePlayerLog> {
    parse_active_player_from_client_log_with_start_time(logs_dir, None)
}

/// Parse the tail of `Logs/Client.log` taking into account the game start time.
pub fn parse_active_player_from_client_log_with_start_time(
    logs_dir: &Path,
    start_secs: Option<u64>,
) -> Option<ActivePlayerLog> {
    let path = if logs_dir.join("Client.log").is_file() {
        logs_dir.join("Client.log")
    } else if logs_dir.join("client.log").is_file() {
        logs_dir.join("client.log")
    } else {
        return None;
    };

    if let Some(s) = start_secs {
        if let Ok(metadata) = path.metadata() {
            if let Ok(mtime) = metadata.modified() {
                let start_time = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(s);
                if mtime < start_time {
                    return None;
                }
            }
        }
    }

    let content = read_log_tail(&path, 65536).ok()?;
    if content.trim().is_empty() {
        return None;
    }
    let lines: Vec<&str> = content.lines().collect();

    // Iterate backwards from the most recent log events
    for (i, line) in lines.iter().enumerate().rev() {
        // Logout or client shutdown events indicate the player is back at character select or exiting
        if line.contains("Client Destroy")
            || line.contains("Client Object Manager Destroyed")
            || line.contains("Stop waiting for active player")
            || line.contains("GameUI::Shutdown")
        {
            return Some(ActivePlayerLog {
                in_world: false,
                guid_hex: None,
                realm_id: None,
            });
        }

        // Active Player Created indicates the character has successfully loaded into the game world
        if line.contains("Active Player Created") {
            let start = i.saturating_sub(30);
            for prev_line in lines[start..=i].iter().rev() {
                if let Some((realm_id, guid_hex)) = extract_player_guid(prev_line) {
                    return Some(ActivePlayerLog {
                        in_world: true,
                        guid_hex: Some(guid_hex),
                        realm_id: Some(realm_id),
                    });
                }
            }

            return Some(ActivePlayerLog {
                in_world: true,
                guid_hex: None,
                realm_id: None,
            });
        }
    }

    None
}

/// Extract ruleset (PvE, PvP, etc.) from `Connection.log` or `General.log`.
pub fn parse_ruleset_from_logs(logs_dir: &Path) -> Option<String> {
    for log_name in &["Connection.log", "connection.log", "General.log", "general.log"] {
        let p = logs_dir.join(log_name);
        if !p.is_file() {
            continue;
        }
        if let Ok(content) = read_log_tail(&p, 65536) {
            for line in content.lines().rev() {
                if let Some(name_idx) = line.find("\"name\":\"") {
                    let rest = &line[name_idx + 8..];
                    if let Some(quote_end) = rest.find('"') {
                        let realm_full = &rest[..quote_end];
                        if let Some(ruleset) = extract_ruleset(realm_full) {
                            return Some(ruleset.to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

/// Resolve an active player's GUID to a character folder name.
fn resolve_guid_to_character(
    account_dir: &Path,
    guid_hex: &str,
    char_folders: &[String],
) -> Option<String> {
    if char_folders.is_empty() {
        return None;
    }
    if char_folders.len() == 1 {
        return Some(char_folders[0].clone());
    }

    // 1. Scan SavedVariables/*.lua for the GUID and match against folder names or prefixes
    let sv_dir = account_dir.join("SavedVariables");
    if sv_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&sv_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "lua") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        let lower_guid = guid_hex.to_ascii_lowercase();
                        if !content.to_ascii_lowercase().contains(&lower_guid) {
                            continue;
                        }
                        for line in content.lines() {
                            let line_lower = line.to_ascii_lowercase();
                            if !line_lower.contains(&lower_guid) {
                                continue;
                            }
                            for folder in char_folders {
                                let prefix = folder.split('-').next().unwrap_or(folder);
                                if line_lower.contains(&folder.to_ascii_lowercase())
                                    || line_lower.contains(&prefix.to_ascii_lowercase())
                                {
                                    return Some(folder.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Check character-list-order.txt (format: "<contentSetID> <realmID>-<guidHex> <orderIndex>")
    let order_file = account_dir.join("character-list-order.txt");
    if order_file.is_file() {
        if let Ok(content) = fs::read_to_string(&order_file) {
            let lower_guid = guid_hex.to_ascii_lowercase();
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.to_ascii_lowercase().contains(&lower_guid) {
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    if let Some(last) = parts.last() {
                        if let Ok(order_idx) = last.parse::<usize>() {
                            if order_idx >= 1 {
                                let mut sorted = char_folders.to_vec();
                                sorted.sort();
                                if let Some(matched) = sorted.get(order_idx - 1) {
                                    return Some(matched.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

/// Determine the display name for a realm (or ruleset for numeric folders).
fn resolve_realm_display(
    realm_folder: &str,
    cvar_realm: Option<&str>,
    logs_dir: Option<&Path>,
    account_dir: &Path,
) -> String {
    // If Config.wtf gave a non-numeric realm name, prefer it
    if let Some(cvar) = cvar_realm {
        if !cvar.chars().all(|c| c.is_ascii_digit()) {
            return cvar.to_string();
        }
    }

    // If the realm folder name is non-numeric (e.g. "Silvermoon", "Wild Growth"), use it
    let is_numeric = realm_folder.chars().all(|c| c.is_ascii_digit());
    if !is_numeric {
        return realm_folder.to_string();
    }

    // Numeric realm folder (e.g. "91" in WoW: Forever):
    // Parse the ruleset (PvE, PvP, RP-PvP, Hardcore, etc.) from logs
    if let Some(logs) = logs_dir {
        if let Some(ruleset) = parse_ruleset_from_logs(logs) {
            return ruleset;
        }
    }

    // Fall back to scanning SavedVariables for a ruleset name
    let sv_dir = account_dir.join("SavedVariables");
    if sv_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&sv_dir) {
            for entry in entries.flatten() {
                if entry.path().extension().is_some_and(|ext| ext == "lua") {
                    if let Ok(content) = fs::read_to_string(entry.path()) {
                        if let Some(ruleset) = extract_ruleset(&content) {
                            return ruleset.to_string();
                        }
                    }
                }
            }
        }
    }

    // Default for Forever / numeric realm where ruleset could not be parsed
    "PvE".to_string()
}

fn get_folder_latest_mtime(folder: &Path) -> SystemTime {
    let mut latest = folder
        .metadata()
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);

    if let Ok(entries) = fs::read_dir(folder) {
        for entry in entries.flatten() {
            if let Ok(m) = entry.metadata().and_then(|m| m.modified()) {
                if m > latest {
                    latest = m;
                }
            }
        }
    }

    latest
}

/// Detect character and realm from a WoW product directory.
pub fn detect_character(product_dir: &Path) -> Option<CharacterInfo> {
    detect_character_with_start_time(product_dir, None)
}

/// Detect character and realm from a WoW product directory, taking into account
/// the game's launch timestamp to avoid premature character display and properly
/// handle in-session character switching.
pub fn detect_character_with_start_time(
    product_dir: &Path,
    start_secs: Option<u64>,
) -> Option<CharacterInfo> {
    let wtf_dir = find_wtf_dir(product_dir)?;
    let logs_dir = find_logs_dir(product_dir);

    // Optional realm name from Config.wtf
    let cvar_realm = crate::cvars::get(&wtf_dir.join("Config.wtf"), "realmName")
        .or_else(|| crate::cvars::get(&wtf_dir.join("config.wtf"), "realmName"))
        .filter(|r| !r.trim().is_empty());

    // Locate the Account folder (case-insensitive check)
    let account_dir = if wtf_dir.join("Account").is_dir() {
        Some(wtf_dir.join("Account"))
    } else if wtf_dir.join("account").is_dir() {
        Some(wtf_dir.join("account"))
    } else {
        None
    };

    let active_log = logs_dir
        .as_deref()
        .and_then(|d| parse_active_player_from_client_log_with_start_time(d, start_secs));

    if let Some(acc_dir) = account_dir {
        if let Some(char_info) = scan_account_dir(
            &acc_dir,
            cvar_realm.as_deref(),
            logs_dir.as_deref(),
            active_log.as_ref(),
            start_secs,
        ) {
            return Some(char_info);
        }
    }

    // Fall back to Config.wtf realmName if no character was found in Account
    cvar_realm.map(|realm| {
        let is_numeric = realm.chars().all(|c| c.is_ascii_digit());
        let display_realm = if is_numeric {
            logs_dir
                .as_deref()
                .and_then(parse_ruleset_from_logs)
                .unwrap_or_else(|| "PvE".to_string())
        } else {
            realm
        };
        CharacterInfo {
            character: None,
            realm: display_realm,
            in_world: false,
        }
    })
}

fn scan_account_dir(
    account_dir: &Path,
    cvar_realm: Option<&str>,
    logs_dir: Option<&Path>,
    active_log: Option<&ActivePlayerLog>,
    start_secs: Option<u64>,
) -> Option<CharacterInfo> {
    let accounts = fs::read_dir(account_dir).ok()?;

    let mut best_account: Option<(PathBuf, SystemTime)> = None;

    for account_entry in accounts.flatten() {
        if !account_entry.file_type().is_ok_and(|ft| ft.is_dir()) {
            continue;
        }

        let acc_name = account_entry.file_name().to_string_lossy().into_owned();
        if acc_name.eq_ignore_ascii_case("SavedVariables") {
            continue;
        }

        let acc_mtime = account_entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);

        if best_account.as_ref().is_none_or(|(_, t)| acc_mtime > *t) {
            best_account = Some((account_entry.path(), acc_mtime));
        }
    }

    let (active_acc_path, _) = best_account?;

    let realm_entries = fs::read_dir(&active_acc_path).ok()?;
    let mut best_realm: Option<(PathBuf, String, SystemTime)> = None;

    for realm_entry in realm_entries.flatten() {
        if !realm_entry.file_type().is_ok_and(|ft| ft.is_dir()) {
            continue;
        }

        let realm_name = realm_entry.file_name().to_string_lossy().into_owned();
        if realm_name.eq_ignore_ascii_case("SavedVariables") {
            continue;
        }

        let realm_mtime = realm_entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);

        let matches_cvar = cvar_realm.is_some_and(|r| realm_name.eq_ignore_ascii_case(r));

        if best_realm.as_ref().is_none_or(|(_, r_name, t)| {
            (matches_cvar && !cvar_realm.is_some_and(|r| r_name.eq_ignore_ascii_case(r)))
                || realm_mtime > *t
        }) {
            best_realm = Some((realm_entry.path(), realm_name, realm_mtime));
        }
    }

    let (realm_path, realm_folder_name, _) = best_realm?;
    let display_realm = resolve_realm_display(
        &realm_folder_name,
        cvar_realm,
        logs_dir,
        &active_acc_path,
    );

    let mut char_folders = Vec::new();
    if let Ok(char_entries) = fs::read_dir(&realm_path) {
        for char_entry in char_entries.flatten() {
            if char_entry.file_type().is_ok_and(|ft| ft.is_dir()) {
                let name = char_entry.file_name().to_string_lossy().into_owned();
                if !name.eq_ignore_ascii_case("SavedVariables") {
                    char_folders.push(name);
                }
            }
        }
    }

    if char_folders.is_empty() {
        return Some(CharacterInfo {
            character: None,
            realm: display_realm,
            in_world: false,
        });
    }

    // 1. Initial launch phase check: during the first 20 seconds after game start,
    // the player is initializing and at the splash / character select screen.
    let now_secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let is_launch_phase = match start_secs {
        Some(s) if s > 0 => now_secs.saturating_sub(s) < 20,
        _ => false,
    };

    if is_launch_phase {
        return Some(CharacterInfo {
            character: None,
            realm: display_realm,
            in_world: false,
        });
    }

    // 2. Authoritative Client.log check: if unbuffered Client.log events are present, use them
    if let Some(log) = active_log {
        if !log.in_world {
            return Some(CharacterInfo {
                character: None,
                realm: display_realm,
                in_world: false,
            });
        }
        if let Some(guid) = log.guid_hex.as_deref() {
            if let Some(c) = resolve_guid_to_character(&active_acc_path, guid, &char_folders) {
                return Some(CharacterInfo {
                    character: Some(format_character_name(&c)),
                    realm: display_realm,
                    in_world: true,
                });
            }
        }
    }

    // 3. File-based session & relog tracking:
    // Check modification times across all character folders.
    let mut char_mtimes: Vec<(String, SystemTime)> = Vec::new();
    for folder in &char_folders {
        let mtime = get_folder_latest_mtime(&realm_path.join(folder));
        char_mtimes.push((folder.clone(), mtime));
    }
    char_mtimes.sort_by_key(|a| std::cmp::Reverse(a.1));

    let chosen_folder = match start_secs {
        Some(s) if s > 0 && char_folders.len() > 1 => {
            let start_time = SystemTime::UNIX_EPOCH
                .checked_add(std::time::Duration::from_secs(s))
                .unwrap_or(SystemTime::UNIX_EPOCH);

            // A logout during the session occurs when a character file was modified
            // after the game process launched, and at least 20s after launch.
            let session_logout = char_mtimes.iter().find(|(_, mtime)| {
                *mtime > start_time && mtime.duration_since(start_time).is_ok_and(|d| d.as_secs() > 20)
            });

            match session_logout {
                Some((logged_out_char, logout_time)) => {
                    let logout_elapsed = logout_time.elapsed().map(|d| d.as_secs()).unwrap_or(999);
                    if logout_elapsed < 15 {
                        return Some(CharacterInfo {
                            character: None,
                            realm: display_realm,
                            in_world: false,
                        });
                    }
                    let other = char_mtimes
                        .iter()
                        .find(|(f, _)| f != logged_out_char)
                        .map(|(f, _)| f);
                    other.unwrap_or(logged_out_char).clone()
                }
                None => char_mtimes
                    .first()
                    .map(|(f, _)| f.clone())
                    .unwrap_or_else(|| char_folders[0].clone()),
            }
        }
        _ => char_mtimes
            .first()
            .map(|(f, _)| f.clone())
            .unwrap_or_else(|| char_folders[0].clone()),
    };

    Some(CharacterInfo {
        character: Some(format_character_name(&chosen_folder)),
        realm: display_realm,
        in_world: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_realm_and_character_when_present() {
        let temp = tempfile::tempdir().unwrap();
        let wtf = temp.path().join("WTF");
        fs::create_dir_all(&wtf).unwrap();

        fs::write(wtf.join("Config.wtf"), "SET realmName \"Silvermoon\"\n").unwrap();

        let char_dir = wtf.join("Account/12345#1/Silvermoon/Arthas");
        fs::create_dir_all(&char_dir).unwrap();

        let info = detect_character(temp.path()).unwrap();
        assert_eq!(info.realm, "Silvermoon");
        assert_eq!(info.character.as_deref(), Some("Arthas"));
        assert!(info.in_world);
        assert_eq!(info.format_state(), "Arthas - Silvermoon");
    }

    #[test]
    fn detects_in_world_character_with_client_log() {
        let temp = tempfile::tempdir().unwrap();
        let wtf = temp.path().join("WTF");
        let logs = temp.path().join("Logs");
        fs::create_dir_all(&wtf).unwrap();
        fs::create_dir_all(&logs).unwrap();

        let acc_dir = wtf.join("Account/12345#1");
        let char_dir = acc_dir.join("Silvermoon/Arthas");
        let sv_dir = acc_dir.join("SavedVariables");
        fs::create_dir_all(&char_dir).unwrap();
        fs::create_dir_all(&sv_dir).unwrap();

        fs::write(
            sv_dir.join("MyAddon.lua"),
            "[\"Player-1234-01CD49A4\"] = { [\"name\"] = \"Arthas\" }\n",
        )
        .unwrap();

        let client_log = "11/11 23:47:27.789  Client Object Manager Initialized: 0\n\
                          11/11 23:47:33.305  Character Login SEND\n\
                          11/11 23:47:37.082  Player-1234-01CD49A4: ignoring outdated\n\
                          11/11 23:47:37.286  Active Player Created\n";
        fs::write(logs.join("Client.log"), client_log).unwrap();

        let info = detect_character(temp.path()).unwrap();
        assert_eq!(info.realm, "Silvermoon");
        assert_eq!(info.character.as_deref(), Some("Arthas"));
        assert!(info.in_world);
        assert_eq!(info.format_state(), "Arthas - Silvermoon");
    }

    #[test]
    fn detects_character_select_after_logout_in_client_log() {
        let temp = tempfile::tempdir().unwrap();
        let wtf = temp.path().join("WTF");
        let logs = temp.path().join("Logs");
        fs::create_dir_all(&wtf).unwrap();
        fs::create_dir_all(&logs).unwrap();

        let char_dir = wtf.join("Account/12345#1/Silvermoon/Arthas");
        fs::create_dir_all(&char_dir).unwrap();

        let client_log = "11/11 23:47:37.286  Active Player Created\n\
                          11/11 23:48:33.756  Client Object Manager Destroyed\n\
                          11/11 23:48:35.148  Client WowCS Destroyed\n";
        fs::write(logs.join("Client.log"), client_log).unwrap();

        let info = detect_character(temp.path()).unwrap();
        assert_eq!(info.realm, "Silvermoon");
        assert_eq!(info.character, None);
        assert!(!info.in_world);
        assert_eq!(info.format_state(), "Character Select - Silvermoon");
    }

    #[test]
    fn replaces_hyphen_with_space_in_character_names() {
        assert_eq!(format_character_name("Chompy-Tyataza"), "Chompy Tyataza");
        assert_eq!(format_character_name("Tyataza-Shu"), "Tyataza Shu");
        assert_eq!(format_character_name("Arthas"), "Arthas");
    }

    #[test]
    fn resolves_numeric_realm_folder_to_ruleset_from_connection_log() {
        let temp = tempfile::tempdir().unwrap();
        let wtf = temp.path().join("WTF");
        let logs = temp.path().join("Logs");
        fs::create_dir_all(&wtf).unwrap();
        fs::create_dir_all(&logs).unwrap();

        let acc_dir = wtf.join("Account/993649430#1");
        let char_dir = acc_dir.join("91/Chompy-Tyataza");
        let sv_dir = acc_dir.join("SavedVariables");
        fs::create_dir_all(&char_dir).unwrap();
        fs::create_dir_all(&sv_dir).unwrap();

        fs::write(
            sv_dir.join("NAGJourney.lua"),
            "[\"Chompy-CAMWOW US2 Realm 1 PvE\"] = \"Player-6796-01CD49A4\",\n",
        )
        .unwrap();

        fs::write(
            logs.join("Connection.log"),
            "11/11 23:47:06.963  realmEntry={\"name\":\"CAMWOW US2 Realm 1 PvE\"}\n",
        )
        .unwrap();

        fs::write(
            logs.join("Client.log"),
            "Player-6796-01CD49A4: enter\nActive Player Created\n",
        )
        .unwrap();

        let info = detect_character(temp.path()).unwrap();
        assert_eq!(info.realm, "PvE");
        assert_eq!(info.character.as_deref(), Some("Chompy Tyataza"));
        assert!(info.in_world);
        assert_eq!(info.format_state(), "Chompy Tyataza - PvE");
    }

    #[test]
    fn switches_active_character_on_relog() {
        let temp = tempfile::tempdir().unwrap();
        let wtf = temp.path().join("WTF");
        let logs = temp.path().join("Logs");
        fs::create_dir_all(&wtf).unwrap();
        fs::create_dir_all(&logs).unwrap();

        let acc_dir = wtf.join("Account/993649430#1");
        let char1_dir = acc_dir.join("91/Chompy-Tyataza");
        let char2_dir = acc_dir.join("91/Tyataza-Shu");
        let sv_dir = acc_dir.join("SavedVariables");
        fs::create_dir_all(&char1_dir).unwrap();
        fs::create_dir_all(&char2_dir).unwrap();
        fs::create_dir_all(&sv_dir).unwrap();

        fs::write(
            sv_dir.join("NAGJourney.lua"),
            "[\"Chompy-CAMWOW US2 Realm 1 PvE\"] = \"Player-6796-01CD49A4\",\n\
             [\"Tyataza-CAMWOW US2 Realm 1 PvE\"] = \"Player-6796-006BD968\",\n",
        )
        .unwrap();

        // 1. Initial login as Chompy
        let log1 = "Player-6796-01CD49A4: login\nActive Player Created\n";
        fs::write(logs.join("Client.log"), log1).unwrap();

        let info1 = detect_character(temp.path()).unwrap();
        assert_eq!(info1.character.as_deref(), Some("Chompy Tyataza"));
        assert!(info1.in_world);

        // 2. Logout to character select
        let log2 = "Player-6796-01CD49A4: login\nActive Player Created\nClient Object Manager Destroyed\n";
        fs::write(logs.join("Client.log"), log2).unwrap();

        let info2 = detect_character(temp.path()).unwrap();
        assert_eq!(info2.character, None);
        assert!(!info2.in_world);
        assert_eq!(info2.format_state(), "Character Select - PvE");

        // 3. Login as Tyataza
        let log3 = "Player-6796-01CD49A4: login\nActive Player Created\nClient Object Manager Destroyed\nPlayer-6796-006BD968: login\nActive Player Created\n";
        fs::write(logs.join("Client.log"), log3).unwrap();

        let info3 = detect_character(temp.path()).unwrap();
        assert_eq!(info3.character.as_deref(), Some("Tyataza Shu"));
        assert!(info3.in_world);
        assert_eq!(info3.format_state(), "Tyataza Shu - PvE");
    }

    #[test]
    fn detects_character_select_during_initial_launch_phase() {
        let temp = tempfile::tempdir().unwrap();
        let wtf = temp.path().join("WTF");
        fs::create_dir_all(&wtf).unwrap();

        let acc_dir = wtf.join("Account/993649430#1");
        let char_dir = acc_dir.join("91/Chompy-Tyataza");
        fs::create_dir_all(&char_dir).unwrap();

        // Launched 5 seconds ago (<20s launch phase)
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let info = detect_character_with_start_time(temp.path(), Some(now - 5)).unwrap();
        assert_eq!(info.character, None);
        assert!(!info.in_world);
        assert_eq!(info.realm, "PvE");
        assert_eq!(info.format_state(), "Character Select - PvE");

        // Launched 30 seconds ago (>=20s, in world)
        let info_later = detect_character_with_start_time(temp.path(), Some(now - 30)).unwrap();
        assert_eq!(info_later.character.as_deref(), Some("Chompy Tyataza"));
        assert!(info_later.in_world);
        assert_eq!(info_later.format_state(), "Chompy Tyataza - PvE");
    }

    #[test]
    fn handles_zero_byte_buffered_client_log_under_proton() {
        let temp = tempfile::tempdir().unwrap();
        let wtf = temp.path().join("WTF");
        let logs = temp.path().join("Logs");
        fs::create_dir_all(&wtf).unwrap();
        fs::create_dir_all(&logs).unwrap();

        // 0-byte Client.log as produced by block-buffered Wine/Proton execution
        fs::write(logs.join("Client.log"), b"").unwrap();

        let acc_dir = wtf.join("Account/993649430#1");
        let char_dir = acc_dir.join("91/Chompy-Tyataza");
        fs::create_dir_all(&char_dir).unwrap();

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // During launch phase: Character Select
        let info_early = detect_character_with_start_time(temp.path(), Some(now - 5)).unwrap();
        assert_eq!(info_early.character, None);
        assert!(!info_early.in_world);
        assert_eq!(info_early.format_state(), "Character Select - PvE");

        // After launch phase: Chompy Tyataza in world
        let info_later = detect_character_with_start_time(temp.path(), Some(now - 30)).unwrap();
        assert_eq!(info_later.character.as_deref(), Some("Chompy Tyataza"));
        assert!(info_later.in_world);
        assert_eq!(info_later.format_state(), "Chompy Tyataza - PvE");
    }

    #[test]
    fn resolves_character_from_character_list_order_when_no_saved_variables() {
        let temp = tempfile::tempdir().unwrap();
        let wtf = temp.path().join("WTF");
        let logs = temp.path().join("Logs");
        fs::create_dir_all(&wtf).unwrap();
        fs::create_dir_all(&logs).unwrap();

        let acc_dir = wtf.join("Account/993649430#1");
        let char1_dir = acc_dir.join("91/Chompy-Tyataza");
        let char2_dir = acc_dir.join("91/Tyataza-Shu");
        fs::create_dir_all(&char1_dir).unwrap();
        fs::create_dir_all(&char2_dir).unwrap();

        // character-list-order.txt has GUID 6796-006BD968 at position 2
        fs::write(
            acc_dir.join("character-list-order.txt"),
            "Version: 2\n137 6796-01CD49A4 1\n137 6796-006BD968 2\n",
        )
        .unwrap();

        let client_log = "Player-6796-006BD968: enter\nActive Player Created\n";
        fs::write(logs.join("Client.log"), client_log).unwrap();

        let info = detect_character(temp.path()).unwrap();
        assert_eq!(info.character.as_deref(), Some("Tyataza Shu"));
        assert!(info.in_world);
        assert_eq!(info.format_state(), "Tyataza Shu - PvE");
    }

    #[test]
    fn parses_pvp_ruleset_correctly() {
        assert_eq!(extract_ruleset("CAMWOW US1 Realm 1 PvP"), Some("PvP"));
        assert_eq!(extract_ruleset("CAMWOW US1 Realm 2 RP-PvP"), Some("RP-PvP"));
        assert_eq!(extract_ruleset("CAMWOW US1 Realm 3 RP-PvE"), Some("RP-PvE"));
        assert_eq!(extract_ruleset("CAMWOW Hardcore Season"), Some("Hardcore"));
        assert_eq!(extract_ruleset("Silvermoon"), None);
    }
}
