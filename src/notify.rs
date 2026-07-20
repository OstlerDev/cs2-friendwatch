use crate::watcher::{join_connect_uri, join_lobby_uri, JoinMethod};
use notify_rust::Notification;

pub fn open_join(method: &JoinMethod, friend_steam_id: u64) -> Result<(), String> {
    match method {
        JoinMethod::Lobby { lobby_id } => {
            let uri = join_lobby_uri(*lobby_id, friend_steam_id);
            open::that(&uri).map_err(|e| format!("failed to open {uri}: {e}"))
        }
        JoinMethod::Connect { connect } => {
            let uri = join_connect_uri(connect);
            open::that(&uri).map_err(|e| format!("failed to open {uri}: {e}"))
        }
    }
}

pub fn notify_spot_available(friend_name: &str) {
    let body = format!("{friend_name} has a free CS2 spot — open Friendwatch to join.");
    let _ = Notification::new()
        .summary("CS2 Friendwatch")
        .body(&body)
        .appname("cs2-friendwatch")
        .timeout(10_000)
        .show();
}
