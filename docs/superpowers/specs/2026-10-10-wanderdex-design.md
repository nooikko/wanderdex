# Wanderdex: Design

Date: 2026-10-10
Status: Draft for review

## 1. Purpose

Wanderdex lets people spawn Pokémon onto their desktop as small pixel-art pets that walk back and forth along the bottom of the screen, nap, and do the occasional move. It grows out of the single-Wobbuffet macOS prototype in this folder.

- **Audience:** anyone who finds the GitHub repository. Not commercial and not listed in any app store.
- **Distribution:** GitHub Releases. Unsigned macOS and Windows builds.
- **Success:** someone downloads a release on Mac or Windows, opens the picker, chooses any Pokémon (including shiny or other forms where they exist), and watches it wander the bottom of their screen. Every sprite is correctly attributed.

## 2. Requirements

### Stated by the user
- Pixel-art style only (no smoothing styles).
- A basic interface for picking any Pokémon from PMD SpriteCollab and spawning it.
- Shiny and other variants are selectable when SpriteCollab has them.
- Pets roam: they walk back and forth, live on the bottom edge of the screen, and sometimes do a pose or move (Wobbuffet salutes).
- No interaction between pets in this version.
- Runs on macOS and Windows.
- Open source on GitHub, distributed through GitHub Releases, with full attribution.

### Assumed (confirmed during design)
- Several pets can be on screen at once.
- Sprites download on demand and are cached. They are not bundled with the app.
- Pets can be clicked and dragged.

### Out of scope (later ideas)
- Interaction between pets (for example, Wobbuffet walking up to another pet and saluting it).
- Climbing walls or windows, and walking on window title bars.
- Live 3D models or higher-resolution art sources.
- Linux builds, code signing and notarization, auto-update.

## 3. Platform and stack

- **Tauri v2.** Rust backend, TypeScript frontend, React + Vite for the picker UI.
- Chosen over Electron (100+ MB downloads, heavier per window) and Godot (fiddly click-through, poor Dock and taskbar awareness, clunky list UI).
- macOS window transparency needs Tauri's `macOSPrivateApi` setting. This is acceptable because the app is not distributed through the App Store.

## 4. Architecture

### 4.1 Windows

- **One small transparent window per pet.** It is frameless, always on top, hidden from the Dock, the taskbar and the window switcher, and not focusable.
- The window is sized to the union of opaque pixels across all animations that pet can play, times the scale. This is the same approach as the prototype.
- This design was validated against shipped desktop pets. Shimeji-ee, eSheep and NekoAI (Tauri) all move one window per pet.
- A full-screen click-through overlay was rejected. The one Tauri app using it (WindowPet) has open issues where pets can't leave the main monitor, transparency breaks on Apple Silicon Macs, and toggling click-through too quickly causes crashes.
- Clicks land only inside a pet's window. Transparent corners of that window still capture clicks, same as the prototype.
- **Picker window:** a normal app window, opened from the tray or menu-bar icon.
- **Credits & license window:** a normal app window.
- **No Dock icon on macOS.** The app sets the accessory activation policy. Windows shows a tray icon only.
- **Memory:** each pet window is a web view, estimated at 20 to 40 MB. Measure this early with 1, 5 and 10 pets on both platforms. If it's too high, the fallback is windows without a web view, with sprites drawn from Rust (for example with `softbuffer`). The rest of the design does not change.

### 4.2 Responsibilities

**Rust (`src-tauri`)**
- `index`: loads the bundled index and answers species, form and variant lookups.
- `sprites`: downloads a variant's `AnimData.xml` and the `*-Anim.png` sheets it needs, caches them under the app data directory, and validates them.
- `animdata`: parses `AnimData.xml` into a typed table and resolves `CopyOf` entries. It checks each sheet's size against `columns × FrameWidth` and `rows × FrameHeight`, and gets the row count from the sheet height. It is the only parser. The parsed table is sent to the frontend.
- `brain`: each pet's state machine and movement (section 5). It runs on a Rust timer at 60 Hz, so web-view timer throttling can't make pets stutter.
- `pets`: creates and destroys pet windows, moves them, and sends each window its current animation and direction.
- `persist`: saves and restores active pets and settings as JSON in the app config directory.
- Tray or menu-bar menu, and launch at login (`tauri-plugin-autostart`).

