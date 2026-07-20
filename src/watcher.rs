//! Pure presence / join logic (no Steam I/O) — unit-tested.

use std::collections::HashMap;

pub const CS2_APP_ID: u32 = 730;

/// How to ask Steam/CS2 to join this friend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JoinMethod {
    /// Classic Steam lobby URI.
    Lobby { lobby_id: u64 },
    /// CS2 rich-presence connect, e.g. `+gcconnect…`.
    Connect { connect: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FriendPresence {
    OfflineOrUnknown,
    OtherGame { app_id: u32 },
    /// In CS2 but no Steam join path (lobby id / connect string).
    InCs2Full,
    Joinable { method: JoinMethod },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchedFriendStatus {
    pub steam_id: u64,
    pub name: String,
    pub presence: FriendPresence,
    /// Map / score / mode line from rich presence when available.
    pub detail: String,
    /// Full rich-presence key dump for debugging.
    pub rich_debug: String,
}

/// Classify from Steamworks lobby id + CS2 rich-presence keys.
///
/// Only treat as joinable when Steam exposes a real join path:
/// a lobby id, or a `connect` / `+gcconnect…` string.
/// Party size keys (`members:numPlayers`, `steam_player_group_size`) are **not**
/// join signals — Premier/comp often show open party seats while mid-match and
/// Steam still hides Join Game.
pub fn classify_presence(
    game_app_id: Option<u32>,
    lobby_id: Option<u64>,
    rich: &HashMap<String, String>,
) -> FriendPresence {
    let Some(app_id) = game_app_id else {
        return FriendPresence::OfflineOrUnknown;
    };
    if app_id != CS2_APP_ID {
        return FriendPresence::OtherGame { app_id };
    }

    if let Some(id) = lobby_id.filter(|id| *id != 0) {
        return FriendPresence::Joinable {
            method: JoinMethod::Lobby { lobby_id: id },
        };
    }

    if let Some(connect) = rich
        .get("connect")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        return FriendPresence::Joinable {
            method: JoinMethod::Connect {
                connect: connect.to_string(),
            },
        };
    }

    FriendPresence::InCs2Full
}

/// First friend in `order` that is currently joinable.
pub fn first_joinable<'a>(
    order: &[u64],
    statuses: &'a [WatchedFriendStatus],
) -> Option<&'a WatchedFriendStatus> {
    for id in order {
        if let Some(s) = statuses.iter().find(|s| s.steam_id == *id) {
            if matches!(s.presence, FriendPresence::Joinable { .. }) {
                return Some(s);
            }
        }
    }
    None
}

pub fn join_lobby_uri(lobby_id: u64, friend_steam_id: u64) -> String {
    format!("steam://joinlobby/{CS2_APP_ID}/{lobby_id}/{friend_steam_id}")
}

/// Launch CS2 with a rich-presence connect string (`+gcconnect…`).
pub fn join_connect_uri(connect: &str) -> String {
    let args = connect.trim();
    // steam://run/<appid>//<args> — encode so '+' survives.
    let encoded: String = args
        .bytes()
        .map(|b| match b {
            b' ' => "%20".to_string(),
            b'+' => "%2B".to_string(),
            b':' => "%3A".to_string(),
            b'/' => "%2F".to_string(),
            b'?' => "%3F".to_string(),
            b'#' => "%23".to_string(),
            b'&' => "%26".to_string(),
            b'=' => "%3D".to_string(),
            c if c.is_ascii_alphanumeric() || c == b'-' || c == b'_' || c == b'.' => {
                (c as char).to_string()
            }
            c => format!("%{c:02X}"),
        })
        .collect();
    format!("steam://run/{CS2_APP_ID}//{encoded}")
}

/// Prefer CS2's own display line (`steam_display` / `status`), else compose from keys.
pub fn format_rich_detail(keys: &HashMap<String, String>) -> String {
    let get = |k: &str| keys.get(k).map(|s| s.trim()).filter(|s| !s.is_empty());

    if let Some(display) = get("steam_display").filter(|s| !s.starts_with('#')) {
        return display.to_string();
    }
    if let Some(status) = get("status") {
        return status.to_string();
    }

    let map = get("game:map")
        .or_else(|| get("game:mapgroupname"))
        .or_else(|| get("map"));
    let mode = get("game:mode").or_else(|| get("mode"));
    let score = get("game:score").or_else(|| get("score"));

    let mut parts: Vec<String> = Vec::new();
    if let Some(m) = mode {
        parts.push(pretty_token(m));
    }
    if let Some(m) = map {
        parts.push(pretty_map(m));
    }
    if let Some(s) = score {
        parts.push(s.to_string());
    }
    parts.join(" · ")
}

/// Sorted `key=value | …` dump for debug UI / logs.
pub fn format_rich_debug(keys: &HashMap<String, String>) -> String {
    let mut pairs: Vec<_> = keys.iter().collect();
    pairs.sort_by(|a, b| a.0.cmp(b.0));
    pairs
        .into_iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(" | ")
}

