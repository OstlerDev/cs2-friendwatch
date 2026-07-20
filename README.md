# cs2-friendwatch

Watch selected Steam friends for a free Counter-Strike 2 lobby spot, get a desktop notification, then confirm Join from the app.

When a friend has a joinable Steam lobby, the same signal that powers **Join Game** in the friends list becomes available. Friendwatch polls that signal and prompts you — it does **not** auto-join unattended.

## Requirements

- Windows (primary target)
- [Rust](https://rustup.rs/) (edition 2021+)
- Steam client running and logged in
- You own / can run CS2 (AppID 730)

## Build & run

```bash
cargo run --release
```

Or build and launch the binary directly:

```bash
cargo build --release
./target/release/cs2-friendwatch.exe
```

A `build.rs` step copies **`steam_api64.dll`** and **`steam_appid.txt`** next to the executable under `target/release/` (required at runtime — Windows will error if the DLL is missing).

`steam_appid.txt` contains `480` (Spacewar) so Steamworks can initialize without conflicting with CS2. If you move the `.exe`, keep both `steam_api64.dll` and `steam_appid.txt` beside it.

## Usage

1. Start Steam, then start Friendwatch.
   - Prefer starting Friendwatch **before** CS2 so it can connect as app **730** (needed for map/score rich presence). If CS2 is already running, it falls back to app **480** and rich presence may be limited.
2. The list shows **only friends currently in CS2**, with map/score from Steam rich presence. Click a row to watch them.
3. Click **Start watching**. A live timer shows how long you’ve been waiting (while idle, the friend list refreshes every **15 seconds**).
4. A friend is treated as joinable when Steam exposes a lobby id, a `+gcconnect…` connect string, or free party/match slots (`members:numPlayers` < `members:numSlots`).
5. When a spot opens: sound + always-on-top alert window + desktop notification. **Join** / **Dismiss** from the popup.
6. After you join, watching **stops** — click **Start watching** again to look for another spot. Friends who leave CS2 are removed from the watch queue automatically.

Watch selection is saved under your OS config dir, e.g. `%APPDATA%/cs2-friendwatch/config.json` on Windows.

## Status meanings

| Status | Meaning |
|--------|---------|
| Offline / unknown | Not in a game (or presence unavailable) |
| Other game | In a non-CS2 game |
| In CS2 (no lobby / full) | In CS2 but no joinable Steam lobby ID (often a full party or already in a match) |
| Joinable | Lobby id, `connect` string, or free member slots |

Rich-presence lines prefer CS2’s `steam_display` / `status` (e.g. `Casual - Nuke [ 0 : 1 ]`).

## Manual test checklist

- [ ] Steam not running → clear error, no crash
- [ ] Friend in full CS2 lobby → status shows no lobby / full; no notification
- [ ] Spot opens → notification + in-app Join banner
- [ ] Join → Steam/CS2 join flow; watching pauses
- [ ] Multiple watched friends joinable → only the first in watch order notifies

## Limits

- This is still a race against other friends grabbing the spot.
- Privacy / party settings can hide lobby IDs even when someone looks in-game.
- Confirming Join uses the documented `steam://joinlobby` URI — the same class of action as clicking Join Game yourself.

## License

MIT
