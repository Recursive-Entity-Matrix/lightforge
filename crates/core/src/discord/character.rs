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
    /// Character class (e.g. "Hunter", "Druid"), if detected.
    pub class: Option<String>,
    /// Character level (e.g. 25), if detected from SavedVariables.
    pub level: Option<u32>,
    /// Realm name (e.g. "Silvermoon") or ruleset for realm-less editions like Forever (e.g. "PvE").
    pub realm: String,
    /// Whether the player is actively in the game world (vs Character Select / loading).
    pub in_world: bool,
}

impl CharacterInfo {
    /// Formatted details string for Discord Rich Presence line 1.
    pub fn format_details(&self) -> String {
        if self.in_world {
            match (&self.character, &self.class, self.level) {
                (Some(name), Some(cls), Some(lvl)) => format!("{name} - Lvl {lvl} {cls}"),
                (Some(name), Some(cls), None) => format!("{name} - {cls}"),
                (Some(name), None, Some(lvl)) => format!("{name} - Lvl {lvl}"),
                (Some(name), None, None) => name.clone(),
                (None, Some(cls), Some(lvl)) => format!("Lvl {lvl} {cls}"),
                (None, Some(cls), None) => cls.clone(),
                (None, None, _) => "In World".to_owned(),
            }
        } else if self.realm.is_empty() {
            "Character Select".to_owned()
        } else {
            format!("Character Select - {}", self.realm)
        }
    }

    /// Formatted state string for Discord Rich Presence line 2.
    pub fn format_state(&self) -> String {
        if self.in_world {
            if self.realm.is_empty() {
                "In World".to_owned()
            } else {
                format!("In World - {}", self.realm)
            }
        } else {
            "In Game".to_owned()
        }
    }
}

/// Cached mtime snapshots so steady-state polls only `stat` files instead of
/// re-reading `Client.log` or `SavedVariables/*.lua` every poll cycle.
#[derive(Debug, Default)]
pub struct CharacterCache {
    client_log_stamp: Option<(PathBuf, u64, SystemTime)>,
    client_log_active: Option<ActivePlayerLog>,
    ruleset_stamp: Option<(PathBuf, SystemTime)>,
    ruleset_value: Option<String>,
    guid_stamp: Option<(PathBuf, String, SystemTime)>,
    guid_char: Option<String>,
    details_stamp: Option<CharacterDetailsStamp>,
    details_class: Option<String>,
    details_level: Option<u32>,
}

#[derive(Debug, PartialEq, Eq)]
struct CharacterDetailsStamp {
    char_dir: PathBuf,
    config_cache_mtime: Option<SystemTime>,
    char_sv_mtime: Option<SystemTime>,
    acc_sv_mtime: Option<SystemTime>,
}

const MAX_LUA_SCAN_BYTES: u64 = 2 * 1024 * 1024;
const MAX_DIR_LUA_SCAN_BYTES: u64 = 16 * 1024 * 1024;
const CLIENT_LOG_TAIL_BYTES: u64 = 65_536;

fn find_subdir(product_dir: &Path, names: &[&str]) -> Option<PathBuf> {
    for name in names {
        let p = product_dir.join(name);
        if p.is_dir() {
            return Some(p);
        }
    }
    if let Some(parent) = product_dir.parent() {
        for name in names {
            let p = parent.join(name);
            if p.is_dir() {
                return Some(p);
            }
        }
    }
    None
}

/// Locate the `WTF` directory within `product_dir` or its immediate parent.
pub fn find_wtf_dir(product_dir: &Path) -> Option<PathBuf> {
    find_subdir(product_dir, &["WTF", "wtf", "Wtf"])
}

/// Locate the `Logs` directory within `product_dir` or its immediate parent.
pub fn find_logs_dir(product_dir: &Path) -> Option<PathBuf> {
    find_subdir(product_dir, &["Logs", "logs", "LOGS"])
}

fn contains_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    haystack
        .as_bytes()
        .windows(needle.len())
        .any(|w| w.eq_ignore_ascii_case(needle.as_bytes()))
}

