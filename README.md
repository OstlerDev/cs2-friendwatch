# CS2 Friendwatch

Keeps an eye on your Steam friends who are in **Counter-Strike 2**, and pings you when a **Join Game** spot opens up.

You still choose when to join — Friendwatch never auto-joins for you.

## Download

1. [Download the latest `cs2-friendwatch.exe` from the Releases page.](https://github.com/OstlerDev/cs2-friendwatch/releases/latest/download/cs2-friendwatch.exe)
2. Put it anywhere you like and run it.

That’s the whole install. No installer, no extra DLLs next to the exe.

## Before you start

- **Windows**
- **Steam** open and logged in
- You own / can launch **CS2**

## How to use

1. Start **Steam**, **CS2**, then start **FriendWatch**.
2. You’ll see friends who are currently in CS2. Click the ones you want to watch for an open spot.
3. Hit **Start watching**.
4. When a spot opens you’ll get:
   - a CS2-style **YOUR MATCH IS READY!** popup
   - an alert sound
   - a desktop notification
5. Click **ACCEPT** (or the green **Join** on their row) to join through Steam — same idea as clicking Join Game yourself.

Open **⚙** in the corner for volume, custom sound, and how often it checks.

## Notes

- Someone else can still grab the spot first. It’s a race.
- Some friends’ privacy / party settings hide join info. Nothing FriendWatch can do about that.
- Your watch list and settings are saved automatically.

## Build from source (optional)

If you’d rather compile it yourself:

```bash
cargo run --release
```

Needs [Rust](https://rustup.rs/) installed.

## License

MIT
