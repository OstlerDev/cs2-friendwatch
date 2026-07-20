use crate::watcher::{
    classify_presence, format_rich_detail, FriendPresence, WatchedFriendStatus, CS2_APP_ID,
};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use steamworks::{sys, Client, FriendFlags, SteamId};

/// Spacewar — fallback when CS2 (730) is already running and holds the app slot.
pub const FALLBACK_APP_ID: u32 = 480;

pub struct SteamSession {
    client: Client,
    pub app_id: u32,
}

#[derive(Debug, Clone)]
pub struct FriendInfo {
    pub steam_id: u64,
    pub name: String,
    pub presence: FriendPresence,
    pub detail: String,
    /// Medium avatar RGBA (64×64×4), if available this poll.
    pub avatar_rgba: Option<Vec<u8>>,
}

impl SteamSession {
    pub fn init() -> Result<Self, String> {
        match Client::init_app(CS2_APP_ID) {
            Ok(client) => Ok(Self {
                client,
                app_id: CS2_APP_ID,
            }),
            Err(e730) => Client::init_app(FALLBACK_APP_ID)
                .map(|client| Self {
                    client,
                    app_id: FALLBACK_APP_ID,
                })
                .map_err(|e480| {
                    format!(
                        "Steamworks init failed (730: {e730}; 480: {e480}). Is Steam running and are you logged in?"
                    )
                }),
        }
    }

    pub fn run_callbacks(&self) {
        self.client.run_callbacks();
    }

    fn friends_ptr(&self) -> *mut sys::ISteamFriends {
        unsafe { sys::SteamAPI_SteamFriends_v018() }
    }

    /// Open friend profile in the Steam overlay (fallback join path).
    pub fn open_friend_overlay(&self, steam_id: u64) {
        self.client
            .friends()
            .activate_game_overlay_to_user("steamid", SteamId::from_raw(steam_id));
    }

    pub fn list_cs2_friends(&self) -> Vec<FriendInfo> {
        let friends_api = self.client.friends();
        let ptr = self.friends_ptr();
        let mut list: Vec<FriendInfo> = friends_api
            .get_friends(FriendFlags::IMMEDIATE)
            .into_iter()
            .filter_map(|f| {
                let game = f.game_played()?;
                let app_id = game.game.app_id().0;
                if app_id != CS2_APP_ID {
                    return None;
                }
                let steam_id = f.id().raw();
                friends_api.request_user_information(f.id(), false);
                request_rich_presence(ptr, steam_id);

                let keys = read_rich_presence(ptr, steam_id);
                let lobby = game.lobby.raw();
                let lobby_opt = if lobby == 0 { None } else { Some(lobby) };
                Some(FriendInfo {
                    steam_id,
                    name: f.name(),
                    presence: classify_presence(Some(app_id), lobby_opt, &keys),
                    detail: format_rich_detail(&keys),
                    avatar_rgba: f.medium_avatar(),
                })
            })
            .collect();
        list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        list
    }

    pub fn poll_watched(&self, watched: &[u64], names: &[(u64, String)]) -> Vec<WatchedFriendStatus> {
        let friends_api = self.client.friends();
        let ptr = self.friends_ptr();
        watched
            .iter()
            .map(|&id| {
                let name = names
                    .iter()
                    .find(|(sid, _)| *sid == id)
                    .map(|(_, n)| n.clone())
                    .unwrap_or_else(|| id.to_string());
                let friend = friends_api.get_friend(SteamId::from_raw(id));
                friends_api.request_user_information(SteamId::from_raw(id), false);
                request_rich_presence(ptr, id);
                let keys = read_rich_presence(ptr, id);
                let presence = match friend.game_played() {
                    Some(game) => {
                        let app_id = game.game.app_id().0;
                        let lobby = game.lobby.raw();
                        let lobby_opt = if lobby == 0 { None } else { Some(lobby) };
                        classify_presence(Some(app_id), lobby_opt, &keys)
                    }
                    None => FriendPresence::OfflineOrUnknown,
                };
                WatchedFriendStatus {
                    steam_id: id,
                    name,
                    presence,
                    detail: format_rich_detail(&keys),
                }
            })
            .collect()
    }
}

fn request_rich_presence(friends: *mut sys::ISteamFriends, steam_id: u64) {
    if friends.is_null() {
        return;
    }
    unsafe {
        sys::SteamAPI_ISteamFriends_RequestFriendRichPresence(friends, steam_id);
    }
}

fn read_rich_presence(friends: *mut sys::ISteamFriends, steam_id: u64) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if friends.is_null() {
        return map;
    }

    const KNOWN: &[&str] = &[
        "status",
        "steam_display",
        "connect",
        "score",
        "game:state",
        "game:mode",
        "game:map",
        "game:mapgroupname",
        "game:score",
        "game:server",
        "game:act",
        "members:numPlayers",
        "members:numSlots",
        "steam_player_group",
        "steam_player_group_size",
        "system:access",
        "system:lock",
        "version",
        "time",
    ];

    unsafe {
        for key in KNOWN {
            if let Some(val) = get_rp_value(friends, steam_id, key) {
                map.insert((*key).to_string(), val);
            }
        }

        let count = sys::SteamAPI_ISteamFriends_GetFriendRichPresenceKeyCount(friends, steam_id);
        for i in 0..count {
            let key_ptr =
                sys::SteamAPI_ISteamFriends_GetFriendRichPresenceKeyByIndex(friends, steam_id, i);
            if key_ptr.is_null() {
                continue;
            }
            let key = CStr::from_ptr(key_ptr).to_string_lossy().into_owned();
            if key.is_empty() || map.contains_key(&key) {
                continue;
            }
            if let Some(val) = get_rp_value(friends, steam_id, &key) {
                map.insert(key, val);
            }
        }
    }
    map
}

unsafe fn get_rp_value(
    friends: *mut sys::ISteamFriends,
    steam_id: u64,
    key: &str,
) -> Option<String> {
    let c_key = CString::new(key).ok()?;
    let val_ptr =
        sys::SteamAPI_ISteamFriends_GetFriendRichPresence(friends, steam_id, c_key.as_ptr());
    if val_ptr.is_null() {
        return None;
    }
    let val = CStr::from_ptr(val_ptr).to_string_lossy();
    let trimmed = val.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