**TypeScript (`src`)**
- `renderer`: draws one frame onto a canvas. It uses nearest-neighbor scaling at integer scales and `imageSmoothingEnabled = false`. It centers each frame on a shared anchor, as in the prototype.
- `pet-window`: receives "play animation X, direction D, loop or once" from Rust and advances frames using the per-frame tick durations, 60 ticks per second. It tells Rust when a one-shot animation finishes. It sends mouse events (click, drag start, drag move, drop, right-click) to Rust.
- `picker`: the React app (section 6).

### 4.3 Sprite data and the index

- `scripts/build-index` fetches `tracker.json` and `credit_names.txt` at a specific SpriteCollab commit, then writes `index.json`, which ships with the app.
- For each species, the index records its Pokédex number, its name, and the form, shiny and female variants that have sprite files. For each variant it records:
  - its sprite path (for example `0202/0000/0001`)
  - its list of animation names
  - whether it has a portrait
  - its credited artists, with names resolved through `credit_names.txt`
- **Only variants with both `Walk` and `Idle` are included.**
- The index records the SpriteCollab commit SHA. Sprite and portrait downloads use that SHA: `https://raw.githubusercontent.com/PMDCollab/SpriteCollab/<sha>/sprite/<path>/...`. Changes in SpriteCollab therefore can't break a released build. Each release regenerates the index.
- The generated `index.json` is committed to the repo, so builds are reproducible.
- **Cache:** `<app data>/sprites/<sha>/<path>/`. If a download fails validation (a sheet size that doesn't match AnimData, or a PNG or XML that won't parse), it is deleted and retried once, then the error is shown.
- Portraits come from `portrait/<dex>/Normal.png`. They load lazily as the picker scrolls, and are cached the same way.

## 5. Pet behavior

### 5.1 Ground
- Pets stand on the bottom edge of the work area of the monitor they're on: on top of the Dock on macOS, or the taskbar on Windows.
- If a monitor's work area changes (for example the Dock is resized or a monitor is unplugged), pets on it move to the new bottom edge. Pets on a monitor that disappears move to the main monitor.

### 5.2 States
| State | Animation | Behavior |
|---|---|---|
| Walk | `Walk` (Left or Right row) | Moves horizontally at a constant speed in screen points per second, scaled with size. Walks a random distance, then picks a new state. Turns around at work-area edges. |
| Idle | `Idle` (Down row, facing the user) | Lasts a random 2 to 8 seconds. |
| Flourish | A random choice from the variant's available animations in a preference list: `Pose`, `Hop`, `Rotate`, `Charge`. Lunging attacks (`Attack`, `Swing`, `Double`) are excluded because they throw the body around the frame. | Plays once. |
| Signature | Taken from `overrides.json`, keyed by Pokédex number (for example `"0202": "Withdraw"`) | Plays once. Replaces Flourish when an override exists. |
| Nap | `Sleep` | Lasts a random 20 to 60 seconds. Only if the variant has `Sleep`. |
| Dragged | `Walk`, facing the drag direction (8 directions) | Follows the cursor. Gives the "spinning" effect. |
| Falling | `Walk` (Down row) | Falls with gravity to the ground after a drop. |
| Landing | `Hop` | Plays once on touching the ground. Falls back to `Idle` if missing. |

- **Transitions:** after Walk, Idle, Flourish or Nap ends, the next state is chosen by weighted random. Starting weights are Walk 50, Idle 30, Flourish 15 and Nap 5. These live in one constants block for tuning.
- Each pet has its own random generator, seeded per pet, so pets don't move in sync. Tests use fixed seeds.
- **Missing animations:** if a state's animation is missing, that state is skipped. Walk and Idle are guaranteed by the index filter.
- **Directions:** row 0 = Down, then counterclockwise: DownRight, Right, UpRight, Up, UpLeft, Left, DownLeft. Animations with a single row always use row 0.
- **Spawn:** a new pet appears on the ground at a random position on the monitor the picker is on, then idles.

