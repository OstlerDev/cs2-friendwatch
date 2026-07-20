# CS2 Friendwatch

Keeps an eye on your Steam friends who are in **Counter-Strike 2**, and pings you when a **Join Game** spot opens up.

You still choose when to join — Friendwatch never auto-joins for you.

## Download

1. Open the [**Releases**](releases) page on GitHub.
2. Grab the latest `cs2-friendwatch-…-windows-x64.exe`.
3. Put it anywhere you like and run it.

That’s the whole install. No installer, no extra DLLs next to the exe.

## Before you start

- **Windows**
- **Steam** open and logged in
- You own / can launch **CS2**

## How to use

1. Start **Steam**, then start **Friendwatch**.
   - Tip: start Friendwatch *before* CS2 if you want map/score info in the list.
2. You’ll see friends who are currently in CS2. Click the ones you want to watch.
3. Hit **Start watching**.
4. When a spot opens you’ll get:
   - a CS2-style **YOUR MATCH IS READY!** popup
   - an alert sound
   - a desktop notification
5. Click **ACCEPT** (or the green **Join** on their row) to join through Steam — same idea as clicking Join Game yourself.
6. After you join, watching stops. Hit **Start watching** again if you want another spot.

Open **⚙** in the corner for volume, custom sound, and how often it checks.

## What “joinable” means

Friendwatch only alerts when Steam actually offers a join path (the same signal as Join Game in your friends list).

If a friend is mid-Premier / mid-match and Steam still hides Join Game, Friendwatch won’t treat them as joinable either — even if their party looks like it has empty seats.

## Tips

- Someone else can still grab the spot first. It’s a race.
- Some friends’ privacy / party settings hide join info. Nothing Friendwatch can do about that.
- Your watch list and settings are saved automatically.

## Build from source (optional)

If you’d rather compile it yourself:

```bash
cargo run --release
```

Needs [Rust](https://rustup.rs/) installed.

## License

MIT