/// Skip multi-megabyte addon databases (TSM, Details, ATT) during periodic polling.
fn read_small_lua(path: &Path) -> Option<String> {
    if path.extension().is_none_or(|ext| ext != "lua") {
        return None;
    }
    let len = fs::metadata(path).ok()?.len();
    if len > MAX_LUA_SCAN_BYTES {
        return None;
    }
    fs::read_to_string(path).ok()
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

fn is_logout_line(line: &str) -> bool {
    line.contains("Client Destroy")
        || line.contains("Client Object Manager Destroyed")
        || line.contains("Stop waiting for active player")
        || line.contains("GameUI::Shutdown")
}

fn find_client_log(logs_dir: &Path) -> Option<PathBuf> {
    if logs_dir.join("Client.log").is_file() {
        Some(logs_dir.join("Client.log"))
    } else if logs_dir.join("client.log").is_file() {
        Some(logs_dir.join("client.log"))
    } else {
        None
    }
}

fn read_fresh_client_log(logs_dir: &Path, start_secs: Option<u64>) -> Option<String> {
    let path = find_client_log(logs_dir)?;

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

    let content = read_log_tail(&path, CLIENT_LOG_TAIL_BYTES).ok()?;
    if content.trim().is_empty() {
        return None;
    }
    Some(content)
}

fn parse_active_player_cached(
    logs_dir: &Path,
    start_secs: Option<u64>,
    cache: &mut CharacterCache,
) -> Option<ActivePlayerLog> {
    let path = find_client_log(logs_dir)?;
    let meta = path.metadata().ok()?;
    let len = meta.len();
    let mtime = meta.modified().ok()?;

    if let Some(s) = start_secs {
        let start_time = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(s);
        if mtime < start_time {
            return None;
        }
    }

    let stamp = (path.clone(), len, mtime);
    if cache.client_log_stamp.as_ref() == Some(&stamp) {
        return cache.client_log_active.clone();
    }

    let active = if len == 0 {
        None
    } else {
        let content = read_log_tail(&path, CLIENT_LOG_TAIL_BYTES).ok()?;
        if content.trim().is_empty() {
            None
        } else {
            let lines: Vec<&str> = content.lines().collect();
            parse_active_player_from_lines(&lines)
        }
    };
    cache.client_log_stamp = Some(stamp);
    cache.client_log_active = active.clone();
    active
}

fn parse_active_player_from_lines(lines: &[&str]) -> Option<ActivePlayerLog> {
    // Iterate backwards from the most recent log events
    for (i, line) in lines.iter().enumerate().rev() {
        // Logout or client shutdown events indicate the player is back at character select or exiting
        if is_logout_line(line) {
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
    let content = read_fresh_client_log(logs_dir, start_secs)?;
    let lines: Vec<&str> = content.lines().collect();
    parse_active_player_from_lines(&lines)
}

fn parse_ruleset_from_logs_cached(
    logs_dir: &Path,
    cache: &mut CharacterCache,
) -> Option<String> {
    for log_name in &["Connection.log", "connection.log", "General.log", "general.log"] {
        let p = logs_dir.join(log_name);
        let Ok(meta) = p.metadata() else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        if let Ok(mtime) = meta.modified() {
            let stamp = (p.clone(), mtime);
            if cache.ruleset_stamp.as_ref() == Some(&stamp) {
                if cache.ruleset_value.is_some() {
                    return cache.ruleset_value.clone();
                }
                continue;
            }
            if let Ok(content) = read_log_tail(&p, 65536) {
                let found = parse_ruleset_from_content(&content);
                cache.ruleset_stamp = Some(stamp);
                cache.ruleset_value = found.clone();
                if found.is_some() {
                    return found;
                }
            }
        }
    }
    None
}

fn parse_ruleset_from_content(content: &str) -> Option<String> {
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
            if let Some(ruleset) = parse_ruleset_from_content(&content) {
                return Some(ruleset);
            }
        }
    }
    None
}

fn dir_mtime(dir: &Path) -> Option<SystemTime> {
    dir.metadata().and_then(|m| m.modified()).ok()
}

fn config_cache_mtime(char_dir: &Path) -> Option<SystemTime> {
    for name in &["config-cache.wtf", "config-cache.old"] {
        if let Ok(mtime) = char_dir.join(name).metadata().and_then(|m| m.modified()) {
            return Some(mtime);
        }
    }
    None
}

/// Resolve an active player's GUID to a character folder name.
fn resolve_guid_to_character_cached(
    account_dir: &Path,
    guid_hex: &str,
    char_folders: &[String],
    cache: &mut CharacterCache,
) -> Option<String> {
    if char_folders.is_empty() {
        return None;
    }
    if char_folders.len() == 1 {
        return Some(char_folders[0].clone());
    }

    let sv_dir = account_dir.join("SavedVariables");
    if sv_dir.is_dir() {
        if let Some(sv_mtime) = dir_mtime(&sv_dir) {
            let stamp = (sv_dir.clone(), guid_hex.to_owned(), sv_mtime);
            if cache.guid_stamp.as_ref() == Some(&stamp) {
                if cache.guid_char.is_some() {
                    return cache.guid_char.clone();
                }
            } else {
                let found = scan_sv_for_guid(&sv_dir, guid_hex, char_folders);
                cache.guid_stamp = Some(stamp);
                cache.guid_char = found.clone();
                if found.is_some() {
                    return found;
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

fn scan_sv_for_guid(sv_dir: &Path, guid_hex: &str, char_folders: &[String]) -> Option<String> {
    let entries = fs::read_dir(sv_dir).ok()?;
    let mut scanned_bytes = 0u64;
    for entry in entries.flatten() {
        if scanned_bytes >= MAX_DIR_LUA_SCAN_BYTES {
            break;
        }
        let path = entry.path();
        if let Some(content) = read_small_lua(&path) {
            scanned_bytes += content.len() as u64;
            if !contains_ignore_ascii_case(&content, guid_hex) {
                continue;
            }
            for line in content.lines() {
                if !contains_ignore_ascii_case(line, guid_hex) {
                    continue;
                }
                for folder in char_folders {
                    let prefix = folder.split('-').next().unwrap_or(folder);
                    if contains_ignore_ascii_case(line, folder)
                        || contains_ignore_ascii_case(line, prefix)
                    {
                        return Some(folder.clone());
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
    cache: &mut CharacterCache,
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
        if let Some(ruleset) = parse_ruleset_from_logs_cached(logs, cache) {
            return ruleset;
        }
    }

    // Fall back to scanning SavedVariables for a ruleset name
    let sv_dir = account_dir.join("SavedVariables");
    if sv_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&sv_dir) {
            let mut scanned_bytes = 0u64;
            for entry in entries.flatten() {
                if scanned_bytes >= MAX_DIR_LUA_SCAN_BYTES {
                    break;
                }
                if let Some(content) = read_small_lua(&entry.path()) {
                    scanned_bytes += content.len() as u64;
                    if let Some(ruleset) = extract_ruleset(&content) {
                        return ruleset.to_string();
                    }
                }
            }
        }
    }

    // Default for Forever / numeric realm where ruleset could not be parsed
    "PvE".to_string()
}

/// Resolve class ID (from CVar EJLootClass) to display name.
pub fn class_name_from_id(id: u32) -> Option<&'static str> {
    match id {
        1 => Some("Warrior"),
        2 => Some("Paladin"),
        3 => Some("Hunter"),
        4 => Some("Rogue"),
        5 => Some("Priest"),
        6 => Some("Death Knight"),
        7 => Some("Shaman"),
        8 => Some("Mage"),
        9 => Some("Warlock"),
        10 => Some("Monk"),
        11 => Some("Druid"),
        12 => Some("Demon Hunter"),
        13 => Some("Evoker"),
        _ => None,
    }
}

const CLASS_PATTERNS: &[(&str, &str, &str, &str, &str)] = &[
    ("Warrior", "\"warrior\"", "'warrior'", "classnum\"] = 1", "class_id\"] = 1"),
    ("Paladin", "\"paladin\"", "'paladin'", "classnum\"] = 2", "class_id\"] = 2"),
    ("Hunter", "\"hunter\"", "'hunter'", "classnum\"] = 3", "class_id\"] = 3"),
    ("Rogue", "\"rogue\"", "'rogue'", "classnum\"] = 4", "class_id\"] = 4"),
    ("Priest", "\"priest\"", "'priest'", "classnum\"] = 5", "class_id\"] = 5"),
    ("Death Knight", "\"death knight\"", "'death knight'", "classnum\"] = 6", "class_id\"] = 6"),
    ("Shaman", "\"shaman\"", "'shaman'", "classnum\"] = 7", "class_id\"] = 7"),
    ("Mage", "\"mage\"", "'mage'", "classnum\"] = 8", "class_id\"] = 8"),
    ("Warlock", "\"warlock\"", "'warlock'", "classnum\"] = 9", "class_id\"] = 9"),
    ("Monk", "\"monk\"", "'monk'", "classnum\"] = 10", "class_id\"] = 10"),
    ("Druid", "\"druid\"", "'druid'", "classnum\"] = 11", "class_id\"] = 11"),
    ("Demon Hunter", "\"demon hunter\"", "'demon hunter'", "classnum\"] = 12", "class_id\"] = 12"),
    ("Evoker", "\"evoker\"", "'evoker'", "classnum\"] = 13", "class_id\"] = 13"),
];

/// Detect character class from character's `config-cache.wtf` or `SavedVariables`.
pub fn parse_character_class(
    char_dir: &Path,
    account_dir: &Path,
    char_name: &str,
) -> Option<String> {
    // 1. Native config-cache.wtf: SET EJLootClass "<id>"
    for config_name in &["config-cache.wtf", "config-cache.old"] {
        let p = char_dir.join(config_name);
        if let Some(val) = crate::cvars::get(&p, "EJLootClass") {
            if let Ok(id) = val.parse::<u32>() {
                if let Some(name) = class_name_from_id(id) {
                    return Some(name.to_string());
                }
            }
        }
    }

    // 2. Scan SavedVariables for class name or class ID associated with character
    let raw_name = char_dir
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let name_prefix = raw_name.split('-').next().unwrap_or(char_name);
    let display_name = format_character_name(&raw_name);

    let search_dirs = [char_dir.join("SavedVariables"), account_dir.join("SavedVariables")];
    for dir in &search_dirs {
        if !dir.is_dir() {
            continue;
        }
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        let mut scanned_bytes = 0u64;
        for entry in entries.flatten() {
            if scanned_bytes >= MAX_DIR_LUA_SCAN_BYTES {
                break;
            }
            let Some(content) = read_small_lua(&entry.path()) else {
                continue;
            };
            scanned_bytes += content.len() as u64;
            if !contains_ignore_ascii_case(&content, &display_name)
                && !contains_ignore_ascii_case(&content, name_prefix)
            {
                continue;
            }
            let lines: Vec<&str> = content.lines().collect();
            for (i, line) in lines.iter().enumerate() {
                if contains_ignore_ascii_case(line, &display_name)
                    || contains_ignore_ascii_case(line, name_prefix)
                {
                    let window_end = (i + 50).min(lines.len());
                    for candidate in &lines[i..window_end] {
                        if !contains_ignore_ascii_case(candidate, "class") {
                            continue;
                        }
                        let lower = candidate.to_ascii_lowercase();
                        for &(cname, q1, q2, n1, n2) in CLASS_PATTERNS {
                            if lower.contains(q1)
                                || lower.contains(q2)
                                || lower.contains(n1)
                                || lower.contains(n2)
                            {
                                return Some(cname.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

fn extract_lua_level(trimmed_line: &str) -> Option<u32> {
    let lower = trimmed_line.to_ascii_lowercase();
    let after_key = lower
        .find("[\"level\"]")
        .map(|idx| &trimmed_line[idx + 9..])
        .or_else(|| lower.find("['level']").map(|idx| &trimmed_line[idx + 9..]))
        .or_else(|| {
            (lower.starts_with("level ") || lower.starts_with("level="))
                .then(|| &trimmed_line[5..])
        })?;
    let eq_idx = after_key.find('=')?;
    let after_eq = after_key[eq_idx + 1..].trim();
    let num_str: String = after_eq.chars().take_while(|c| c.is_ascii_digit()).collect();
    let lvl = num_str.parse::<u32>().ok()?;
    (1..=100).contains(&lvl).then_some(lvl)
}

/// Detect character level from SavedVariables.
pub fn parse_character_level(
    char_dir: &Path,
    account_dir: &Path,
    char_name: &str,
) -> Option<u32> {
    let raw_name = char_dir
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let name_prefix = raw_name.split('-').next().unwrap_or(char_name);
    let display_name = format_character_name(&raw_name);

    let search_dirs = [char_dir.join("SavedVariables"), account_dir.join("SavedVariables")];

    for dir in &search_dirs {
        if !dir.is_dir() {
            continue;
        }
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        let mut scanned_bytes = 0u64;
        for entry in entries.flatten() {
            if scanned_bytes >= MAX_DIR_LUA_SCAN_BYTES {
                break;
            }
            let Some(content) = read_small_lua(&entry.path()) else {
                continue;
            };
            scanned_bytes += content.len() as u64;
            if !contains_ignore_ascii_case(&content, &display_name)
                && !contains_ignore_ascii_case(&content, name_prefix)
            {
                continue;
            }

            let lines: Vec<&str> = content.lines().collect();
            for (i, line) in lines.iter().enumerate() {
                if contains_ignore_ascii_case(line, &display_name)
                    || contains_ignore_ascii_case(line, name_prefix)
                {
                    let window_end = (i + 50).min(lines.len());
                    for candidate_line in &lines[i..window_end] {
                        if let Some(lvl) = extract_lua_level(candidate_line.trim()) {
                            return Some(lvl);
                        }
                    }
                }
            }
        }
    }

    None
}

fn resolve_character_details_cached(
    char_dir: &Path,
    account_dir: &Path,
    char_name: &str,
    cache: &mut CharacterCache,
) -> (Option<String>, Option<u32>) {
    let char_sv = char_dir.join("SavedVariables");
    let acc_sv = account_dir.join("SavedVariables");
    let stamp = CharacterDetailsStamp {
        char_dir: char_dir.to_path_buf(),
        config_cache_mtime: config_cache_mtime(char_dir),
        char_sv_mtime: char_sv.is_dir().then(|| get_folder_latest_mtime(&char_sv)),
        acc_sv_mtime: acc_sv.is_dir().then(|| get_folder_latest_mtime(&acc_sv)),
    };
    if cache.details_stamp.as_ref() == Some(&stamp) {
        return (cache.details_class.clone(), cache.details_level);
    }

    let class = parse_character_class(char_dir, account_dir, char_name);
    let level = parse_character_level(char_dir, account_dir, char_name);
    cache.details_stamp = Some(stamp);
    cache.details_class = class.clone();
    cache.details_level = level;
    (class, level)
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
    let mut cache = CharacterCache::default();
    detect_character_cached(product_dir, start_secs, &mut cache)
}

/// Detect character and realm using `cache` so steady-state polls only `stat`
/// files when nothing on disk has changed.
pub fn detect_character_cached(
    product_dir: &Path,
    start_secs: Option<u64>,
    cache: &mut CharacterCache,
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
        .and_then(|d| parse_active_player_cached(d, start_secs, cache));

    if let Some(acc_dir) = account_dir {
        if let Some(char_info) = scan_account_dir(
            &acc_dir,
            cvar_realm.as_deref(),
            logs_dir.as_deref(),
            active_log.as_ref(),
            start_secs,
            cache,
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
                .and_then(|d| parse_ruleset_from_logs_cached(d, cache))
                .unwrap_or_else(|| "PvE".to_string())
        } else {
            realm
        };
        CharacterInfo {
            character: None,
            class: None,
            level: None,
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
    cache: &mut CharacterCache,
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
            let best_matches = cvar_realm.is_some_and(|r| r_name.eq_ignore_ascii_case(r));
            (matches_cvar && !best_matches) || (matches_cvar == best_matches && realm_mtime > *t)
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
        cache,
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
            class: None,
            level: None,
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
            class: None,
            level: None,
            realm: display_realm,
            in_world: false,
        });
    }

    // 2. Authoritative Client.log check: if unbuffered Client.log events are present, use them
    if let Some(log) = active_log {
        if !log.in_world {
            return Some(CharacterInfo {
                character: None,
                class: None,
                level: None,
                realm: display_realm,
                in_world: false,
            });
        }
        if let Some(guid) = log.guid_hex.as_deref() {
            if let Some(c) =
                resolve_guid_to_character_cached(&active_acc_path, guid, &char_folders, cache)
            {
                let char_dir = realm_path.join(&c);
                let (class, level) =
                    resolve_character_details_cached(&char_dir, &active_acc_path, &c, cache);
                return Some(CharacterInfo {
                    character: Some(format_character_name(&c)),
                    class,
                    level,
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
                            class: None,
                            level: None,
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

    let char_dir = realm_path.join(&chosen_folder);
    let (class, level) =
        resolve_character_details_cached(&char_dir, &active_acc_path, &chosen_folder, cache);

    Some(CharacterInfo {
        character: Some(format_character_name(&chosen_folder)),
        class,
        level,
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
        assert_eq!(info.format_details(), "Arthas");
        assert_eq!(info.format_state(), "In World - Silvermoon");
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
        assert_eq!(info.format_details(), "Arthas");
        assert_eq!(info.format_state(), "In World - Silvermoon");
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
        assert_eq!(info.format_details(), "Character Select - Silvermoon");
        assert_eq!(info.format_state(), "In Game");
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
        assert_eq!(info.format_details(), "Chompy Tyataza");
        assert_eq!(info.format_state(), "In World - PvE");
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
        assert_eq!(info2.format_details(), "Character Select - PvE");
        assert_eq!(info2.format_state(), "In Game");

        // 3. Login as Tyataza
        let log3 = "Player-6796-01CD49A4: login\nActive Player Created\nClient Object Manager Destroyed\nPlayer-6796-006BD968: login\nActive Player Created\n";
        fs::write(logs.join("Client.log"), log3).unwrap();

        let info3 = detect_character(temp.path()).unwrap();
        assert_eq!(info3.character.as_deref(), Some("Tyataza Shu"));
        assert!(info3.in_world);
        assert_eq!(info3.format_details(), "Tyataza Shu");
        assert_eq!(info3.format_state(), "In World - PvE");
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
        assert_eq!(info.format_details(), "Character Select - PvE");
        assert_eq!(info.format_state(), "In Game");

        // Launched 30 seconds ago (>=20s, in world)
        let info_later = detect_character_with_start_time(temp.path(), Some(now - 30)).unwrap();
        assert_eq!(info_later.character.as_deref(), Some("Chompy Tyataza"));
        assert!(info_later.in_world);
        assert_eq!(info_later.format_details(), "Chompy Tyataza");
        assert_eq!(info_later.format_state(), "In World - PvE");
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
        assert_eq!(info_early.format_details(), "Character Select - PvE");
        assert_eq!(info_early.format_state(), "In Game");

        // After launch phase: Chompy Tyataza in world
        let info_later = detect_character_with_start_time(temp.path(), Some(now - 30)).unwrap();
        assert_eq!(info_later.character.as_deref(), Some("Chompy Tyataza"));
        assert!(info_later.in_world);
        assert_eq!(info_later.format_details(), "Chompy Tyataza");
        assert_eq!(info_later.format_state(), "In World - PvE");
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
        assert_eq!(info.format_details(), "Tyataza Shu");
        assert_eq!(info.format_state(), "In World - PvE");
    }

    #[test]
    fn parses_pvp_ruleset_correctly() {
        assert_eq!(extract_ruleset("CAMWOW US1 Realm 1 PvP"), Some("PvP"));
        assert_eq!(extract_ruleset("CAMWOW US1 Realm 2 RP-PvP"), Some("RP-PvP"));
        assert_eq!(extract_ruleset("CAMWOW US1 Realm 3 RP-PvE"), Some("RP-PvE"));
        assert_eq!(extract_ruleset("CAMWOW Hardcore Season"), Some("Hardcore"));
        assert_eq!(extract_ruleset("Silvermoon"), None);
    }

    #[test]
    fn detects_class_from_config_cache() {
        let temp = tempfile::tempdir().unwrap();
        let char_dir = temp.path().join("Account/12345#1/91/Chompy-Tyataza");
        fs::create_dir_all(&char_dir).unwrap();
        fs::write(char_dir.join("config-cache.wtf"), "SET EJLootClass \"3\"\n").unwrap();

        let class = parse_character_class(&char_dir, temp.path(), "Chompy Tyataza");
        assert_eq!(class.as_deref(), Some("Hunter"));
    }

    #[test]
    fn detects_level_from_saved_variables() {
        let temp = tempfile::tempdir().unwrap();
        let acc_dir = temp.path().join("Account/12345#1");
        let char_dir = acc_dir.join("91/Chompy-Tyataza");
        let sv_dir = acc_dir.join("SavedVariables");
        fs::create_dir_all(&char_dir).unwrap();
        fs::create_dir_all(&sv_dir).unwrap();

        let lua_data = "ZygorData = {\n\
            [\"Chompy\"] = {\n\
                [\"level\"] = 25,\n\
                [\"class\"] = \"HUNTER\",\n\
            },\n\
        }\n";
        fs::write(sv_dir.join("Zygor.lua"), lua_data).unwrap();

        let level = parse_character_level(&char_dir, &acc_dir, "Chompy-Tyataza");
        assert_eq!(level, Some(25));
    }

    #[test]
    fn formats_enriched_details_and_state() {
        let info = CharacterInfo {
            character: Some("Chompy Tyataza".to_string()),
            class: Some("Hunter".to_string()),
            level: Some(25),
            realm: "PvE".to_string(),
            in_world: true,
        };
        assert_eq!(info.format_details(), "Chompy Tyataza - Lvl 25 Hunter");
        assert_eq!(info.format_state(), "In World - PvE");
    }

    #[test]
    fn gracefully_degrades_when_level_or_class_missing() {
        let info = CharacterInfo {
            character: Some("Chompy Tyataza".to_string()),
            class: Some("Hunter".to_string()),
            level: None,
            realm: "PvE".to_string(),
            in_world: true,
        };
        assert_eq!(info.format_details(), "Chompy Tyataza - Hunter");
        assert_eq!(info.format_state(), "In World - PvE");

        let info_bare = CharacterInfo {
            character: Some("Chompy Tyataza".to_string()),
            class: None,
            level: None,
            realm: "Silvermoon".to_string(),
            in_world: true,
        };
        assert_eq!(info_bare.format_details(), "Chompy Tyataza");
        assert_eq!(info_bare.format_state(), "In World - Silvermoon");
    }

    #[test]
    fn cached_detection_refreshes_when_saved_variables_change() {
        let temp = tempfile::tempdir().unwrap();
        let wtf = temp.path().join("WTF");
        let acc_dir = wtf.join("Account/12345#1");
        let char_dir = acc_dir.join("Silvermoon/Arthas");
        let sv_dir = acc_dir.join("SavedVariables");
        fs::create_dir_all(&char_dir).unwrap();
        fs::create_dir_all(&sv_dir).unwrap();
        fs::write(wtf.join("Config.wtf"), "SET realmName \"Silvermoon\"\n").unwrap();
        fs::write(char_dir.join("config-cache.wtf"), "SET EJLootClass \"6\"\n").unwrap();
        fs::write(
            sv_dir.join("Addon.lua"),
            "DB = { [\"Arthas\"] = { [\"level\"] = 79 } }\n",
        )
        .unwrap();

        let mut cache = CharacterCache::default();
        let first = detect_character_cached(temp.path(), None, &mut cache).unwrap();
        assert_eq!(first.level, Some(79));
        assert_eq!(first.class.as_deref(), Some("Death Knight"));

        // Ensure filesystem mtime granularity advances before updating SavedVariables.
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(
            sv_dir.join("Addon.lua"),
            "DB = { [\"Arthas\"] = { [\"level\"] = 80 } }\n",
        )
        .unwrap();

        let second = detect_character_cached(temp.path(), None, &mut cache).unwrap();
        assert_eq!(second.level, Some(80));
    }

    #[test]
    fn class_and_level_ignore_other_characters_and_unrelated_level_keys() {
        let temp = tempfile::tempdir().unwrap();
        let acc_dir = temp.path().join("Account/12345#1");
        let char_dir = acc_dir.join("Silvermoon/Arthas");
        let sv_dir = acc_dir.join("SavedVariables");
        fs::create_dir_all(&char_dir).unwrap();
        fs::create_dir_all(&sv_dir).unwrap();

        let lua_data = "AddonDB = {\n\
            [\"Jaina\"] = {\n\
                [\"class\"] = \"Mage\",\n\
                [\"level\"] = 70,\n\
            },\n\
            [\"Arthas\"] = {\n\
                [\"detail_level\"] = 2,\n\
                [\"level\"] = 80,\n\
                [\"class\"] = \"Death Knight\",\n\
            },\n\
        }\n";
        fs::write(sv_dir.join("AddonDB.lua"), lua_data).unwrap();

        assert_eq!(
            parse_character_class(&char_dir, &acc_dir, "Arthas").as_deref(),
            Some("Death Knight")
        );
        assert_eq!(parse_character_level(&char_dir, &acc_dir, "Arthas"), Some(80));
    }

    #[test]
    fn cvar_realm_wins_over_other_realms_regardless_of_mtime() {
        let temp = tempfile::tempdir().unwrap();
        let wtf = temp.path().join("WTF");
        fs::create_dir_all(&wtf).unwrap();
        fs::write(wtf.join("Config.wtf"), "SET realmName \"Silvermoon\"\n").unwrap();

        let acc = wtf.join("Account/12345#1");
        fs::create_dir_all(acc.join("Silvermoon/Arthas")).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::create_dir_all(acc.join("Stormrage/Jaina")).unwrap();

        let info = detect_character(temp.path()).unwrap();
        assert_eq!(info.realm, "Silvermoon");
        assert_eq!(info.character.as_deref(), Some("Arthas"));
    }
}