### 5.3 Interaction
- **Click** (press and release without moving more than 3 points): plays the Signature, or a Flourish if there's no signature.
- **Drag:** Dragged state. On release, Falling, then Landing.
- **Right-click menu:** Do a move · Credits for this Pokémon · Remove · Remove all · Open picker.

### 5.4 Size
- One global scale: 2×, 3× or 4×. The default is 3×. Integer only, nearest-neighbor only.
- All species share the same scale, so their sizes compared to each other match the games.

## 6. Picker and menus

### 6.1 Picker window
- **Search:** filters by name or Pokédex number.
- **Grid:** one tile per species, showing the portrait at 2× with nearest-neighbor scaling, the name and the number. Species without a portrait show a plain numbered tile.
- **Detail panel:** opens when a species is selected.
  - Form dropdown, listing indexed forms only.
  - Shiny and Female toggles, shown only when that variant exists for the selected form.
  - Live preview cycling Idle → Walk → Signature or Flourish, using the same renderer.
  - Credits line for the exact variant.
  - **Spawn** button. It can be pressed repeatedly.
- **"On your desktop" strip:** active pets as small icons. Clicking one removes it. There's also a Remove all button.

### 6.2 Tray or menu-bar menu
Open picker · Size (2× / 3× / 4×) · Hide all / Show all · Launch at login · Credits & license · Quit.

### 6.3 Credits & license window
- The CC BY-NC 4.0 license for the sprites, with a link to SpriteCollab.
- The credited artists for every variant the user has spawned.
- A non-affiliation notice: Pokémon is © Nintendo, Creatures and GAME FREAK.

### 6.4 Errors
- Download failure: the detail panel shows "Couldn't download sprites. Check your connection." with a Retry button. Variants already cached keep working offline.
- Validation failure: the cache entry is deleted and the download retried once, then the same message is shown.
- Saved pets whose variant is no longer in the index (after an update) are skipped on restore and logged.

## 7. Repository, licensing and releases

- `git init` in this folder.
  - **First commit:** the Swift prototype, without downloaded sprite assets, preview images, build output or the GPL-3 xBRZ sources.
  - **After that:** the Tauri app is built at the repo root. The Swift prototype is deleted once Wanderdex reaches feature parity: one pet that walks, idles, salutes, drags and drops.
- Our code is MIT-licensed. Sprites are never committed. They download at runtime with attribution. The xBRZ and MMPX code is not carried over.
- `README.md`:
  - what Wanderdex is
  - screenshots
  - download links
  - first-launch steps for unsigned apps (macOS: System Settings → Privacy & Security → Open Anyway; Windows: SmartScreen → More info → Run anyway)
  - credits and non-affiliation notice
- **GitHub Actions:**
  - On pull requests: Rust tests, TypeScript tests and type checks.
  - On `v*` tags: `tauri-action` builds a universal macOS `.dmg` and a Windows NSIS installer, and attaches them to a draft GitHub Release.

## 8. Testing

- **Rust unit tests:**
  - `animdata`, using Wobbuffet's real `AnimData.xml` and sheet sizes as test data, including `CopyOf` resolution and size checks
  - tick-to-frame timing for looping and one-shot animations
  - direction mapping from drag movement to row
  - `build-index` filtering and credit resolution, using a small trimmed tracker as test data
  - `brain` transitions and turnarounds with fixed seeds
  - ground placement when the work area changes
- **TypeScript tests (Vitest):** renderer frame-rectangle math and anchor alignment, and frame advancement from durations.
- **Contact-sheet command:** a dev-only command that renders the first frame of every animation for a chosen variant to a PNG, with automated checks for crispness (every scale×scale block is a single color) and clipping. This is the same method used to verify the prototype.
- **Manual checks on macOS and on the user's Windows PC:** spawn, walk, flourish, nap, drag and drop across monitors, size changes, restore after restart, offline behavior. Memory use with 1, 5 and 10 pets.
