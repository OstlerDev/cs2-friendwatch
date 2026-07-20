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

**Distribution:** ship only `cs2-friendwatch.exe`. The Steam API DLL and `steam_appid.txt` are embedded in the binary; on first launch they are extracted under `%LOCALAPPDATA%/cs2-friendwatch/steam/`. No sidecar files need to sit next to the `.exe`.

(`steam_appid.txt` embeds AppID `480` / Spacewar so Steamworks can initialize without conflicting with CS2.)

## Usage

1. Start Steam, then start Friendwatch.
   - Prefer starting Friendwatch **before** CS2 so it can connect as app **730** (needed for map/score rich presence). If CS2 is already running, it falls back to app **480** and rich presence may be limited.
2. The list shows **only friends currently in CS2**, with map/score from Steam rich presence. Click a row to watch them.
3. Click **Start watching**. A live timer shows how long you’ve been waiting (while idle, the friend list refreshes every **15 seconds**).
4. A friend is treated as joinable only when Steam exposes a real join path: a lobby id, or a `+gcconnect…` connect string. Open Premier/comp party seats alone are **not** treated as joinable (Steam still hides Join Game mid-match). Joinable friends also show a green **Join** button on the right of their row for an instant join.
5. When a spot opens: CS2-style **YOUR MATCH IS READY!** popup (ACCEPT / close window to dismiss) + alert sound + desktop notification. Polling continues; if the spot disappears, the popup closes, the sound stops, and watching resumes.
6. After you join, watching **stops** — click **Start watching** again to look for another spot. Friends who leave CS2 are removed from the watch queue automatically.

### Settings (⚙)

Open **⚙** in the top-right to:
- Set alert **volume** and optional **custom sound**
- Configure **active poll** (1–10s, default 1.5) and **idle poll** (5–60s, default 15)
- Toggle **rich presence debug**

Watch selection and settings are saved under your OS config dir, e.g. `%APPDATA%/cs2-friendwatch/config.json` on Windows.

## Status meanings

| Status | Meaning |
|--------|---------|
| Offline / unknown | Not in a game (or presence unavailable) |
| Other game | In a non-CS2 game |
| In CS2 (no lobby / full) | In CS2 but no lobby id or `connect` string (includes mid-Premier with open party seats) |
| Joinable | Lobby id or `connect` string present |

Rich-presence lines prefer CS2’s `steam_display` / `status` (e.g. `Casual - Nuke [ 0 : 1 ]`).

## Manual test checklist

- [ ] Steam not running → clear error, no crash
- [ ] Friend in active Premier (no Join Game in Steam) → not joinable; no notification
- [ ] Friend with `+gcconnect…` / lobby → notification + sound + popup
- [ ] Spot closes while popup is open → popup closes, sound stops, watching continues
- [ ] Join → Steam/CS2 join flow; watching stops
- [ ] Settings volume / custom sound / Test sound work
- [ ] Multiple watched friends joinable → only the first in watch order notifies

## Limits

- This is still a race against other friends grabbing the spot.
- Privacy / party settings can hide lobby IDs even when someone looks in-game.
- Confirming Join uses the documented `steam://joinlobby` / `steam://run` URIs — the same class of action as clicking Join Game yourself.

## License

MIT
