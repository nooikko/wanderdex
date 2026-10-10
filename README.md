# Wanderdex

Pokémon that wander your desktop. Pick any Pokémon from the PMD Sprite Repository, spawn it, and it walks
back and forth along the bottom of your screen, naps, and shows off its moves. Shiny and alternate
forms are supported where the sprites exist.

## Download

Get the latest build from [Releases](../../releases):
- **macOS:** `Wanderdex_x.y.z_universal.dmg`
- **Windows:** `Wanderdex_x.y.z_x64-setup.exe`

### First launch (unsigned app)

Wanderdex isn't code-signed, so your OS will warn you the first time:
- **macOS:** open the app, dismiss the warning, then go to **System Settings → Privacy & Security** and click **Open Anyway**.
  If macOS instead says the app "is damaged and can't be opened", run this once in Terminal, then open it again:
  `xattr -cr /Applications/Wanderdex.app`
- **Windows:** on the SmartScreen prompt, click **More info → Run anyway**.

## Using it

- Click the Wanderdex icon in the menu bar (macOS) or system tray (Windows) → **Open picker**.
- Search for a Pokémon, choose form / shiny / female if available, and press **Spawn**.
- Click a pet to make it do a move. Drag it around and drop it. Right-click for more.
- Tray menu: Size (2×/3×/4×), Hide all, Launch at login, Credits & license.

## Credits

Sprites: [PMD Sprite Repository (SpriteCollab)](https://github.com/PMDCollab/SpriteCollab), licensed
[CC BY-NC 4.0](https://creativecommons.org/licenses/by-nc/4.0/). Artist credits for every Pokémon you spawn
are shown in the app (tray → Credits & license). Sprites are downloaded at runtime and are not part of this repository.

Wanderdex is a non-commercial fan project. Pokémon is © Nintendo / Creatures Inc. / GAME FREAK inc.
Wanderdex is not affiliated with or endorsed by them.

## Development

Requires Rust (stable), Node 22+, pnpm 10.

    pnpm install
    pnpm tauri dev                       # run the app
    pnpm test && cargo test --manifest-path src-tauri/Cargo.toml
    pnpm contact-sheet 0202              # render-check a variant (macOS)
    ./scripts/update-index.sh            # refresh the sprite index from SpriteCollab HEAD

Code is MIT licensed (see LICENSE).