fn pretty_map(raw: &str) -> String {
    let name = raw.rsplit('/').next().unwrap_or(raw);
    let name = name.strip_prefix("de_").unwrap_or(name);
    let name = name.strip_prefix("cs_").unwrap_or(name);
    let name = name.strip_prefix("gd_").unwrap_or(name);
    name.split('_')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn pretty_token(raw: &str) -> String {
    let mut c = raw.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

pub fn format_elapsed(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotifyKey {
    pub steam_id: u64,
    /// Distinguishes lobby id vs connect token.
    pub join_token: String,
}

impl NotifyKey {
    pub fn from_joinable(steam_id: u64, method: &JoinMethod) -> Self {
        let join_token = match method {
            JoinMethod::Lobby { lobby_id } => format!("lobby:{lobby_id}"),
            JoinMethod::Connect { connect } => format!("connect:{connect}"),
        };
        Self {
            steam_id,
            join_token,
        }
    }
}

#[derive(Debug, Default)]
pub struct NotifyDebouncer {
    last_notified: Option<NotifyKey>,
    cooldown_until: Option<std::time::Instant>,
}

impl NotifyDebouncer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn consider(
        &mut self,
        joinable: Option<NotifyKey>,
        now: std::time::Instant,
        cooldown: std::time::Duration,
    ) -> Option<NotifyKey> {
        let Some(key) = joinable else {
            if self.cooldown_until.is_none_or(|t| now >= t) {
                self.last_notified = None;
            }
            return None;
        };

        if self.cooldown_until.is_some_and(|t| now < t) {
            return None;
        }
        if self.last_notified.as_ref() == Some(&key) {
            return None;
        }

        self.last_notified = Some(key.clone());
        self.cooldown_until = Some(now + cooldown);
        Some(key)
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn keys(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn lobby_id_is_joinable() {
        assert_eq!(
            classify_presence(Some(730), Some(99), &HashMap::new()),
            FriendPresence::Joinable {
                method: JoinMethod::Lobby { lobby_id: 99 }
            }
        );
    }

    #[test]
    fn connect_string_is_joinable() {
        let rich = keys(&[("connect", "+gcconnectABC"), ("game:map", "de_nuke")]);
        assert_eq!(
            classify_presence(Some(730), None, &rich),
            FriendPresence::Joinable {
                method: JoinMethod::Connect {
                    connect: "+gcconnectABC".into()
                }
            }
        );
    }

    #[test]
    fn premier_open_party_slots_are_not_joinable() {
        // Real-world Premier mid-match presence: party seats look free, Steam still
        // hides Join Game (no lobby id, no connect, system:lock=mmqueue).
        let rich = keys(&[
            ("game:map", "de_mirage"),
            ("game:mode", "competitive"),
            ("game:score", "[ 8 : 11 ]"),
            ("members:numPlayers", "4"),
            ("members:numSlots", "10"),
            ("steam_player_group_size", "4"),
            ("system:access", "public"),
            ("system:lock", "mmqueue"),
            ("steam_display", "Premier - Mirage [ 8 : 11 ]"),
        ]);
        assert_eq!(
            classify_presence(Some(730), None, &rich),
            FriendPresence::InCs2Full
        );
    }

    #[test]
    fn member_slots_alone_not_joinable() {
        let rich = keys(&[
            ("members:numPlayers", "4"),
            ("members:numSlots", "10"),
            ("game:map", "de_mirage"),
        ]);
        assert_eq!(
            classify_presence(Some(730), None, &rich),
            FriendPresence::InCs2Full
        );
    }

    #[test]
    fn detail_prefers_steam_display() {
        let rich = keys(&[
            ("steam_display", "Casual - Nuke [ 0 : 1 ]"),
            ("game:map", "de_nuke"),
        ]);
        assert_eq!(format_rich_detail(&rich), "Casual - Nuke [ 0 : 1 ]");
    }

    #[test]
    fn connect_uri_encodes_plus() {
        assert_eq!(
            join_connect_uri("+gcconnectABC"),
            "steam://run/730//%2BgcconnectABC"
        );
    }

    #[test]
    fn first_available_respects_order() {
        let statuses = vec![
            WatchedFriendStatus {
                steam_id: 2,
                name: "B".into(),
                presence: FriendPresence::Joinable {
                    method: JoinMethod::Lobby { lobby_id: 1 },
                },
                detail: String::new(),
                rich_debug: String::new(),
            },
            WatchedFriendStatus {
                steam_id: 1,
                name: "A".into(),
                presence: FriendPresence::Joinable {
                    method: JoinMethod::Connect {
                        connect: "+gcconnectX".into(),
                    },
                },
                detail: "Casual Nuke".into(),
                rich_debug: String::new(),
            },
        ];
        let first = first_joinable(&[1, 2], &statuses).unwrap();
        assert_eq!(first.steam_id, 1);
    }

    #[test]
    fn join_lobby_uri_format() {
        assert_eq!(join_lobby_uri(111, 222), "steam://joinlobby/730/111/222");
    }

    #[test]
    fn elapsed_format() {
        assert_eq!(format_elapsed(65), "01:05");
        assert_eq!(format_elapsed(3661), "1:01:01");
    }

    #[test]
    fn debouncer_fires_once_then_cooldown() {
        let mut d = NotifyDebouncer::new();
        let t0 = Instant::now();
        let key = NotifyKey {
            steam_id: 1,
            join_token: "connect:x".into(),
        };
        assert_eq!(
            d.consider(Some(key.clone()), t0, Duration::from_secs(30)),
            Some(key.clone())
        );
        assert_eq!(
            d.consider(
                Some(key.clone()),
                t0 + Duration::from_secs(1),
                Duration::from_secs(30)
            ),
            None
        );
        assert_eq!(
            d.consider(None, t0 + Duration::from_secs(32), Duration::from_secs(30)),
            None
        );
        assert_eq!(
            d.consider(
                Some(key.clone()),
                t0 + Duration::from_secs(33),
                Duration::from_secs(30)
            ),
            Some(key)
        );
    }
}
