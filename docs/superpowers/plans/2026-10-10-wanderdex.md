# Wanderdex Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Tauri v2 app for macOS and Windows that spawns PMD SpriteCollab Pokémon as pixel-art desktop pets that wander the bottom edge of the screen.

**Architecture:**
- **One small transparent window per pet.**
- **Rust owns the pets and data:** each pet's behavior state machine on a 60 Hz thread, window movement, sprite downloading and caching, AnimData.xml parsing, saved settings, and the tray menu.
- **TypeScript draws:** it renders frames with nearest-neighbor scaling, and runs the React picker and credits windows.
- **Sprite index:** a compact index built from SpriteCollab's tracker.json ships with the app, pinned to one SpriteCollab commit.

**Tech Stack:**
- Rust: Tauri 2.12.3, tauri-plugin-autostart 2.7.0, roxmltree 0.21.1, png 0.18.1, rand 0.10.3 + rand_chacha 0.10.0, reqwest 0.13.5, thiserror 2, serde/serde_json.
- Frontend: TypeScript 5.9.3, React 19.3.0, Vite 7.3.7, Vitest 4.1.11, @tauri-apps/api 2.12.3, @tauri-apps/cli 2.12.1, pnpm.

**Spec:** `docs/superpowers/specs/2026-10-10-wanderdex-design.md`

## Global Constraints

- App name **Wanderdex**. Bundle identifier `io.github.wanderdex`. Code license MIT.
- macOS and Windows only. Unsigned builds. Distributed through GitHub Releases. No Linux, code signing or auto-update.
- Sprites and portraits are never committed. They download at runtime from `https://raw.githubusercontent.com/PMDCollab/SpriteCollab/<sha>/...`, where `<sha>` is the commit recorded in the bundled index.
- Pixel art only: integer scale (2×, 3× or 4×, default 3×), nearest-neighbor only, no smoothing, no fractional sprite scaling.
- No frame sizes, durations or row counts hardcoded. Everything comes from each variant's `AnimData.xml`. Row count = sheet height / FrameHeight.
- Direction rows: 0 Down, 1 DownRight, 2 Right, 3 UpRight, 4 Up, 5 UpLeft, 6 Left, 7 DownLeft. Single-row animations always use row 0.
- Durations are ticks at 60 per second.
- Only variants with both `Walk` and `Idle` are listed or spawnable.
- Behavior weights: Walk 50, Idle 30, Flourish 15, Nap 5. Idle lasts 2–8 s, Nap 20–60 s.
- Flourish preference list: `Pose`, `Hop`, `Rotate`, `Charge`, `Double`, `Swing`, `Attack`. Signature overrides: `{"0202": "Withdraw"}`.
- Attribution: credits for each variant come from tracker `sprite_credit`, with names resolved through `credit_names.txt`. Sprite license CC BY-NC 4.0. Non-affiliation notice: Pokémon is © Nintendo / Creatures Inc. / GAME FREAK inc.
- Never hold a `Mutex` guard across an `.await`.
- Never call a blocking Tauri getter (`cursor_position`, `available_monitors`, `inner_size`, …) while holding the `pets` lock. Window commands are `async`, so they run off the main thread. While holding the lock, the 60 Hz loop only calls fire-and-forget setters (`set_position`, `set_size`, `show`, `emit_to`).

## Review Focus

1. **Monitors or the Dock/taskbar change while pets are out** (monitor unplugged, Dock resized, pet dropped above every monitor). Pets re-ground on the new bottom edge. Pets whose monitor disappeared move to the primary monitor, and are never stranded off-screen. → Tests in Task 6 (`ground_for` fallbacks) and Task 7 (`tick` re-grounds and clamps).
2. **Variants with few animations** (only Walk + Idle, single-row Sleep, no Hop). No panic, no stuck pet, and missing behaviors are skipped. → Task 7 tests `minimal_set_never_panics_and_stays_in_range` and `drop_without_hop_goes_idle`.
3. **Interrupted or corrupt downloads** (AnimData present but a sheet missing, a truncated PNG, a network error). Detected, deleted and retried once, then a clear error. → Task 4 tests.
4. **Corrupt settings file, or a saved pet whose variant left the index after an update.** The app starts with defaults and skips unknown pets. → Task 6 tests.
5. **A sprite wider than the screen's usable width** (huge Pokémon at 4× on a small screen). The pet is centered with no NaN and no jittering. → Task 7 test `pet_wider_than_ground_is_centered`.

---

## File Structure

```
package.json, pnpm-lock.yaml, tsconfig.json, vite.config.ts
index.html            picker window entry
pet.html              pet window entry
credits.html          credits window entry
contact.html          contact-sheet dev window entry
src/shared/types.ts       TS mirrors of Rust payloads
src/shared/player.ts      tick → frame column; Player (restart on anim/seq change)
src/shared/renderer.ts    frame source/dest rect math, canvas setup, drawFrame
src/shared/sheets.ts      load sheet images from the cache dir via asset protocol
src/shared/*.test.ts      Vitest tests
src/pet/main.ts           pet window: draw loop + pointer events → Rust
src/picker/main.tsx, App.tsx, api.ts, SpeciesGrid.tsx, DetailPanel.tsx, Preview.tsx, DesktopStrip.tsx, picker.css
src/credits/main.tsx      credits & license window
src/contact/main.ts       contact-sheet render + crispness check
scripts/update-index.sh   fetch tracker + credit names at a SHA → build_index → resources/index.json
src-tauri/Cargo.toml, build.rs, tauri.conf.json, capabilities/default.json, icons/
src-tauri/resources/index.json      generated, committed
src-tauri/resources/overrides.json  signature moves
src-tauri/src/main.rs        calls wanderdex_lib::run()
src-tauri/src/lib.rs         module wiring + app setup
src-tauri/src/animdata.rs    parse/validate AnimData.xml
src-tauri/src/index.rs       index types, build(), bundled(), lookups, overrides
src-tauri/src/bin/build_index.rs
src-tauri/src/bounds.rs      PNG alpha decode + content boxes
src-tauri/src/sprites.rs     download/cache/validate variants and portraits
src-tauri/src/brain.rs       per-pet behavior state machine (pure)
src-tauri/src/ground.rs      monitors → Ground (pure)
src-tauri/src/persist.rs     settings load/save (pure-ish)
src-tauri/src/pets.rs        AppState, pet windows, 60 Hz loop
src-tauri/src/commands.rs    Tauri commands
src-tauri/src/tray.rs        tray menu, picker/credits windows, menu routing
.github/workflows/ci.yml, release.yml
README.md, LICENSE, CREDITS.md
```

---

### Task 1: Scaffold Wanderdex

**Files:**
- Create: `package.json`, `tsconfig.json`, `vite.config.ts`, `index.html`, `pet.html`, `credits.html`, `contact.html`, `src/picker/main.tsx`, `src/pet/main.ts`, `src/credits/main.tsx`, `src/contact/main.ts`, `src/shared/types.ts`, `src/shared/smoke.test.ts`
- Create: `src-tauri/Cargo.toml`, `src-tauri/build.rs`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`, `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`, `src-tauri/icons/*` (copied from the official template)
- Create: `.github/workflows/ci.yml`
- Modify: `.gitignore`

**Interfaces:**
- Produces: library crate `wanderdex_lib` with `pub fn run()`; binary `wanderdex`; a Vite multi-page build with entries `index.html` (picker), `pet.html`, `credits.html`, `contact.html`; `pnpm test`, `pnpm typecheck`, `pnpm build`.

- [ ] **Step 1: Copy the official template icons**

```bash
TMP=$(mktemp -d)
cd "$TMP" && pnpm create tauri-app@4.7.5 tmpl --template react-ts --manager pnpm --yes
mkdir -p /Users/quinn/dev/wobbuffet/src-tauri
cp -R "$TMP/tmpl/src-tauri/icons" /Users/quinn/dev/wobbuffet/src-tauri/icons
rm -rf "$TMP"
```
Expected: `src-tauri/icons/` contains `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.icns`, `icon.ico`, `icon.png`. (If `create-tauri-app` rejects `--yes`, run it interactively choosing TypeScript/pnpm/React/TypeScript. Only the icons are used.)

- [ ] **Step 2: Write `package.json` and install pinned dependencies**

```json
{
  "name": "wanderdex",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc --noEmit && vite build",
    "typecheck": "tsc --noEmit",
    "test": "vitest run",
    "tauri": "tauri",
    "contact-sheet": "tauri build --debug --no-bundle && ./src-tauri/target/debug/wanderdex --contact"
  }
}
```

```bash
cd /Users/quinn/dev/wobbuffet
pnpm add react@19.3.0 react-dom@19.3.0 @tauri-apps/api@2.12.3
pnpm add -D typescript@5.9.3 vite@7.3.7 vitest@4.1.11 @vitejs/plugin-react@5.2.0 @tauri-apps/cli@2.12.1 @types/react@19.3.0 @types/react-dom@19.3.0
```

- [ ] **Step 3: Write `tsconfig.json` and `vite.config.ts`**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "moduleResolution": "bundler",
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "skipLibCheck": true,
    "noEmit": true,
    "types": ["vite/client"]
  },
  "include": ["src"]
}
```

```ts
// vite.config.ts
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import { fileURLToPath } from "node:url";

const page = (file: string) => fileURLToPath(new URL(file, import.meta.url));

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: {
    target: "es2022",
    rollupOptions: {
      input: {
        picker: page("./index.html"),
        pet: page("./pet.html"),
        credits: page("./credits.html"),
        contact: page("./contact.html"),
      },
    },
  },
  test: { environment: "node" },
});
```

- [ ] **Step 4: Write the four HTML entries and stub scripts**

`index.html`:
```html
<!doctype html>
<html lang="en">
  <head><meta charset="UTF-8" /><title>Wanderdex</title></head>
  <body><div id="root"></div><script type="module" src="/src/picker/main.tsx"></script></body>
</html>
```
`pet.html` (transparent background is required for the pet window):
```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <style>html,body{margin:0;padding:0;background:transparent;overflow:hidden;user-select:none;-webkit-user-select:none}canvas{display:block;image-rendering:pixelated}</style>
  </head>
  <body><canvas id="pet"></canvas><script type="module" src="/src/pet/main.ts"></script></body>
</html>
```
`credits.html`:
```html
<!doctype html>
<html lang="en">
  <head><meta charset="UTF-8" /><title>Wanderdex Credits</title></head>
  <body><div id="root"></div><script type="module" src="/src/credits/main.tsx"></script></body>
</html>
```
`contact.html`:
```html
<!doctype html>
<html lang="en">
  <head><meta charset="UTF-8" /><title>Contact sheet</title></head>
  <body style="margin:0;background:#fff"><pre id="report"></pre><canvas id="sheet"></canvas><script type="module" src="/src/contact/main.ts"></script></body>
</html>
```
Stubs (replaced in later tasks):
```tsx
// src/picker/main.tsx
import { createRoot } from "react-dom/client";
createRoot(document.getElementById("root")!).render(<h1>Wanderdex</h1>);
```
```tsx
// src/credits/main.tsx
import { createRoot } from "react-dom/client";
createRoot(document.getElementById("root")!).render(<h1>Credits</h1>);
```
```ts
// src/pet/main.ts
export {};
```
```ts
// src/contact/main.ts
export {};
```
```ts
// src/shared/types.ts
export {};
```

- [ ] **Step 5: Write a smoke test and run the frontend checks**

```ts
// src/shared/smoke.test.ts
import { expect, test } from "vitest";
test("vitest runs", () => expect(1 + 1).toBe(2));
```
Run: `pnpm test && pnpm typecheck && pnpm build`
Expected: 1 test passes, no type errors, `dist/` contains `index.html`, `pet.html`, `credits.html`, `contact.html`.

- [ ] **Step 6: Write the Rust crate**

`src-tauri/Cargo.toml`:
```toml
[package]
name = "wanderdex"
version = "0.1.0"
edition = "2021"
license = "MIT"
default-run = "wanderdex"

[lib]
name = "wanderdex_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { version = "2.7.2", features = [] }

[dependencies]
tauri = { version = "2.12.3", features = ["tray-icon", "macos-private-api", "protocol-asset", "image-png"] }
tauri-plugin-autostart = "2.7.0"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
roxmltree = "0.21.1"
png = "0.18.1"
rand = "0.10.3"
rand_chacha = "0.10.0"
reqwest = "0.13.5"
thiserror = "2"

[dev-dependencies]
tempfile = "3"
```
`src-tauri/build.rs`:
```rust
fn main() {
    tauri_build::build()
}
```
`src-tauri/src/main.rs`:
```rust
// Prevents an extra console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    wanderdex_lib::run()
}
```
`src-tauri/src/lib.rs`:
```rust
use tauri::RunEvent;

pub fn run() {
    tauri::Builder::default()
        .build(tauri::generate_context!())
        .expect("error while building Wanderdex")
        .run(|_app, event| {
            // Wanderdex lives in the tray: closing the last window must not quit.
            if let RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
```
`src-tauri/tauri.conf.json`:
```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Wanderdex",
  "version": "0.1.0",
  "identifier": "io.github.wanderdex",
  "build": {
    "beforeDevCommand": "pnpm dev",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "pnpm build",
    "frontendDist": "../dist"
  },
  "app": {
    "macOSPrivateApi": true,
    "windows": [],
    "security": {
      "csp": null,
      "assetProtocol": { "enable": true, "scope": ["$APPDATA/**"] }
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "category": "Entertainment",
    "shortDescription": "Pokémon that wander your desktop",
    "license": "MIT",
    "icon": ["icons/32x32.png", "icons/128x128.png", "icons/128x128@2x.png", "icons/icon.icns", "icons/icon.ico"]
  }
}
```
`src-tauri/capabilities/default.json`:
```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Permissions for all Wanderdex windows",
  "windows": ["picker", "credits", "contact", "pet-*"],
  "permissions": ["core:default"]
}
```

- [ ] **Step 7: Build and test the Rust crate**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: compiles; `0 passed` (no tests yet), exit code 0.

- [ ] **Step 8: Add CI and update `.gitignore`**

Append to `.gitignore`:
```
/src-tauri/target
/src-tauri/gen
/contact-sheet.png
```
`.github/workflows/ci.yml`:
```yaml
name: CI
on:
  push:
    branches: [main]
  pull_request:
jobs:
  test:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - uses: pnpm/action-setup@v4
        with: { version: 10 }
      - uses: actions/setup-node@v4
        with: { node-version: 22, cache: pnpm }
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
        with: { workspaces: src-tauri }
      - run: pnpm install --frozen-lockfile
      - run: pnpm typecheck
      - run: pnpm test
      - run: pnpm build
      - run: cargo test --manifest-path src-tauri/Cargo.toml
```

- [ ] **Step 9: Commit**

```bash
git add package.json pnpm-lock.yaml tsconfig.json vite.config.ts *.html src src-tauri .github .gitignore
git commit -m "Scaffold Wanderdex (Tauri 2 + React + Vite)"
```

---

### Task 2: Parse and validate AnimData.xml

**Files:**
- Create: `src-tauri/src/animdata.rs`
- Modify: `src-tauri/src/lib.rs` (add `pub mod animdata;`)

**Interfaces:**
- Produces:
  - `pub struct AnimInfo { pub name: String, pub source: String, pub frame_width: u32, pub frame_height: u32, pub durations: Vec<u32>, pub rows: u32 }`, serialized camelCase. `rows` is 0 until `validate`.
  - `impl AnimInfo { pub fn total_secs(&self) -> f64 }`
  - `pub struct AnimTable { pub anims: Vec<AnimInfo> }`
  - `impl AnimTable { pub fn get(&self, name: &str) -> Option<&AnimInfo>; pub fn sources(&self) -> BTreeSet<String>; pub fn validate(&mut self, sheet_size: impl Fn(&str) -> Option<(u32, u32)>) -> Result<(), AnimError> }`
  - `pub fn parse(xml: &str) -> Result<AnimTable, AnimError>`
  - `pub enum AnimError` (Display via thiserror)

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/animdata.rs` containing only the test module. The test XML uses Wobbuffet's real structure and numbers.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Structure and numbers from Wobbuffet (0202): a CopyOf entry, a single-row sheet, varied durations.
    pub const WOBBUFFET_XML: &str = r#"<?xml version="1.0" ?>
<AnimData><ShadowSize>1</ShadowSize><Anims>
  <Anim><Name>Walk</Name><Index>0</Index><FrameWidth>24</FrameWidth><FrameHeight>32</FrameHeight>
    <Durations><Duration>8</Duration><Duration>10</Duration><Duration>8</Duration><Duration>10</Duration></Durations></Anim>
  <Anim><Name>Attack</Name><Index>1</Index><FrameWidth>64</FrameWidth><FrameHeight>72</FrameHeight>
    <Durations><Duration>2</Duration><Duration>4</Duration><Duration>1</Duration><Duration>1</Duration><Duration>1</Duration>
    <Duration>2</Duration><Duration>2</Duration><Duration>2</Duration><Duration>2</Duration><Duration>2</Duration></Durations></Anim>
  <Anim><Name>Strike</Name><Index>2</Index><CopyOf>Attack</CopyOf></Anim>
  <Anim><Name>Sleep</Name><Index>5</Index><FrameWidth>24</FrameWidth><FrameHeight>40</FrameHeight>
    <Durations><Duration>30</Duration><Duration>35</Duration></Durations></Anim>
  <Anim><Name>Idle</Name><Index>7</Index><FrameWidth>24</FrameWidth><FrameHeight>40</FrameHeight>
    <Durations><Duration>40</Duration><Duration>4</Duration><Duration>4</Duration><Duration>4</Duration><Duration>4</Duration>
    <Duration>4</Duration><Duration>4</Duration><Duration>4</Duration><Duration>4</Duration></Durations></Anim>
</Anims></AnimData>"#;

    pub fn wobbuffet_sizes(source: &str) -> Option<(u32, u32)> {
        match source {
            "Walk" => Some((96, 256)),
            "Attack" => Some((640, 576)),
            "Sleep" => Some((48, 40)),
            "Idle" => Some((216, 320)),
            _ => None,
        }
    }

    #[test]
    fn parses_in_file_order_and_resolves_copy_of() {
        let t = parse(WOBBUFFET_XML).unwrap();
        let names: Vec<&str> = t.anims.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["Walk", "Attack", "Strike", "Sleep", "Idle"]);
        let strike = t.get("Strike").unwrap();
        assert_eq!(strike.source, "Attack");
        assert_eq!((strike.frame_width, strike.frame_height), (64, 72));
        assert_eq!(strike.durations.len(), 10);
        assert_eq!(t.sources().into_iter().collect::<Vec<_>>(), ["Attack", "Idle", "Sleep", "Walk"]);
    }

    #[test]
    fn total_secs_uses_60_ticks_per_second() {
        let t = parse(WOBBUFFET_XML).unwrap();
        assert!((t.get("Idle").unwrap().total_secs() - 72.0 / 60.0).abs() < 1e-9);
    }

    #[test]
    fn validate_derives_rows_from_sheet_height() {
        let mut t = parse(WOBBUFFET_XML).unwrap();
        t.validate(wobbuffet_sizes).unwrap();
        assert_eq!(t.get("Idle").unwrap().rows, 8);
        assert_eq!(t.get("Sleep").unwrap().rows, 1);
        assert_eq!(t.get("Strike").unwrap().rows, 8);
    }

    #[test]
    fn validate_rejects_wrong_sheet_width() {
        let mut t = parse(WOBBUFFET_XML).unwrap();
        let err = t
            .validate(|s| if s == "Walk" { Some((100, 256)) } else { wobbuffet_sizes(s) })
            .unwrap_err();
        assert!(matches!(err, AnimError::SheetSize { .. }), "{err}");
    }

    #[test]
    fn validate_rejects_missing_sheet() {
        let mut t = parse(WOBBUFFET_XML).unwrap();
        let err = t.validate(|s| if s == "Idle" { None } else { wobbuffet_sizes(s) }).unwrap_err();
        assert_eq!(err, AnimError::MissingSheet("Idle".into()));
    }

    #[test]
    fn rejects_copy_of_cycles_and_dangling_targets() {
        let cycle = r#"<AnimData><Anims>
            <Anim><Name>A</Name><CopyOf>B</CopyOf></Anim>
            <Anim><Name>B</Name><CopyOf>A</CopyOf></Anim></Anims></AnimData>"#;
        assert!(matches!(parse(cycle), Err(AnimError::BadCopyOf(_))));
        let dangling = r#"<AnimData><Anims><Anim><Name>A</Name><CopyOf>Nope</CopyOf></Anim></Anims></AnimData>"#;
        assert!(matches!(parse(dangling), Err(AnimError::BadCopyOf(_))));
    }

    #[test]
    fn rejects_invalid_xml_and_missing_fields() {
        assert!(matches!(parse("<AnimData><Anims>"), Err(AnimError::Xml(_))));
        let no_width = r#"<AnimData><Anims><Anim><Name>A</Name><FrameHeight>8</FrameHeight>
            <Durations><Duration>1</Duration></Durations></Anim></Anims></AnimData>"#;
        assert!(matches!(parse(no_width), Err(AnimError::MissingField(_, "FrameWidth"))));
        let no_durations = r#"<AnimData><Anims><Anim><Name>A</Name><FrameWidth>8</FrameWidth>
            <FrameHeight>8</FrameHeight></Anim></Anims></AnimData>"#;
        assert!(matches!(parse(no_durations), Err(AnimError::NoDurations(_))));
    }
}
```
Add `pub mod animdata;` to the top of `src-tauri/src/lib.rs`.

- [ ] **Step 2: Run the tests to confirm they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml animdata`
Expected: compile errors (`cannot find function parse`, `AnimError` not found).

- [ ] **Step 3: Implement the module**

Insert above the test module in `src-tauri/src/animdata.rs`:
```rust
//! Parses PMD SpriteCollab AnimData.xml into a typed table.

use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AnimInfo {
    pub name: String,
    /// Sheet name after following CopyOf (`<source>-Anim.png`).
    pub source: String,
    pub frame_width: u32,
    pub frame_height: u32,
    /// Ticks per frame, 60 per second.
    pub durations: Vec<u32>,
    /// Direction rows; filled in by `AnimTable::validate`.
    pub rows: u32,
}

impl AnimInfo {
    pub fn total_secs(&self) -> f64 {
        self.durations.iter().sum::<u32>() as f64 / 60.0
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AnimTable {
    pub anims: Vec<AnimInfo>,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum AnimError {
    #[error("AnimData.xml is not valid XML: {0}")]
    Xml(String),
    #[error("<Anim> without <Name>")]
    MissingName,
    #[error("{0}: bad CopyOf chain")]
    BadCopyOf(String),
    #[error("{0}: missing or invalid {1}")]
    MissingField(String, &'static str),
    #[error("{0}: no Durations")]
    NoDurations(String),
    #[error("missing sheet {0}-Anim.png")]
    MissingSheet(String),
    #[error("{name}: sheet is {width}x{height}, expected {columns} columns of {frame_width}x{frame_height}")]
    SheetSize { name: String, width: u32, height: u32, columns: u32, frame_width: u32, frame_height: u32 },
}

fn child_text(node: roxmltree::Node, tag: &str) -> Option<String> {
    node.children()
        .find(|c| c.has_tag_name(tag))
        .and_then(|c| c.text())
        .map(|t| t.trim().to_string())
}

pub fn parse(xml: &str) -> Result<AnimTable, AnimError> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| AnimError::Xml(e.to_string()))?;
    let mut raw: Vec<(String, roxmltree::Node)> = Vec::new();
    for node in doc.descendants().filter(|n| n.has_tag_name("Anim")) {
        let name = child_text(node, "Name").ok_or(AnimError::MissingName)?;
        raw.push((name, node));
    }
    let find = |name: &str| raw.iter().find(|(n, _)| n == name).map(|(_, node)| *node);

    let mut anims = Vec::new();
    for (name, _) in &raw {
        let mut source = name.clone();
        let mut seen = BTreeSet::from([name.clone()]);
        while let Some(target) = find(&source).and_then(|n| child_text(n, "CopyOf")) {
            if find(&target).is_none() || !seen.insert(target.clone()) {
                return Err(AnimError::BadCopyOf(name.clone()));
            }
            source = target;
        }
        let node = find(&source).expect("source exists");
        let number = |tag: &'static str| {
            child_text(node, tag)
                .and_then(|t| t.parse::<u32>().ok())
                .filter(|v| *v > 0)
                .ok_or_else(|| AnimError::MissingField(source.clone(), tag))
        };
        let frame_width = number("FrameWidth")?;
        let frame_height = number("FrameHeight")?;
        let durations: Vec<u32> = node
            .children()
            .find(|c| c.has_tag_name("Durations"))
            .map(|d| {
                d.children()
                    .filter(|c| c.has_tag_name("Duration"))
                    .filter_map(|c| c.text()?.trim().parse().ok())
                    .collect()
            })
            .unwrap_or_default();
        if durations.is_empty() {
            return Err(AnimError::NoDurations(source));
        }
        anims.push(AnimInfo { name: name.clone(), source, frame_width, frame_height, durations, rows: 0 });
    }
    Ok(AnimTable { anims })
}

impl AnimTable {
    pub fn get(&self, name: &str) -> Option<&AnimInfo> {
        self.anims.iter().find(|a| a.name == name)
    }

    pub fn sources(&self) -> BTreeSet<String> {
        self.anims.iter().map(|a| a.source.clone()).collect()
    }

    /// Checks every sheet against its frame size and fills in `rows`.
    pub fn validate(&mut self, sheet_size: impl Fn(&str) -> Option<(u32, u32)>) -> Result<(), AnimError> {
        for a in &mut self.anims {
            let (width, height) = sheet_size(&a.source).ok_or_else(|| AnimError::MissingSheet(a.source.clone()))?;
            let columns = a.durations.len() as u32;
            if width != a.frame_width * columns || height == 0 || height % a.frame_height != 0 {
                return Err(AnimError::SheetSize {
                    name: a.source.clone(),
                    width,
                    height,
                    columns,
                    frame_width: a.frame_width,
                    frame_height: a.frame_height,
                });
            }
            a.rows = height / a.frame_height;
        }
        Ok(())
    }
}
```

- [ ] **Step 4: Run the tests to confirm they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml animdata`
Expected: 7 passed.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/animdata.rs src-tauri/src/lib.rs
git commit -m "Parse and validate AnimData.xml"
```

---

### Task 3: Build the bundled sprite index

**Files:**
- Create: `src-tauri/src/index.rs`, `src-tauri/src/bin/build_index.rs`, `src-tauri/resources/overrides.json`, `scripts/update-index.sh`
- Create (generated): `src-tauri/resources/index.json`
- Modify: `src-tauri/src/lib.rs` (add `pub mod index;`)

**Interfaces:**
- Produces:
  - `pub struct Index { pub sprite_collab_sha: String, pub species: Vec<Species> }`
  - `pub struct Species { pub dex: String, pub name: String, pub has_portrait: bool, pub variants: Vec<Variant> }`
  - `pub struct Variant { pub path: String, pub form_id: String, pub form: String, pub shiny: bool, pub female: bool, pub anims: Vec<String>, pub credits: Vec<String> }`
  - All serialized camelCase, all `Clone + Serialize + Deserialize + PartialEq + Debug`.
  - `pub fn build(tracker: &serde_json::Value, credit_names: &str, sha: &str) -> Index`
  - `pub fn bundled() -> &'static Index`
  - `pub fn find_variant(index: &Index, path: &str) -> Option<(&Species, &Variant)>`
  - `pub fn signature_for(dex: &str) -> Option<&'static str>`

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/index.rs` with only the tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const CREDIT_NAMES: &str = "Name\tDiscord\tContact\nCHUNSOFT\tCHUNSOFT\thttps://www.spike-chunsoft.com/\nTacocoa\t<@!234720295292895232>\t\n";

    fn files(names: &[&str]) -> serde_json::Value {
        let mut m = serde_json::Map::new();
        for n in names { m.insert((*n).into(), json!(true)); }
        m.insert("Hurt".into(), json!(false)); // false entries are not available
        serde_json::Value::Object(m)
    }

    fn tracker() -> serde_json::Value {
        json!({
            "0000": { "name": "Missingno_", "sprite_files": files(&["Walk", "Idle"]), "subgroups": {} },
            "0202": {
                "name": "Wobbuffet",
                "portrait_files": { "Normal": true },
                "sprite_files": files(&["Walk", "Idle", "Withdraw"]),
                "sprite_credit": { "primary": "CHUNSOFT", "secondary": [] },
                "subgroups": {
                    "0000": { "name": "", "sprite_files": {}, "subgroups": {
                        "0000": { "name": "", "sprite_files": {}, "subgroups": {
                            "0002": { "name": "Female", "sprite_files": files(&["Walk", "Idle"]),
                                      "sprite_credit": { "primary": "CHUNSOFT", "secondary": [] }, "subgroups": {} } } },
                        "0001": { "name": "Shiny", "sprite_files": files(&["Walk", "Idle"]),
                                  "sprite_credit": { "primary": "<@!234720295292895232>", "secondary": ["SomeoneNew"] }, "subgroups": {} } } }
                }
            },
            "0019": {
                "name": "Rattata",
                "portrait_files": { "Normal": false },
                "sprite_files": files(&["Walk", "Idle"]),
                "sprite_credit": { "primary": "", "secondary": [] },
                "subgroups": {
                    "0001": { "name": "Alola", "sprite_files": files(&["Walk", "Idle"]),
                              "sprite_credit": { "primary": "CHUNSOFT", "secondary": [] }, "subgroups": {
                        "0001": { "name": "Shiny", "sprite_files": files(&["Walk", "Idle"]),
                                  "sprite_credit": { "primary": "CHUNSOFT", "secondary": [] }, "subgroups": {} } } },
                    "0003": { "name": "Gigantamax", "sprite_files": files(&["Idle"]), "subgroups": {} }
                }
            },
            "0151": { "name": "Mew", "sprite_files": files(&["Idle"]), "subgroups": {} }
        })
    }

    #[test]
    fn builds_variants_with_paths_flags_and_form_names() {
        let idx = build(&tracker(), CREDIT_NAMES, "abc123");
        assert_eq!(idx.sprite_collab_sha, "abc123");
        let dexes: Vec<&str> = idx.species.iter().map(|s| s.dex.as_str()).collect();
        assert_eq!(dexes, ["0019", "0202"], "sorted, skips 0000 and species without usable variants");

        let wob = &idx.species[1];
        assert_eq!(wob.name, "Wobbuffet");
        assert!(wob.has_portrait);
        let summary: Vec<(&str, &str, bool, bool)> =
            wob.variants.iter().map(|v| (v.path.as_str(), v.form.as_str(), v.shiny, v.female)).collect();
        assert_eq!(summary, [
            ("0202", "Normal", false, false),
            ("0202/0000/0000/0002", "Normal", false, true),
            ("0202/0000/0001", "Normal", true, false),
        ]);
        assert_eq!(wob.variants[0].anims, ["Idle", "Walk", "Withdraw"]);

        let rat = &idx.species[0];
        assert!(!rat.has_portrait);
        let paths: Vec<(&str, &str, &str, bool)> =
            rat.variants.iter().map(|v| (v.path.as_str(), v.form_id.as_str(), v.form.as_str(), v.shiny)).collect();
        assert_eq!(paths, [
            ("0019", "0000", "Normal", false),
            ("0019/0001", "0001", "Alola", false),
            ("0019/0001/0001", "0001", "Alola", true),
        ], "Gigantamax has no Walk and is excluded");
    }

    #[test]
    fn resolves_credit_names_and_keeps_unknown_names() {
        let idx = build(&tracker(), CREDIT_NAMES, "abc123");
        let wob = &idx.species[1];
        assert_eq!(wob.variants[0].credits, ["CHUNSOFT"]);
        assert_eq!(wob.variants[2].credits, ["Tacocoa", "SomeoneNew"]);
        assert!(idx.species[0].variants[0].credits.is_empty(), "empty primary is dropped");
    }

    #[test]
    fn finds_variants_by_path() {
        let idx = build(&tracker(), CREDIT_NAMES, "abc123");
        let (s, v) = find_variant(&idx, "0202/0000/0001").unwrap();
        assert_eq!((s.name.as_str(), v.shiny), ("Wobbuffet", true));
        assert!(find_variant(&idx, "9999").is_none());
    }

    #[test]
    fn signature_overrides_load() {
        assert_eq!(signature_for("0202"), Some("Withdraw"));
        assert_eq!(signature_for("0025"), None);
    }

    #[test]
    fn bundled_index_is_real_and_pinned() {
        let idx = bundled();
        assert_eq!(idx.sprite_collab_sha.len(), 40, "pinned to a full commit SHA");
        assert!(idx.species.len() > 900);
        let (_, wob) = find_variant(idx, "0202").expect("Wobbuffet present");
        assert!(wob.anims.iter().any(|a| a == "Walk") && wob.anims.iter().any(|a| a == "Idle"));
        for s in &idx.species {
            for v in &s.variants {
                assert!(v.anims.iter().any(|a| a == "Walk") && v.anims.iter().any(|a| a == "Idle"), "{}", v.path);
            }
        }
    }
}
```
Add `pub mod index;` to `src-tauri/src/lib.rs`. Create `src-tauri/resources/overrides.json`:
```json
{ "signatures": { "0202": "Withdraw" } }
```
Create a placeholder index so the crate compiles: `echo '{"spriteCollabSha":"","species":[]}' > src-tauri/resources/index.json`

- [ ] **Step 2: Run the tests to confirm they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml index::`
Expected: compile errors (`build`, `find_variant`, `signature_for`, `bundled` not found).

- [ ] **Step 3: Implement the index module**

Insert above the tests in `src-tauri/src/index.rs`:
```rust
//! The bundled SpriteCollab index: which species/variants exist, their animations and credits.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::LazyLock;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Index {
    pub sprite_collab_sha: String,
    pub species: Vec<Species>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Species {
    pub dex: String,
    pub name: String,
    pub has_portrait: bool,
    pub variants: Vec<Variant>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Variant {
    /// SpriteCollab path under `sprite/`, e.g. `0202/0000/0001`.
    pub path: String,
    pub form_id: String,
    pub form: String,
    pub shiny: bool,
    pub female: bool,
    pub anims: Vec<String>,
    /// Primary artist first, then secondary.
    pub credits: Vec<String>,
}

fn parse_credit_names(text: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in text.lines().skip(1) {
        let cols: Vec<&str> = line.split('\t').collect();
        let Some(name) = cols.first().map(|s| s.trim()).filter(|s| !s.is_empty()) else { continue };
        map.insert(name.to_string(), name.to_string());
        if let Some(discord) = cols.get(1).map(|s| s.trim()).filter(|s| !s.is_empty()) {
            map.insert(discord.to_string(), name.to_string());
        }
    }
    map
}

fn resolve(token: &str, names: &HashMap<String, String>) -> String {
    names.get(token).cloned().unwrap_or_else(|| token.to_string())
}

fn collect<'a>(
    node: &'a Value,
    dex: &str,
    segs: &[String],
    form_name: Option<&'a str>,
    names: &HashMap<String, String>,
    out: &mut Vec<Variant>,
) {
    let form_name = if segs.len() == 1 { node["name"].as_str() } else { form_name };
    let files = node["sprite_files"].as_object();
    let has = |k: &str| files.and_then(|f| f.get(k)).and_then(Value::as_bool).unwrap_or(false);
    if has("Walk") && has("Idle") {
        let form_id = segs.first().cloned().unwrap_or_else(|| "0000".to_string());
        let form = if form_id == "0000" {
            "Normal".to_string()
        } else {
            match form_name.filter(|n| !n.is_empty()) {
                Some(n) => n.replace('_', " "),
                None => format!("Form {form_id}"),
            }
        };
        let mut anims: Vec<String> = files
            .into_iter()
            .flatten()
            .filter(|(_, v)| v.as_bool() == Some(true))
            .map(|(k, _)| k.clone())
            .collect();
        anims.sort();
        let credit = &node["sprite_credit"];
        let mut credits = Vec::new();
        if let Some(p) = credit["primary"].as_str().filter(|p| !p.is_empty()) {
            credits.push(resolve(p, names));
        }
        for s in credit["secondary"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            credits.push(resolve(s, names));
        }
        let mut path = dex.to_string();
        for s in segs {
            path.push('/');
            path.push_str(s);
        }
        out.push(Variant {
            path,
            form_id,
            form,
            shiny: segs.get(1).is_some_and(|s| s == "0001"),
            female: segs.get(2).is_some_and(|s| s == "0002"),
            anims,
            credits,
        });
    }
    if let Some(subs) = node["subgroups"].as_object() {
        let mut keys: Vec<&String> = subs.keys().collect();
        keys.sort();
        for k in keys {
            let mut next = segs.to_vec();
            next.push(k.clone());
            collect(&subs[k], dex, &next, form_name, names, out);
        }
    }
}

/// Builds the index from SpriteCollab's tracker.json and credit_names.txt.
pub fn build(tracker: &Value, credit_names: &str, sha: &str) -> Index {
    let names = parse_credit_names(credit_names);
    let mut species = Vec::new();
    if let Some(obj) = tracker.as_object() {
        let mut dexes: Vec<&String> = obj.keys().collect();
        dexes.sort();
        for dex in dexes.into_iter().filter(|d| d.as_str() != "0000") {
            let entry = &obj[dex];
            let mut variants = Vec::new();
            collect(entry, dex, &[], None, &names, &mut variants);
            if variants.is_empty() {
                continue;
            }
            species.push(Species {
                dex: dex.clone(),
                name: entry["name"].as_str().unwrap_or("").replace('_', " "),
                has_portrait: entry["portrait_files"]["Normal"].as_bool().unwrap_or(false),
                variants,
            });
        }
    }
    Index { sprite_collab_sha: sha.to_string(), species }
}

static BUNDLED: LazyLock<Index> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../resources/index.json")).expect("bundled index.json is valid")
});

pub fn bundled() -> &'static Index {
    &BUNDLED
}

pub fn find_variant<'a>(index: &'a Index, path: &str) -> Option<(&'a Species, &'a Variant)> {
    let dex = path.split('/').next()?;
    let species = index.species.iter().find(|s| s.dex == dex)?;
    species.variants.iter().find(|v| v.path == path).map(|v| (species, v))
}

#[derive(Deserialize)]
struct Overrides {
    signatures: HashMap<String, String>,
}

static OVERRIDES: LazyLock<Overrides> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../resources/overrides.json")).expect("overrides.json is valid")
});

/// A species' signature move (an animation name), if one is configured.
pub fn signature_for(dex: &str) -> Option<&'static str> {
    OVERRIDES.signatures.get(dex).map(String::as_str)
}
```

- [ ] **Step 4: Run the unit tests (all except the bundled-index test)**

Run: `cargo test --manifest-path src-tauri/Cargo.toml index:: -- --skip bundled_index_is_real_and_pinned`
Expected: 4 passed.

- [ ] **Step 5: Write the build_index binary and the update script**

`src-tauri/src/bin/build_index.rs`:
```rust
//! Usage: build_index <tracker.json> <credit_names.txt> <sha>  (writes index JSON to stdout)
use wanderdex_lib::index;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        eprintln!("usage: build_index <tracker.json> <credit_names.txt> <sha>");
        std::process::exit(2);
    }
    let tracker: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&args[1]).expect("read tracker.json")).expect("parse tracker.json");
    let credit_names = std::fs::read_to_string(&args[2]).expect("read credit_names.txt");
    let idx = index::build(&tracker, &credit_names, &args[3]);
    let variants: usize = idx.species.iter().map(|s| s.variants.len()).sum();
    eprintln!("{} species, {} variants", idx.species.len(), variants);
    println!("{}", serde_json::to_string(&idx).expect("serialize index"));
}
```
`scripts/update-index.sh`:
```bash
#!/usr/bin/env bash
# Regenerates src-tauri/resources/index.json from SpriteCollab at a commit (default: current HEAD).
set -euo pipefail
cd "$(dirname "$0")/.."
SHA="${1:-$(git ls-remote https://github.com/PMDCollab/SpriteCollab.git HEAD | cut -f1)}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
curl -fsSL "https://raw.githubusercontent.com/PMDCollab/SpriteCollab/$SHA/tracker.json" -o "$TMP/tracker.json"
curl -fsSL "https://raw.githubusercontent.com/PMDCollab/SpriteCollab/$SHA/credit_names.txt" -o "$TMP/credit_names.txt"
cargo run --quiet --manifest-path src-tauri/Cargo.toml --bin build_index -- \
  "$TMP/tracker.json" "$TMP/credit_names.txt" "$SHA" > "$TMP/index.json"
mv "$TMP/index.json" src-tauri/resources/index.json
echo "Index updated to SpriteCollab $SHA"
```

- [ ] **Step 6: Generate the real index**

Run: `chmod +x scripts/update-index.sh && ./scripts/update-index.sh`
Expected: stderr shows roughly `9xx species, 3xxx variants`; the script prints `Index updated to SpriteCollab <40-char sha>`; `src-tauri/resources/index.json` is under 2 MB (`ls -lh`).

- [ ] **Step 7: Run all index tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml index::`
Expected: 5 passed.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/index.rs src-tauri/src/bin src-tauri/src/lib.rs src-tauri/resources scripts/update-index.sh
git commit -m "Build bundled SpriteCollab index with credits and signature overrides"
```

---

### Task 4: Download, cache, and validate sprites

**Files:**
- Create: `src-tauri/src/bounds.rs` (PNG decode + content boxes; used here for validation, and by Task 7)
- Create: `src-tauri/src/sprites.rs`
- Modify: `src-tauri/src/lib.rs` (add `pub mod bounds; pub mod sprites;`)

**Interfaces:**
- Consumes: `animdata::{parse, AnimTable}` (Task 2).
- Produces:
  - `bounds::BoxI { pub min_x: i32, pub min_y: i32, pub max_x: i32, pub max_y: i32 }` (camelCase serde, `Copy`), with `width()`, `height()`, `union()`.
  - `bounds::AlphaSheet { pub width: u32, pub height: u32, pub alpha: Vec<u8> }`
  - `bounds::decode_alpha(bytes: &[u8]) -> Result<AlphaSheet, String>`
  - `bounds::frame_bounds(sheet: &AlphaSheet, x0: u32, y0: u32, fw: u32, fh: u32) -> Option<BoxI>`
  - `bounds::content_box(table: &AnimTable, sheets: &HashMap<String, AlphaSheet>, names: &[String]) -> BoxI`
  - `sprites::Fetch` trait: `async fn get(&self, url: &str) -> Result<Vec<u8>, String>`
  - `sprites::HttpFetch::new() -> HttpFetch` (implements `Fetch`)
  - `sprites::LoadedVariant { pub path: String, pub dir: PathBuf, pub table: AnimTable, pub sheets: HashMap<String, AlphaSheet> }`
  - `sprites::SpriteError` (serializes as its message string)
  - `sprites::ensure_variant<F: Fetch>(fetch: &F, cache_root: &Path, sha: &str, path: &str) -> Result<LoadedVariant, SpriteError>`
  - `sprites::ensure_portrait<F: Fetch>(fetch: &F, cache_root: &Path, sha: &str, dex: &str) -> Result<PathBuf, SpriteError>`

- [ ] **Step 1: Write failing tests for `bounds`**

Create `src-tauri/src/bounds.rs` with tests only:
```rust
#[cfg(test)]
pub mod tests {
    use super::*;

    /// RGBA PNG of `w`x`h`; `opaque(x, y)` decides which pixels have alpha 255.
    pub fn png_bytes(w: u32, h: u32, opaque: impl Fn(u32, u32) -> bool) -> Vec<u8> {
        let mut data = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                data.extend_from_slice(&[10, 20, 30, if opaque(x, y) { 255 } else { 0 }]);
            }
        }
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, w, h);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut writer = enc.write_header().unwrap();
            writer.write_image_data(&data).unwrap();
            writer.finish().unwrap();
        }
        out
    }

    #[test]
    fn decodes_alpha_plane() {
        let sheet = decode_alpha(&png_bytes(4, 2, |x, y| x == 1 && y == 1)).unwrap();
        assert_eq!((sheet.width, sheet.height), (4, 2));
        assert_eq!(sheet.alpha, [0, 0, 0, 0, 0, 255, 0, 0]);
    }

    #[test]
    fn rejects_garbage() {
        assert!(decode_alpha(b"not a png").is_err());
        let mut truncated = png_bytes(8, 8, |_, _| true);
        truncated.truncate(truncated.len() / 2);
        assert!(decode_alpha(&truncated).is_err());
    }

    #[test]
    fn frame_bounds_are_relative_to_cell_center() {
        // One 8x8 cell; opaque block at x 2..5, y 3..8 → relative to center (4,4): x -2..1, y -1..4
        let sheet = decode_alpha(&png_bytes(8, 8, |x, y| (2..5).contains(&x) && (3..8).contains(&y))).unwrap();
        assert_eq!(frame_bounds(&sheet, 0, 0, 8, 8), Some(BoxI { min_x: -2, min_y: -1, max_x: 1, max_y: 4 }));
        let empty = decode_alpha(&png_bytes(8, 8, |_, _| false)).unwrap();
        assert_eq!(frame_bounds(&empty, 0, 0, 8, 8), None);
    }

    #[test]
    fn content_box_unions_frames_rows_and_named_anims_only() {
        // Walk: 2 columns x 2 rows of 8x8. Frame (col 1,row 1) has a pixel at its top-left corner.
        let walk = png_bytes(16, 16, |x, y| (x, y) == (3, 4) || (x, y) == (8, 8));
        let other = png_bytes(8, 8, |_, _| true);
        let mut table = crate::animdata::parse(r#"<AnimData><Anims>
            <Anim><Name>Walk</Name><FrameWidth>8</FrameWidth><FrameHeight>8</FrameHeight><Durations><Duration>1</Duration><Duration>1</Duration></Durations></Anim>
            <Anim><Name>Big</Name><FrameWidth>8</FrameWidth><FrameHeight>8</FrameHeight><Durations><Duration>1</Duration></Durations></Anim>
            </Anims></AnimData>"#).unwrap();
        let mut sheets = HashMap::new();
        sheets.insert("Walk".to_string(), decode_alpha(&walk).unwrap());
        sheets.insert("Big".to_string(), decode_alpha(&other).unwrap());
        table.validate(|s| sheets.get(s).map(|a| (a.width, a.height))).unwrap();
        let b = content_box(&table, &sheets, &["Walk".to_string(), "Missing".to_string()]);
        assert_eq!(b, BoxI { min_x: -4, min_y: -4, max_x: 0, max_y: 1 });
        assert_eq!((b.width(), b.height()), (4, 5));
    }
}
```

- [ ] **Step 2: Run to confirm failure**

Add `pub mod bounds;` to `lib.rs`. Run: `cargo test --manifest-path src-tauri/Cargo.toml bounds`
Expected: compile errors (`decode_alpha`, `BoxI` missing).

- [ ] **Step 3: Implement `bounds`**

Insert above the tests in `src-tauri/src/bounds.rs`:
```rust
//! PNG alpha decoding and opaque-pixel bounds, relative to each frame's center.

use crate::animdata::AnimTable;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoxI {
    pub min_x: i32,
    pub min_y: i32,
    pub max_x: i32, // exclusive
    pub max_y: i32, // exclusive
}

impl BoxI {
    pub fn width(&self) -> i32 { self.max_x - self.min_x }
    pub fn height(&self) -> i32 { self.max_y - self.min_y }
    pub fn union(&self, o: &BoxI) -> BoxI {
        BoxI {
            min_x: self.min_x.min(o.min_x),
            min_y: self.min_y.min(o.min_y),
            max_x: self.max_x.max(o.max_x),
            max_y: self.max_y.max(o.max_y),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AlphaSheet {
    pub width: u32,
    pub height: u32,
    /// One byte per pixel, row-major, top row first.
    pub alpha: Vec<u8>,
}

pub fn decode_alpha(bytes: &[u8]) -> Result<AlphaSheet, String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16 | png::Transformations::ALPHA);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("PNG too large")?];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let channels = match info.color_type {
        png::ColorType::Rgba => 4,
        png::ColorType::GrayscaleAlpha => 2,
        other => return Err(format!("unexpected PNG color type {other:?}")),
    };
    let mut alpha = Vec::with_capacity((info.width * info.height) as usize);
    for y in 0..info.height as usize {
        let row = &buf[y * info.line_size..];
        for x in 0..info.width as usize {
            alpha.push(row[x * channels + channels - 1]);
        }
    }
    Ok(AlphaSheet { width: info.width, height: info.height, alpha })
}

/// Opaque bounds of the cell at (x0, y0), relative to the cell center (fw/2, fh/2).
pub fn frame_bounds(sheet: &AlphaSheet, x0: u32, y0: u32, fw: u32, fh: u32) -> Option<BoxI> {
    let mut b: Option<BoxI> = None;
    let (cx, cy) = ((fw / 2) as i32, (fh / 2) as i32);
    for y in 0..fh {
        for x in 0..fw {
            if sheet.alpha[((y0 + y) * sheet.width + x0 + x) as usize] == 0 {
                continue;
            }
            let p = BoxI { min_x: x as i32 - cx, min_y: y as i32 - cy, max_x: x as i32 - cx + 1, max_y: y as i32 - cy + 1 };
            b = Some(b.map_or(p, |b| b.union(&p)));
        }
    }
    b
}

/// Union of opaque pixels over every frame and row of the named animations that exist.
pub fn content_box(table: &AnimTable, sheets: &HashMap<String, AlphaSheet>, names: &[String]) -> BoxI {
    let mut b: Option<BoxI> = None;
    for name in names {
        let Some(anim) = table.get(name) else { continue };
        let Some(sheet) = sheets.get(&anim.source) else { continue };
        for row in 0..anim.rows {
            for col in 0..anim.durations.len() as u32 {
                if let Some(f) = frame_bounds(sheet, col * anim.frame_width, row * anim.frame_height, anim.frame_width, anim.frame_height) {
                    b = Some(b.map_or(f, |b| b.union(&f)));
                }
            }
        }
    }
    b.unwrap_or(BoxI { min_x: -1, min_y: -1, max_x: 1, max_y: 1 })
}
```

- [ ] **Step 4: Run bounds tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml bounds`
Expected: 4 passed.

- [ ] **Step 5: Write failing tests for `sprites`**

Create `src-tauri/src/sprites.rs` with tests only:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::bounds::tests::png_bytes;
    use std::collections::HashMap;
    use std::sync::Mutex;

    const XML: &str = r#"<AnimData><Anims>
        <Anim><Name>Walk</Name><FrameWidth>8</FrameWidth><FrameHeight>8</FrameHeight><Durations><Duration>4</Duration><Duration>4</Duration></Durations></Anim>
        <Anim><Name>Idle</Name><FrameWidth>8</FrameWidth><FrameHeight>8</FrameHeight><Durations><Duration>30</Duration></Durations></Anim>
        <Anim><Name>Strike</Name><CopyOf>Idle</CopyOf></Anim>
        </Anims></AnimData>"#;

    struct FakeFetch {
        files: Mutex<HashMap<String, Vec<u8>>>,
        calls: Mutex<Vec<String>>,
    }

    impl FakeFetch {
        fn good() -> Self {
            let mut files = HashMap::new();
            let base = "https://raw.githubusercontent.com/PMDCollab/SpriteCollab/sha1/sprite/0202";
            files.insert(format!("{base}/AnimData.xml"), XML.as_bytes().to_vec());
            files.insert(format!("{base}/Walk-Anim.png"), png_bytes(16, 64, |_, _| true));
            files.insert(format!("{base}/Idle-Anim.png"), png_bytes(8, 8, |_, _| true));
            files.insert(
                "https://raw.githubusercontent.com/PMDCollab/SpriteCollab/sha1/portrait/0202/Normal.png".into(),
                png_bytes(40, 40, |_, _| true),
            );
            FakeFetch { files: Mutex::new(files), calls: Mutex::new(Vec::new()) }
        }
        fn calls(&self) -> Vec<String> { self.calls.lock().unwrap().clone() }
    }

    impl Fetch for FakeFetch {
        async fn get(&self, url: &str) -> Result<Vec<u8>, String> {
            self.calls.lock().unwrap().push(url.to_string());
            self.files.lock().unwrap().get(url).cloned().ok_or_else(|| format!("404 {url}"))
        }
    }

    fn run<T>(f: impl std::future::Future<Output = T>) -> T { tauri::async_runtime::block_on(f) }

    #[test]
    fn downloads_each_source_sheet_once_and_validates() {
        let cache = tempfile::tempdir().unwrap();
        let fetch = FakeFetch::good();
        let v = run(ensure_variant(&fetch, cache.path(), "sha1", "0202")).unwrap();
        assert_eq!(v.table.get("Walk").unwrap().rows, 8);
        assert_eq!(v.table.get("Strike").unwrap().source, "Idle");
        assert!(v.dir.join("Walk-Anim.png").exists());
        assert_eq!(fetch.calls().len(), 3, "AnimData + Walk + Idle (Strike reuses Idle): {:?}", fetch.calls());
    }

    #[test]
    fn second_load_uses_cache() {
        let cache = tempfile::tempdir().unwrap();
        let fetch = FakeFetch::good();
        run(ensure_variant(&fetch, cache.path(), "sha1", "0202")).unwrap();
        let before = fetch.calls().len();
        run(ensure_variant(&fetch, cache.path(), "sha1", "0202")).unwrap();
        assert_eq!(fetch.calls().len(), before);
    }

    #[test]
    fn corrupt_or_missing_cached_sheet_is_redownloaded() {
        let cache = tempfile::tempdir().unwrap();
        let fetch = FakeFetch::good();
        let v = run(ensure_variant(&fetch, cache.path(), "sha1", "0202")).unwrap();
        std::fs::write(v.dir.join("Walk-Anim.png"), b"garbage").unwrap();
        run(ensure_variant(&fetch, cache.path(), "sha1", "0202")).unwrap();
        std::fs::remove_file(v.dir.join("Idle-Anim.png")).unwrap();
        let v = run(ensure_variant(&fetch, cache.path(), "sha1", "0202")).unwrap();
        assert!(v.dir.join("Idle-Anim.png").exists());
    }

    #[test]
    fn network_failure_retries_once_then_errors() {
        let cache = tempfile::tempdir().unwrap();
        let fetch = FakeFetch { files: Mutex::new(HashMap::new()), calls: Mutex::new(Vec::new()) };
        let err = run(ensure_variant(&fetch, cache.path(), "sha1", "0202")).unwrap_err();
        assert!(matches!(err, SpriteError::Download(_)), "{err}");
        assert_eq!(fetch.calls().len(), 2);
    }

    #[test]
    fn wrong_sheet_size_is_invalid_and_cache_is_cleared() {
        let cache = tempfile::tempdir().unwrap();
        let fetch = FakeFetch::good();
        fetch.files.lock().unwrap().insert(
            "https://raw.githubusercontent.com/PMDCollab/SpriteCollab/sha1/sprite/0202/Walk-Anim.png".into(),
            png_bytes(15, 64, |_, _| true),
        );
        let err = run(ensure_variant(&fetch, cache.path(), "sha1", "0202")).unwrap_err();
        assert!(matches!(err, SpriteError::Invalid(_)), "{err}");
        assert!(!cache.path().join("sprites/sha1/0202").exists());
    }

    #[test]
    fn portraits_download_once() {
        let cache = tempfile::tempdir().unwrap();
        let fetch = FakeFetch::good();
        let p = run(ensure_portrait(&fetch, cache.path(), "sha1", "0202")).unwrap();
        assert!(p.ends_with("portraits/sha1/0202.png") && p.exists());
        run(ensure_portrait(&fetch, cache.path(), "sha1", "0202")).unwrap();
        assert_eq!(fetch.calls().len(), 1);
    }
}
```
Add `pub mod sprites;` to `lib.rs`.

- [ ] **Step 6: Run to confirm failure**

Run: `cargo test --manifest-path src-tauri/Cargo.toml sprites`
Expected: compile errors (`Fetch`, `ensure_variant`, `SpriteError` missing).

- [ ] **Step 7: Implement `sprites`**

Insert above the tests in `src-tauri/src/sprites.rs`:
```rust
//! Downloads a variant's AnimData.xml and sheets from SpriteCollab (pinned SHA), caches and validates them.

use crate::animdata::{self, AnimTable};
use crate::bounds::{decode_alpha, AlphaSheet};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub trait Fetch {
    fn get(&self, url: &str) -> impl std::future::Future<Output = Result<Vec<u8>, String>> + Send;
}

pub struct HttpFetch(reqwest::Client);

impl HttpFetch {
    pub fn new() -> Self {
        HttpFetch(reqwest::Client::builder().user_agent("Wanderdex").build().expect("HTTP client"))
    }
}

impl Default for HttpFetch {
    fn default() -> Self { Self::new() }
}

impl Fetch for HttpFetch {
    async fn get(&self, url: &str) -> Result<Vec<u8>, String> {
        let resp = self.0.get(url).send().await.map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {} for {url}", resp.status()));
        }
        resp.bytes().await.map(|b| b.to_vec()).map_err(|e| e.to_string())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SpriteError {
    #[error("Couldn't download sprites: {0}")]
    Download(String),
    #[error("Downloaded sprites are invalid: {0}")]
    Invalid(String),
    #[error("Couldn't write the sprite cache: {0}")]
    Io(String),
}

impl serde::Serialize for SpriteError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub struct LoadedVariant {
    pub path: String,
    pub dir: PathBuf,
    pub table: AnimTable,
    pub sheets: HashMap<String, AlphaSheet>,
}

pub fn raw_url(sha: &str, rel: &str) -> String {
    format!("https://raw.githubusercontent.com/PMDCollab/SpriteCollab/{sha}/{rel}")
}

fn load_cached(dir: &Path, path: &str) -> Result<LoadedVariant, SpriteError> {
    let xml = std::fs::read_to_string(dir.join("AnimData.xml")).map_err(|e| SpriteError::Invalid(e.to_string()))?;
    let mut table = animdata::parse(&xml).map_err(|e| SpriteError::Invalid(e.to_string()))?;
    let mut sheets = HashMap::new();
    for source in table.sources() {
        let bytes = std::fs::read(dir.join(format!("{source}-Anim.png")))
            .map_err(|e| SpriteError::Invalid(format!("{source}: {e}")))?;
        let sheet = decode_alpha(&bytes).map_err(|e| SpriteError::Invalid(format!("{source}: {e}")))?;
        sheets.insert(source, sheet);
    }
    table
        .validate(|s| sheets.get(s).map(|a| (a.width, a.height)))
        .map_err(|e| SpriteError::Invalid(e.to_string()))?;
    Ok(LoadedVariant { path: path.to_string(), dir: dir.to_path_buf(), table, sheets })
}

async fn download<F: Fetch>(fetch: &F, dir: &Path, sha: &str, path: &str) -> Result<(), SpriteError> {
    std::fs::create_dir_all(dir).map_err(|e| SpriteError::Io(e.to_string()))?;
    let xml = fetch.get(&raw_url(sha, &format!("sprite/{path}/AnimData.xml"))).await.map_err(SpriteError::Download)?;
    let text = String::from_utf8(xml).map_err(|e| SpriteError::Invalid(e.to_string()))?;
    let table = animdata::parse(&text).map_err(|e| SpriteError::Invalid(e.to_string()))?;
    for source in table.sources() {
        let png = fetch
            .get(&raw_url(sha, &format!("sprite/{path}/{source}-Anim.png")))
            .await
            .map_err(SpriteError::Download)?;
        std::fs::write(dir.join(format!("{source}-Anim.png")), png).map_err(|e| SpriteError::Io(e.to_string()))?;
    }
    std::fs::write(dir.join("AnimData.xml"), text).map_err(|e| SpriteError::Io(e.to_string()))
}

/// Returns a validated variant, using the cache when valid; otherwise downloads (retrying once).
pub async fn ensure_variant<F: Fetch>(fetch: &F, cache_root: &Path, sha: &str, path: &str) -> Result<LoadedVariant, SpriteError> {
    let dir = cache_root.join("sprites").join(sha).join(path);
    if let Ok(v) = load_cached(&dir, path) {
        return Ok(v);
    }
    let mut last = SpriteError::Download("not attempted".into());
    for _ in 0..2 {
        let _ = std::fs::remove_dir_all(&dir);
        match download(fetch, &dir, sha, path).await {
            Ok(()) => match load_cached(&dir, path) {
                Ok(v) => return Ok(v),
                Err(e) => last = e,
            },
            Err(e) => last = e,
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    Err(last)
}

/// Returns the cached path of a species' Normal portrait, downloading it if needed.
pub async fn ensure_portrait<F: Fetch>(fetch: &F, cache_root: &Path, sha: &str, dex: &str) -> Result<PathBuf, SpriteError> {
    let file = cache_root.join("portraits").join(sha).join(format!("{dex}.png"));
    if std::fs::read(&file).ok().is_some_and(|b| decode_alpha(&b).is_ok()) {
        return Ok(file);
    }
    let bytes = fetch.get(&raw_url(sha, &format!("portrait/{dex}/Normal.png"))).await.map_err(SpriteError::Download)?;
    decode_alpha(&bytes).map_err(SpriteError::Invalid)?;
    std::fs::create_dir_all(file.parent().expect("has parent")).map_err(|e| SpriteError::Io(e.to_string()))?;
    std::fs::write(&file, bytes).map_err(|e| SpriteError::Io(e.to_string()))?;
    Ok(file)
}
```

- [ ] **Step 8: Run sprite tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml sprites`
Expected: 6 passed.

- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/bounds.rs src-tauri/src/sprites.rs src-tauri/src/lib.rs
git commit -m "Download, cache and validate SpriteCollab variants and portraits"
```

---

### Task 5: Frontend renderer and frame player

**Files:**
- Create: `src/shared/player.ts`, `src/shared/renderer.ts`, `src/shared/sheets.ts`, `src/shared/player.test.ts`, `src/shared/renderer.test.ts`
- Modify: `src/shared/types.ts`
- Delete: `src/shared/smoke.test.ts`

**Interfaces:**
- Produces (TypeScript):
  - `types.ts`: `AnimInfo { name: string; source: string; frameWidth: number; frameHeight: number; durations: number[]; rows: number }`, `ContentBox { minX: number; minY: number; maxX: number; maxY: number }`, `Visual { anim: string; dir: number; looped: boolean; seq: number }`, `PetInit { dir: string; table: AnimInfo[]; contentBox: ContentBox; scale: number; visual: Visual }`, `VariantPayload { dir: string; table: AnimInfo[]; contentBox: ContentBox; signature: string | null }`, plus index/picker types listed in the code below.
  - `player.ts`: `columnAt(durations: number[], tick: number, looped: boolean): number | null`; `class Player { constructor(table: AnimInfo[]); play(v: Visual, nowMs: number): void; frame(nowMs: number): { anim: AnimInfo; dir: number; col: number } | null }`
  - `renderer.ts`: `frameSource(anim, dir, col)`, `frameDest(anim, box, scale)`, `canvasSize(box, scale)`, `setupCanvas(canvas, box, scale, dpr)`, `drawFrame(ctx, img, anim, dir, col, box, scale)`
  - `sheets.ts`: `loadSheets(dir: string, table: AnimInfo[]): Promise<Map<string, HTMLImageElement>>` (keyed by `source`)

- [ ] **Step 1: Write the shared types**

Replace `src/shared/types.ts`:
```ts
// Mirrors of Rust payloads (serde camelCase).
export interface AnimInfo {
  name: string;
  source: string;
  frameWidth: number;
  frameHeight: number;
  durations: number[]; // ticks at 60/s
  rows: number;
}
export interface ContentBox { minX: number; minY: number; maxX: number; maxY: number }
export interface Visual { anim: string; dir: number; looped: boolean; seq: number }
export interface PetInit { dir: string; table: AnimInfo[]; contentBox: ContentBox; scale: number; visual: Visual }
export interface VariantPayload { dir: string; table: AnimInfo[]; contentBox: ContentBox; signature: string | null }
export interface Variant { path: string; formId: string; form: string; shiny: boolean; female: boolean; anims: string[]; credits: string[] }
export interface Species { dex: string; name: string; hasPortrait: boolean; variants: Variant[] }
export interface Index { spriteCollabSha: string; species: Species[] }
export interface ActivePet { label: string; variant: string; dex: string; name: string; form: string; shiny: boolean; female: boolean }
export interface CreditEntry { name: string; form: string; shiny: boolean; female: boolean; credits: string[] }

export const DIR = { Down: 0, DownRight: 1, Right: 2, UpRight: 3, Up: 4, UpLeft: 5, Left: 6, DownLeft: 7 } as const;
```
Delete `src/shared/smoke.test.ts`.

- [ ] **Step 2: Write failing tests**

`src/shared/player.test.ts`:
```ts
import { describe, expect, test } from "vitest";
import { columnAt, Player } from "./player";
import type { AnimInfo } from "./types";

const idle: AnimInfo = { name: "Idle", source: "Idle", frameWidth: 24, frameHeight: 40, durations: [40, 4, 4, 4, 4, 4, 4, 4, 4], rows: 8 };
const hop: AnimInfo = { name: "Hop", source: "Hop", frameWidth: 32, frameHeight: 80, durations: [2, 1, 2], rows: 8 };

describe("columnAt", () => {
  test("walks durations and loops", () => {
    expect(columnAt(idle.durations, 0, true)).toBe(0);
    expect(columnAt(idle.durations, 39, true)).toBe(0);
    expect(columnAt(idle.durations, 40, true)).toBe(1);
    expect(columnAt(idle.durations, 72, true)).toBe(0); // wrapped
  });
  test("one-shot returns null when finished", () => {
    expect(columnAt(hop.durations, 4, false)).toBe(2);
    expect(columnAt(hop.durations, 5, false)).toBeNull();
  });
});

describe("Player", () => {
  test("restarts timing on new anim or seq, keeps timing on direction change", () => {
    const p = new Player([idle, hop]);
    p.play({ anim: "Idle", dir: 0, looped: true, seq: 1 }, 0);
    expect(p.frame(700)!.col).toBe(1); // 700ms = 42 ticks
    p.play({ anim: "Idle", dir: 2, looped: true, seq: 1 }, 700);
    expect(p.frame(700)).toMatchObject({ dir: 2, col: 1 });
    p.play({ anim: "Idle", dir: 2, looped: true, seq: 2 }, 700);
    expect(p.frame(700)!.col).toBe(0);
  });
  test("holds the last frame after a one-shot ends", () => {
    const p = new Player([idle, hop]);
    p.play({ anim: "Hop", dir: 0, looped: false, seq: 5 }, 0);
    expect(p.frame(10_000)!.col).toBe(2);
  });
  test("unknown animation yields null", () => {
    const p = new Player([idle]);
    p.play({ anim: "Nope", dir: 0, looped: true, seq: 1 }, 0);
    expect(p.frame(0)).toBeNull();
  });
});
```
`src/shared/renderer.test.ts`:
```ts
import { expect, test } from "vitest";
import { canvasSize, frameDest, frameSource } from "./renderer";
import type { AnimInfo, ContentBox } from "./types";

const idle: AnimInfo = { name: "Idle", source: "Idle", frameWidth: 24, frameHeight: 40, durations: [40], rows: 8 };
const sleep: AnimInfo = { name: "Sleep", source: "Sleep", frameWidth: 24, frameHeight: 40, durations: [30, 35], rows: 1 };
const box: ContentBox = { minX: -8, minY: -16, maxX: 8, maxY: 7 }; // Wobbuffet's measured box

test("frameSource picks column and direction row; single-row sheets ignore direction", () => {
  expect(frameSource(idle, 6, 3)).toEqual({ sx: 72, sy: 240, sw: 24, sh: 40 });
  expect(frameSource(sleep, 6, 1)).toEqual({ sx: 24, sy: 0, sw: 24, sh: 40 });
});

test("frameDest centers the frame on the shared anchor at integer scale", () => {
  // anchor at (8,16) sprite px from the window's top-left; frame top-left = anchor - (12,20)
  expect(frameDest(idle, box, 3)).toEqual({ dx: -12, dy: -12, dw: 72, dh: 120 });
});

test("canvasSize is the content box times scale", () => {
  expect(canvasSize(box, 3)).toEqual({ width: 48, height: 69 });
});
```

- [ ] **Step 3: Run to confirm failure**

Run: `pnpm test`
Expected: FAIL (cannot resolve `./player`, `./renderer`).

- [ ] **Step 4: Implement**

`src/shared/player.ts`:
```ts
import type { AnimInfo, Visual } from "./types";

/** Frame column at a tick (60/s); null once a non-looping animation has finished. */
export function columnAt(durations: number[], tick: number, looped: boolean): number | null {
  const total = durations.reduce((a, b) => a + b, 0);
  if (!looped && tick >= total) return null;
  let t = looped ? tick % total : tick;
  for (let i = 0; i < durations.length; i++) {
    if (t < durations[i]) return i;
    t -= durations[i];
  }
  return durations.length - 1;
}

export class Player {
  private byName: Map<string, AnimInfo>;
  private visual: Visual | null = null;
  private startMs = 0;

  constructor(table: AnimInfo[]) {
    this.byName = new Map(table.map((a) => [a.name, a]));
  }

  play(v: Visual, nowMs: number): void {
    const prev = this.visual;
    if (!prev || prev.anim !== v.anim || prev.seq !== v.seq) this.startMs = nowMs;
    this.visual = v;
  }

  frame(nowMs: number): { anim: AnimInfo; dir: number; col: number } | null {
    if (!this.visual) return null;
    const anim = this.byName.get(this.visual.anim);
    if (!anim) return null;
    const tick = Math.floor(((nowMs - this.startMs) * 60) / 1000);
    const col = columnAt(anim.durations, tick, this.visual.looped) ?? anim.durations.length - 1;
    return { anim, dir: this.visual.dir, col };
  }
}
```
`src/shared/renderer.ts`:
```ts
import type { AnimInfo, ContentBox } from "./types";

export function frameSource(anim: AnimInfo, dir: number, col: number) {
  const row = anim.rows <= 1 ? 0 : dir % anim.rows;
  return { sx: col * anim.frameWidth, sy: row * anim.frameHeight, sw: anim.frameWidth, sh: anim.frameHeight };
}

/** Where a frame lands in the canvas (CSS px). Every frame's center sits on the anchor (-minX, -minY). */
export function frameDest(anim: AnimInfo, box: ContentBox, scale: number) {
  const ax = -box.minX;
  const ay = -box.minY;
  return {
    dx: (ax - Math.floor(anim.frameWidth / 2)) * scale,
    dy: (ay - Math.floor(anim.frameHeight / 2)) * scale,
    dw: anim.frameWidth * scale,
    dh: anim.frameHeight * scale,
  };
}

export function canvasSize(box: ContentBox, scale: number) {
  return { width: (box.maxX - box.minX) * scale, height: (box.maxY - box.minY) * scale };
}

/** Sizes the canvas (CSS px × device pixel ratio) and returns a context with smoothing off. */
export function setupCanvas(canvas: HTMLCanvasElement, box: ContentBox, scale: number, dpr: number): CanvasRenderingContext2D {
  const { width, height } = canvasSize(box, scale);
  canvas.style.width = `${width}px`;
  canvas.style.height = `${height}px`;
  canvas.width = Math.round(width * dpr);
  canvas.height = Math.round(height * dpr);
  const ctx = canvas.getContext("2d")!;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.imageSmoothingEnabled = false;
  return ctx;
}

export function drawFrame(
  ctx: CanvasRenderingContext2D,
  img: CanvasImageSource,
  anim: AnimInfo,
  dir: number,
  col: number,
  box: ContentBox,
  scale: number,
): void {
  const s = frameSource(anim, dir, col);
  const d = frameDest(anim, box, scale);
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(img, s.sx, s.sy, s.sw, s.sh, d.dx, d.dy, d.dw, d.dh);
}
```
`src/shared/sheets.ts`:
```ts
import { convertFileSrc } from "@tauri-apps/api/core";
import type { AnimInfo } from "./types";

/** Loads every source sheet of a variant from the local cache, keyed by source name. */
export async function loadSheets(dir: string, table: AnimInfo[]): Promise<Map<string, HTMLImageElement>> {
  const sources = [...new Set(table.map((a) => a.source))];
  const entries = await Promise.all(
    sources.map(
      (source) =>
        new Promise<[string, HTMLImageElement]>((resolve, reject) => {
          const img = new Image();
          img.onload = () => resolve([source, img]);
          img.onerror = () => reject(new Error(`Couldn't load ${source}-Anim.png`));
          img.src = convertFileSrc(`${dir}/${source}-Anim.png`);
        }),
    ),
  );
  return new Map(entries);
}
```

- [ ] **Step 5: Run tests and typecheck**

Run: `pnpm test && pnpm typecheck`
Expected: 8 tests pass, no type errors.

- [ ] **Step 6: Commit**

```bash
git add src/shared
git commit -m "Add frame player and nearest-neighbor renderer"
```

---

### Task 6: Ground detection and saved settings

**Files:**
- Create: `src-tauri/src/ground.rs`, `src-tauri/src/persist.rs`
- Modify: `src-tauri/src/lib.rs` (add `pub mod ground; pub mod persist;`)

**Interfaces:**
- Consumes: `index::{Index, find_variant}` (Task 3).
- Produces:
  - `ground::Ground { pub left: f64, pub right: f64, pub y: f64, pub px_per_pt: f64 }` (physical px; `Copy + PartialEq + Debug`)
  - `ground::MonitorInfo { pub x: f64, pub y: f64, pub width: f64, pub height: f64, pub scale: f64, pub primary: bool }` (work area, physical px)
  - `ground::ground_for(px: f64, py: f64, monitors: &[MonitorInfo]) -> Option<Ground>`
  - `persist::Settings { pub scale: u32, pub hidden: bool, pub pets: Vec<SavedPet>, pub credited: Vec<String> }` (Default: scale 3)
  - `persist::SavedPet { pub variant: String, pub x: f64, pub y: f64 }`
  - `persist::load(path: &Path) -> Settings`, `persist::save(path: &Path, s: &Settings) -> std::io::Result<()>`, `persist::restorable<'a>(s: &'a Settings, index: &Index) -> Vec<&'a SavedPet>`

- [ ] **Step 1: Write failing tests**

`src-tauri/src/ground.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn monitors() -> Vec<MonitorInfo> {
        vec![
            MonitorInfo { x: 0.0, y: 0.0, width: 2880.0, height: 1700.0, scale: 2.0, primary: true },
            MonitorInfo { x: 2880.0, y: 200.0, width: 1920.0, height: 1040.0, scale: 1.0, primary: false },
        ]
    }

    #[test]
    fn picks_the_monitor_under_the_pet() {
        let g = ground_for(3000.0, 900.0, &monitors()).unwrap();
        assert_eq!(g, Ground { left: 2880.0, right: 4800.0, y: 1240.0, px_per_pt: 1.0 });
    }

    #[test]
    fn feet_exactly_on_the_ground_stay_on_that_monitor() {
        assert_eq!(ground_for(3000.0, 1240.0, &monitors()).unwrap().right, 4800.0);
    }

    #[test]
    fn point_above_a_monitor_uses_that_monitors_column() {
        assert_eq!(ground_for(3000.0, -500.0, &monitors()).unwrap().left, 2880.0);
    }

    #[test]
    fn point_outside_every_monitor_falls_back_to_primary() {
        let g = ground_for(99_999.0, 99_999.0, &monitors()).unwrap();
        assert_eq!((g.left, g.y, g.px_per_pt), (0.0, 1700.0, 2.0));
    }

    #[test]
    fn no_monitors_means_no_ground() {
        assert!(ground_for(0.0, 0.0, &[]).is_none());
    }
}
```
`src-tauri/src/persist.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let s = Settings {
            scale: 4,
            hidden: true,
            pets: vec![SavedPet { variant: "0202".into(), x: 10.0, y: 20.0 }],
            credited: vec!["0202".into()],
        };
        save(&path, &s).unwrap();
        assert_eq!(load(&path), s);
    }

    #[test]
    fn missing_or_corrupt_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        assert_eq!(load(&path), Settings::default());
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(load(&path), Settings::default());
        assert_eq!(Settings::default().scale, 3);
    }

    #[test]
    fn invalid_scale_is_reset_and_unknown_fields_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"scale": 7, "future": 1}"#).unwrap();
        assert_eq!(load(&path).scale, 3);
    }

    #[test]
    fn restorable_skips_variants_missing_from_the_index() {
        let index = crate::index::Index {
            sprite_collab_sha: "x".into(),
            species: vec![crate::index::Species {
                dex: "0202".into(),
                name: "Wobbuffet".into(),
                has_portrait: true,
                variants: vec![crate::index::Variant {
                    path: "0202".into(),
                    form_id: "0000".into(),
                    form: "Normal".into(),
                    shiny: false,
                    female: false,
                    anims: vec!["Idle".into(), "Walk".into()],
                    credits: vec![],
                }],
            }],
        };
        let s = Settings {
            pets: vec![
                SavedPet { variant: "0202".into(), x: 0.0, y: 0.0 },
                SavedPet { variant: "9999/0001".into(), x: 0.0, y: 0.0 },
            ],
            ..Settings::default()
        };
        let r = restorable(&s, &index);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].variant, "0202");
    }
}
```
Add `pub mod ground; pub mod persist;` to `lib.rs`.

- [ ] **Step 2: Run to confirm failure**

Run: `cargo test --manifest-path src-tauri/Cargo.toml ground persist`
Expected: compile errors.

- [ ] **Step 3: Implement**

Insert above the tests in `src-tauri/src/ground.rs`:
```rust
//! Which bottom edge a pet stands on, from the monitors' work areas (physical pixels).

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ground {
    pub left: f64,
    pub right: f64,
    /// The bottom of the work area: pets' feet rest here.
    pub y: f64,
    /// Physical pixels per point on this monitor.
    pub px_per_pt: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonitorInfo {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub scale: f64,
    pub primary: bool,
}

impl MonitorInfo {
    fn ground(&self) -> Ground {
        Ground { left: self.x, right: self.x + self.width, y: self.y + self.height, px_per_pt: self.scale }
    }
    fn contains_x(&self, px: f64) -> bool {
        px >= self.x && px < self.x + self.width
    }
}

/// The ground under (px, py): the monitor containing the point just above the feet, else the monitor
/// whose column contains px, else the primary monitor, else the first.
pub fn ground_for(px: f64, py: f64, monitors: &[MonitorInfo]) -> Option<Ground> {
    let probe_y = py - 1.0;
    monitors
        .iter()
        .find(|m| m.contains_x(px) && probe_y >= m.y && probe_y < m.y + m.height)
        .or_else(|| monitors.iter().find(|m| m.contains_x(px)))
        .or_else(|| monitors.iter().find(|m| m.primary))
        .or_else(|| monitors.first())
        .map(MonitorInfo::ground)
}
```
Insert above the tests in `src-tauri/src/persist.rs`:
```rust
//! Settings and active pets, saved as JSON in the app config directory.

use crate::index::{find_variant, Index};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const SCALES: [u32; 3] = [2, 3, 4];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub scale: u32,
    pub hidden: bool,
    pub pets: Vec<SavedPet>,
    /// Variant paths ever spawned, for the credits window.
    pub credited: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { scale: 3, hidden: false, pets: Vec::new(), credited: Vec::new() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedPet {
    pub variant: String,
    pub x: f64,
    pub y: f64,
}

pub fn load(path: &Path) -> Settings {
    let mut s: Settings = std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    if !SCALES.contains(&s.scale) {
        s.scale = 3;
    }
    s
}

pub fn save(path: &Path, s: &Settings) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(s).expect("settings serialize"))?;
    std::fs::rename(tmp, path)
}

/// Saved pets whose variant still exists in the index.
pub fn restorable<'a>(s: &'a Settings, index: &Index) -> Vec<&'a SavedPet> {
    s.pets.iter().filter(|p| find_variant(index, &p.variant).is_some()).collect()
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml ground persist`
Expected: 9 passed.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/ground.rs src-tauri/src/persist.rs src-tauri/src/lib.rs
git commit -m "Add ground detection across monitors and saved settings"
```

---

### Task 7: Pet behavior brain

**Files:**
- Create: `src-tauri/src/brain.rs`
- Modify: `src-tauri/src/lib.rs` (add `pub mod brain;`)

**Interfaces:**
- Consumes: `animdata::AnimTable` (Task 2), `ground::Ground` (Task 6).
- Produces:
  - Constants `DOWN = 0`, `RIGHT = 2`, `LEFT = 6`, `FLOURISH_PREFERENCE: [&str; 7]`.
  - `pub fn dir_from_delta(dx: f64, dy: f64) -> u8` (screen coordinates, y down)
  - `pub fn behavior_anims(signature: Option<&str>) -> Vec<String>` (every animation a pet may show; used for content boxes)
  - `pub struct AnimSet` with `AnimSet::from_table(&AnimTable)` and `AnimSet::from_secs(&[(&str, f64)])` (test helper, also public)
  - `pub struct Visual { pub anim: String, pub dir: u8, pub looped: bool, pub seq: u64 }` (camelCase serde, `Clone + PartialEq`)
  - `pub struct Brain` with `pub x: f64`, `pub y: f64` (physical px; x = anchor, y = feet), and methods: `new(seed: u64, anims: AnimSet, signature: Option<String>, scale: u32, extent: (f64, f64), x: f64, ground: &Ground) -> Brain`, `tick(&mut self, dt: f64, g: &Ground)`, `visual(&self) -> Visual`, `click(&mut self)`, `drag_start(&mut self, cursor: (f64, f64))`, `drag_move(&mut self, cursor: (f64, f64))`, `drop_pet(&mut self)`, `set_scale(&mut self, scale: u32)`, `is_dragged(&self) -> bool`.
  - `extent` = (pixels left of anchor, pixels right of anchor) in sprite pixels: `(-box.min_x, box.max_x)`.

- [ ] **Step 1: Write failing tests**

Create `src-tauri/src/brain.rs` with tests only:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    const DT: f64 = 1.0 / 60.0;

    fn ground() -> Ground {
        Ground { left: 0.0, right: 1000.0, y: 800.0, px_per_pt: 1.0 }
    }

    fn full_set() -> AnimSet {
        AnimSet::from_secs(&[("Walk", 0.6), ("Idle", 1.2), ("Sleep", 1.1), ("Hop", 0.4), ("Withdraw", 0.73), ("Rotate", 0.3)])
    }

    fn brain(anims: AnimSet, signature: Option<&str>) -> Brain {
        Brain::new(42, anims, signature.map(String::from), 3, (8.0, 8.0), 500.0, &ground())
    }

    #[test]
    fn directions_map_to_pmd_rows() {
        assert_eq!(dir_from_delta(0.0, 1.0), 0); // down
        assert_eq!(dir_from_delta(1.0, 1.0), 1);
        assert_eq!(dir_from_delta(1.0, 0.0), 2); // right
        assert_eq!(dir_from_delta(1.0, -1.0), 3);
        assert_eq!(dir_from_delta(0.0, -1.0), 4); // up
        assert_eq!(dir_from_delta(-1.0, -1.0), 5);
        assert_eq!(dir_from_delta(-1.0, 0.0), 6); // left
        assert_eq!(dir_from_delta(-1.0, 1.0), 7);
    }

    #[test]
    fn behavior_anims_include_signature_once() {
        let a = behavior_anims(Some("Withdraw"));
        assert!(a.contains(&"Walk".to_string()) && a.contains(&"Withdraw".to_string()));
        assert_eq!(a.iter().filter(|n| *n == "Hop").count(), 1);
    }

    #[test]
    fn walking_pet_turns_around_at_the_right_edge() {
        let mut b = brain(full_set(), None);
        b.force_walk(RIGHT, 10_000.0);
        b.x = 970.0; // right extent is 8 px * scale 3 = 24 → max x = 976
        for _ in 0..30 { b.tick(DT, &ground()); }
        assert_eq!(b.visual().dir, LEFT);
        assert!(b.x <= 976.0);
    }

    #[test]
    fn stays_on_moved_ground_and_inside_shrunken_range() {
        let mut b = brain(full_set(), None);
        b.x = 900.0;
        let smaller = Ground { left: 0.0, right: 600.0, y: 700.0, px_per_pt: 1.0 };
        b.tick(DT, &smaller);
        assert_eq!(b.y, 700.0);
        assert!(b.x <= 600.0 - 24.0);
    }

    #[test]
    fn minimal_set_never_panics_and_stays_in_range() {
        let mut b = brain(AnimSet::from_secs(&[("Walk", 0.6), ("Idle", 1.2)]), None);
        let g = ground();
        for _ in 0..20_000 {
            b.tick(DT, &g);
            let v = b.visual();
            assert!(v.anim == "Walk" || v.anim == "Idle", "{}", v.anim);
            assert!(b.x >= 24.0 && b.x <= 976.0 && b.x.is_finite());
            assert_eq!(b.y, 800.0);
        }
        b.click(); // no flourish available: nothing happens
        assert!(b.visual().anim == "Walk" || b.visual().anim == "Idle");
    }

    #[test]
    fn every_behavior_eventually_happens_with_a_full_set() {
        let mut b = brain(full_set(), None);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..200_000 {
            b.tick(DT, &ground());
            seen.insert(b.visual().anim);
        }
        for anim in ["Walk", "Idle", "Sleep", "Hop", "Rotate"] {
            assert!(seen.contains(anim), "{anim} never shown; saw {seen:?}");
        }
    }

    #[test]
    fn click_plays_signature_once_then_moves_on() {
        let mut b = brain(full_set(), Some("Withdraw"));
        b.click();
        let v = b.visual();
        assert_eq!((v.anim.as_str(), v.looped, v.dir), ("Withdraw", false, DOWN));
        for _ in 0..60 { b.tick(DT, &ground()); } // 1 s > 0.73 s
        assert_ne!(b.visual().seq, v.seq);
    }

    #[test]
    fn drag_faces_movement_then_drop_falls_and_lands_with_hop() {
        let mut b = brain(full_set(), None);
        b.drag_start((500.0, 790.0));
        b.drag_move((450.0, 400.0)); // up-left-ish, mostly up
        assert!(b.is_dragged());
        assert_eq!(b.visual().anim, "Walk");
        assert_eq!(b.visual().dir, dir_from_delta(-50.0, -390.0));
        assert_eq!((b.x, b.y), (450.0, 410.0)); // keeps the grab offset (+10 px)
        b.drop_pet();
        let mut landed = false;
        for _ in 0..120 {
            b.tick(DT, &ground());
            if b.visual().anim == "Hop" { landed = true; break; }
        }
        assert!(landed, "should land within 2 s");
        assert_eq!(b.y, 800.0);
    }

    #[test]
    fn drop_without_hop_goes_idle() {
        let mut b = brain(AnimSet::from_secs(&[("Walk", 0.6), ("Idle", 1.2)]), None);
        b.drag_start((500.0, 800.0));
        b.drop_pet();
        for _ in 0..120 { b.tick(DT, &ground()); }
        assert_eq!(b.y, 800.0);
        assert!(!b.is_dragged());
    }

    #[test]
    fn pet_wider_than_ground_is_centered() {
        let narrow = Ground { left: 100.0, right: 120.0, y: 800.0, px_per_pt: 1.0 };
        let mut b = Brain::new(1, full_set(), None, 4, (40.0, 40.0), 0.0, &narrow);
        let mut prev = b.visual();
        for _ in 0..600 {
            b.tick(DT, &narrow);
            assert_eq!(b.x, 110.0);
            let v = b.visual();
            if v.seq == prev.seq {
                assert_eq!(v.dir, prev.dir, "direction must not flip without a mode change");
            }
            prev = v;
        }
    }

    #[test]
    fn same_seed_same_behavior() {
        let mut a = brain(full_set(), None);
        let mut b = brain(full_set(), None);
        for _ in 0..5_000 {
            a.tick(DT, &ground());
            b.tick(DT, &ground());
            assert_eq!(a.visual(), b.visual());
            assert_eq!(a.x, b.x);
        }
    }
}
```
Add `pub mod brain;` to `lib.rs`.

- [ ] **Step 2: Run to confirm failure**

Run: `cargo test --manifest-path src-tauri/Cargo.toml brain`
Expected: compile errors.

- [ ] **Step 3: Implement the brain**

Insert above the tests in `src-tauri/src/brain.rs`:
```rust
//! One pet's behavior: walk / idle / flourish / nap on the ground, plus drag, fall and land.
//! Pure logic in physical pixels; the pets module moves windows to match.

use crate::animdata::AnimTable;
use crate::ground::Ground;
use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::Serialize;
use std::collections::HashMap;

pub const DOWN: u8 = 0;
pub const RIGHT: u8 = 2;
pub const LEFT: u8 = 6;
pub const FLOURISH_PREFERENCE: [&str; 7] = ["Pose", "Hop", "Rotate", "Charge", "Double", "Swing", "Attack"];

/// Tuning (spec §5.2).
const WEIGHT_WALK: u32 = 50;
const WEIGHT_IDLE: u32 = 30;
const WEIGHT_FLOURISH: u32 = 15;
const WEIGHT_NAP: u32 = 5;
const IDLE_SECS: std::ops::Range<f64> = 2.0..8.0;
const NAP_SECS: std::ops::Range<f64> = 20.0..60.0;
const WALK_PTS: std::ops::Range<f64> = 40.0..240.0;
const WALK_PTS_PER_SEC_PER_SCALE: f64 = 10.0;
const GRAVITY_PTS_PER_SEC2: f64 = 2400.0;
const DRAG_TURN_PX: f64 = 6.0;

/// Screen-space movement (y down) to a PMD direction row.
pub fn dir_from_delta(dx: f64, dy: f64) -> u8 {
    let angle = dy.atan2(dx).to_degrees(); // 0 right, 90 down, ±180 left, -90 up
    (((2 - (angle / 45.0).round() as i32) % 8 + 8) % 8) as u8
}

/// Every animation a pet may show (for sizing its window).
pub fn behavior_anims(signature: Option<&str>) -> Vec<String> {
    let mut names: Vec<String> = ["Walk", "Idle", "Sleep", "Hop"].iter().map(|s| s.to_string()).collect();
    for n in FLOURISH_PREFERENCE.iter().copied().chain(signature) {
        if !names.iter().any(|x| x == n) {
            names.push(n.to_string());
        }
    }
    names
}

#[derive(Debug, Clone)]
pub struct AnimSet {
    secs: HashMap<String, f64>,
}

impl AnimSet {
    pub fn from_table(t: &AnimTable) -> Self {
        AnimSet { secs: t.anims.iter().map(|a| (a.name.clone(), a.total_secs())).collect() }
    }
    pub fn from_secs(list: &[(&str, f64)]) -> Self {
        AnimSet { secs: list.iter().map(|(n, s)| (n.to_string(), *s)).collect() }
    }
    fn has(&self, name: &str) -> bool {
        self.secs.contains_key(name)
    }
    fn secs(&self, name: &str) -> f64 {
        self.secs.get(name).copied().unwrap_or(0.0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Visual {
    pub anim: String,
    pub dir: u8,
    pub looped: bool,
    /// Bumps on every mode change so the renderer restarts the animation.
    pub seq: u64,
}

#[derive(Debug, Clone, PartialEq)]
enum Mode {
    Walk { remaining_pts: f64 },
    Idle { remaining: f64 },
    OneShot { anim: String, remaining: f64 },
    Nap { remaining: f64 },
    Dragged,
    Falling { vy: f64 },
    Landing { remaining: f64 },
}

pub struct Brain {
    mode: Mode,
    /// Anchor (frame center) x, physical px.
    pub x: f64,
    /// Feet y (bottom of the window), physical px.
    pub y: f64,
    facing: u8,
    rng: ChaCha8Rng,
    anims: AnimSet,
    signature: Option<String>,
    scale: f64,
    extent: (f64, f64),
    seq: u64,
    drag_offset: (f64, f64),
    last_drag: (f64, f64),
}

impl Brain {
    pub fn new(seed: u64, anims: AnimSet, signature: Option<String>, scale: u32, extent: (f64, f64), x: f64, ground: &Ground) -> Brain {
        let signature = signature.filter(|s| anims.has(s));
        let mut b = Brain {
            mode: Mode::Idle { remaining: 1.0 },
            x,
            y: ground.y,
            facing: DOWN,
            rng: ChaCha8Rng::seed_from_u64(seed),
            anims,
            signature,
            scale: scale as f64,
            extent,
            seq: 0,
            drag_offset: (0.0, 0.0),
            last_drag: (0.0, 0.0),
        };
        b.clamp_x(ground);
        b
    }

    pub fn set_scale(&mut self, scale: u32) {
        self.scale = scale as f64;
    }

    pub fn is_dragged(&self) -> bool {
        self.mode == Mode::Dragged
    }

    fn x_range(&self, g: &Ground) -> (f64, f64) {
        let k = self.scale * g.px_per_pt;
        let lo = g.left + self.extent.0 * k;
        let hi = g.right - self.extent.1 * k;
        if lo > hi {
            let mid = (g.left + g.right) / 2.0;
            (mid, mid)
        } else {
            (lo, hi)
        }
    }

    fn clamp_x(&mut self, g: &Ground) {
        let (lo, hi) = self.x_range(g);
        self.x = self.x.clamp(lo, hi);
    }

    fn set_mode(&mut self, mode: Mode, facing: u8) {
        self.mode = mode;
        self.facing = facing;
        self.seq += 1;
    }

    fn start_idle(&mut self) {
        let remaining = self.rng.random_range(IDLE_SECS);
        self.set_mode(Mode::Idle { remaining }, DOWN);
    }

    fn start_walk(&mut self) {
        let facing = if self.rng.random_bool(0.5) { LEFT } else { RIGHT };
        let remaining_pts = self.rng.random_range(WALK_PTS);
        self.set_mode(Mode::Walk { remaining_pts }, facing);
    }

    fn start_one_shot(&mut self, anim: String) {
        let remaining = self.anims.secs(&anim);
        self.set_mode(Mode::OneShot { anim, remaining }, DOWN);
    }

    fn pick_flourish(&mut self) -> Option<String> {
        if let Some(sig) = &self.signature {
            return Some(sig.clone());
        }
        let options: Vec<&str> = FLOURISH_PREFERENCE.iter().copied().filter(|n| self.anims.has(n)).collect();
        if options.is_empty() {
            None
        } else {
            Some(options[self.rng.random_range(0..options.len())].to_string())
        }
    }

    fn choose_next(&mut self) {
        let flourish = self.pick_flourish();
        let can_nap = self.anims.has("Sleep");
        let weights = [
            WEIGHT_WALK,
            WEIGHT_IDLE,
            if flourish.is_some() { WEIGHT_FLOURISH } else { 0 },
            if can_nap { WEIGHT_NAP } else { 0 },
        ];
        let mut roll = self.rng.random_range(0..weights.iter().sum::<u32>());
        let mut choice = 1;
        for (i, w) in weights.iter().enumerate() {
            if roll < *w {
                choice = i;
                break;
            }
            roll -= w;
        }
        match choice {
            0 => self.start_walk(),
            2 => self.start_one_shot(flourish.expect("weight is 0 without a flourish")),
            3 => {
                let remaining = self.rng.random_range(NAP_SECS);
                self.set_mode(Mode::Nap { remaining }, DOWN);
            }
            _ => self.start_idle(),
        }
    }

    fn land(&mut self) {
        if self.anims.has("Hop") {
            let remaining = self.anims.secs("Hop");
            self.set_mode(Mode::Landing { remaining }, DOWN);
        } else {
            self.start_idle();
        }
    }

    pub fn tick(&mut self, dt: f64, g: &Ground) {
        match &mut self.mode {
            Mode::Dragged => return,
            Mode::Falling { vy } => {
                *vy += GRAVITY_PTS_PER_SEC2 * g.px_per_pt * dt;
                self.y += *vy * dt;
                self.clamp_x(g);
                if self.y >= g.y {
                    self.y = g.y;
                    self.land();
                }
                return;
            }
            _ => {}
        }
        self.y = g.y;
        self.clamp_x(g);
        let (lo, hi) = self.x_range(g);
        let speed_pts = WALK_PTS_PER_SEC_PER_SCALE * self.scale;
        let done = match &mut self.mode {
            Mode::Walk { remaining_pts } => {
                if lo >= hi {
                    *remaining_pts = 0.0; // no room to walk: pick something else
                }
                let step = speed_pts * dt;
                let sign = if self.facing == LEFT { -1.0 } else { 1.0 };
                self.x = (self.x + sign * step * g.px_per_pt).clamp(lo, hi);
                if self.x <= lo && self.facing == LEFT {
                    self.facing = RIGHT;
                } else if self.x >= hi && self.facing == RIGHT {
                    self.facing = LEFT;
                }
                *remaining_pts -= step;
                *remaining_pts <= 0.0
            }
            Mode::Idle { remaining } | Mode::Nap { remaining } | Mode::OneShot { remaining, .. } | Mode::Landing { remaining } => {
                *remaining -= dt;
                *remaining <= 0.0
            }
            Mode::Dragged | Mode::Falling { .. } => false,
        };
        if done {
            self.choose_next();
        }
    }

    pub fn visual(&self) -> Visual {
        let (anim, looped) = match &self.mode {
            Mode::Walk { .. } | Mode::Dragged | Mode::Falling { .. } => ("Walk".to_string(), true),
            Mode::Idle { .. } => ("Idle".to_string(), true),
            Mode::OneShot { anim, .. } => (anim.clone(), false),
            Mode::Nap { .. } => ("Sleep".to_string(), true),
            Mode::Landing { .. } => ("Hop".to_string(), false),
        };
        Visual { anim, dir: self.facing, looped, seq: self.seq }
    }

    pub fn click(&mut self) {
        if matches!(self.mode, Mode::Dragged | Mode::Falling { .. }) {
            return;
        }
        if let Some(anim) = self.pick_flourish() {
            self.start_one_shot(anim);
        }
    }

    pub fn drag_start(&mut self, cursor: (f64, f64)) {
        self.drag_offset = (self.x - cursor.0, self.y - cursor.1);
        self.last_drag = cursor;
        let facing = self.facing;
        self.set_mode(Mode::Dragged, facing);
    }

    pub fn drag_move(&mut self, cursor: (f64, f64)) {
        if self.mode != Mode::Dragged {
            return;
        }
        self.x = cursor.0 + self.drag_offset.0;
        self.y = cursor.1 + self.drag_offset.1;
        let (dx, dy) = (cursor.0 - self.last_drag.0, cursor.1 - self.last_drag.1);
        if dx.hypot(dy) >= DRAG_TURN_PX {
            self.facing = dir_from_delta(dx, dy);
            self.last_drag = cursor;
        }
    }

    pub fn drop_pet(&mut self) {
        if self.mode == Mode::Dragged {
            self.set_mode(Mode::Falling { vy: 0.0 }, DOWN);
        }
    }

    #[cfg(test)]
    fn force_walk(&mut self, facing: u8, pts: f64) {
        self.set_mode(Mode::Walk { remaining_pts: pts }, facing);
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml brain`
Expected: 11 passed. If `every_behavior_eventually_happens_with_a_full_set` fails because one-shot animations never appear, check that `choose_next` runs when a mode ends (the `done` flag). Don't loosen the test.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/brain.rs src-tauri/src/lib.rs
git commit -m "Add pet behavior brain (walk, idle, flourish, nap, drag, fall)"
```

---

### Task 8: Pet windows, the 60 Hz loop, and pet commands

**Files:**
- Create: `src-tauri/src/pets.rs`, `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs` (wire state, loop, commands, restore)
- Modify: `src/pet/main.ts`

**Interfaces:**
- Consumes: `sprites::{ensure_variant, HttpFetch, LoadedVariant}`, `bounds::{content_box, BoxI}`, `brain::*`, `ground::*`, `persist::*`, `index::{bundled, find_variant, signature_for}`.
- Produces (Rust):
  - `pets::AppState { pub pets: Mutex<Vec<Pet>>, pub settings: Mutex<Settings>, pub cache_root: PathBuf, pub settings_path: PathBuf, pub http: HttpFetch, pub next_id: AtomicU64 }`
  - `pets::spawn_pet(app: &AppHandle, variant: &str, at: Option<(f64, f64)>) -> Result<(), String>` (async)
  - `pets::remove_pet(app: &AppHandle, label: &str)`, `pets::remove_all(app: &AppHandle)`, `pets::set_scale(app: &AppHandle, scale: u32)`, `pets::set_hidden(app: &AppHandle, hidden: bool)`, `pets::save_settings(app: &AppHandle)`, `pets::start_loop(app: AppHandle)`, `pets::restore(app: AppHandle)`
  - Event `pet:anim` (payload `Visual`), sent with `emit_to(label, ...)`. Event `pet:scale` (payload `u32`). Global event `pets:changed` (no payload).
  - Commands: `pet_ready`, `pet_click`, `pet_drag_start`, `pet_drag_move`, `pet_drop`, `pet_context_menu`. All are `async` and take the calling `WebviewWindow`.
  - The spawn position defaults to a random spot on the ground of the monitor under the cursor. That's the picker's monitor, since the user just clicked Spawn there (spec §5.2).
- Produces (TS): `src/pet/main.ts`, the pet window runtime.

- [ ] **Step 1: Write `pets.rs`**

```rust
//! Live pets: one transparent window each, moved by a 60 Hz loop that ticks every pet's brain.

use crate::bounds::{content_box, BoxI};
use crate::brain::{behavior_anims, AnimSet, Brain, Visual};
use crate::ground::{ground_for, Ground, MonitorInfo};
use crate::index::{bundled, find_variant, signature_for};
use crate::persist::{self, SavedPet, Settings};
use crate::sprites::{ensure_variant, HttpFetch};
use crate::animdata::AnimTable;
use rand::RngExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub struct Pet {
    pub label: String,
    pub variant: String,
    pub dir: PathBuf,
    pub table: AnimTable,
    pub content: BoxI,
    pub brain: Brain,
    pub window: WebviewWindow,
    last_visual: Option<Visual>,
    last_ppt: f64,
    last_pos: (i32, i32),
    shown: bool,
}

pub struct AppState {
    pub pets: Mutex<Vec<Pet>>,
    pub settings: Mutex<Settings>,
    pub cache_root: PathBuf,
    pub settings_path: PathBuf,
    pub http: HttpFetch,
    pub next_id: AtomicU64,
    monitors: Mutex<Vec<MonitorInfo>>,
}

impl AppState {
    pub fn new(cache_root: PathBuf, settings_path: PathBuf) -> Self {
        let settings = persist::load(&settings_path);
        AppState {
            pets: Mutex::new(Vec::new()),
            settings: Mutex::new(settings),
            cache_root,
            settings_path,
            http: HttpFetch::new(),
            next_id: AtomicU64::new(1),
            monitors: Mutex::new(Vec::new()),
        }
    }
}

pub fn monitor_infos(app: &AppHandle) -> Vec<MonitorInfo> {
    let primary = app.primary_monitor().ok().flatten().map(|m| *m.position());
    app.available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|m| {
            let wa = m.work_area();
            MonitorInfo {
                x: wa.position.x as f64,
                y: wa.position.y as f64,
                width: wa.size.width as f64,
                height: wa.size.height as f64,
                scale: m.scale_factor(),
                primary: primary == Some(*m.position()),
            }
        })
        .collect()
}

fn ground_at(app: &AppHandle, x: f64, y: f64) -> Ground {
    let state = app.state::<AppState>();
    let mut monitors = state.monitors.lock().unwrap();
    if monitors.is_empty() {
        *monitors = monitor_infos(app);
    }
    ground_for(x, y, &monitors).unwrap_or(Ground { left: 0.0, right: 1280.0, y: 800.0, px_per_pt: 1.0 })
}

fn cursor(app: &AppHandle) -> (f64, f64) {
    app.cursor_position().map(|p| (p.x, p.y)).unwrap_or((0.0, 0.0))
}

fn window_size(content: &BoxI, scale: u32, ppt: f64) -> PhysicalSize<u32> {
    let k = scale as f64 * ppt;
    PhysicalSize::new((content.width() as f64 * k).round() as u32, (content.height() as f64 * k).round() as u32)
}

/// Downloads (if needed) and spawns a pet. `at` is a physical (x, feet y); default: random spot under the cursor.
pub async fn spawn_pet(app: &AppHandle, variant: &str, at: Option<(f64, f64)>) -> Result<(), String> {
    let index = bundled();
    let (species, _) = find_variant(index, variant).ok_or_else(|| format!("Unknown variant {variant}"))?;
    let state = app.state::<AppState>();
    let loaded = ensure_variant(&state.http, &state.cache_root, &index.sprite_collab_sha, variant)
        .await
        .map_err(|e| e.to_string())?;
    let signature = signature_for(&species.dex).map(String::from);
    let content = content_box(&loaded.table, &loaded.sheets, &behavior_anims(signature.as_deref()));
    let scale = state.settings.lock().unwrap().scale;

    let (x, y) = at.unwrap_or_else(|| {
        let c = cursor(app);
        let g = ground_at(app, c.0, c.1);
        (rand::rng().random_range(g.left..g.right.max(g.left + 1.0)), g.y)
    });
    let g = ground_at(app, x, y);
    let id = state.next_id.fetch_add(1, Ordering::Relaxed);
    let label = format!("pet-{id}");
    let pts = (content.width() as f64 * scale as f64, content.height() as f64 * scale as f64);
    let window = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("pet.html".into()))
        .title("Wanderdex pet")
        .transparent(true)
        .decorations(false)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible_on_all_workspaces(true)
        .focused(false)
        .focusable(false)
        .accept_first_mouse(true)
        .visible(false)
        .inner_size(pts.0, pts.1)
        .build()
        .map_err(|e| e.to_string())?;
    let mut brain = Brain::new(id ^ 0x5eed, AnimSet::from_table(&loaded.table), signature, scale, (-content.min_x as f64, content.max_x as f64), x, &g);
    brain.y = y.min(g.y);
    if brain.y < g.y {
        brain.drag_start((brain.x, brain.y));
        brain.drop_pet(); // restored mid-air: fall to the ground
    }
    let pet = Pet {
        label,
        variant: variant.to_string(),
        dir: loaded.dir,
        table: loaded.table,
        content,
        brain,
        window,
        last_visual: None,
        last_ppt: 0.0,
        last_pos: (i32::MIN, i32::MIN),
        shown: false,
    };
    state.pets.lock().unwrap().push(pet);
    {
        let mut s = state.settings.lock().unwrap();
        if !s.credited.iter().any(|c| c == variant) {
            s.credited.push(variant.to_string());
        }
    }
    save_settings(app);
    let _ = app.emit("pets:changed", ());
    Ok(())
}

pub fn remove_pet(app: &AppHandle, label: &str) {
    let state = app.state::<AppState>();
    let removed: Vec<Pet> = {
        let mut pets = state.pets.lock().unwrap();
        let (gone, keep): (Vec<Pet>, Vec<Pet>) = pets.drain(..).partition(|p| p.label == label);
        *pets = keep;
        gone
    };
    for p in removed {
        let _ = p.window.destroy();
    }
    save_settings(app);
    let _ = app.emit("pets:changed", ());
}

pub fn remove_all(app: &AppHandle) {
    let state = app.state::<AppState>();
    let all: Vec<Pet> = state.pets.lock().unwrap().drain(..).collect();
    for p in all {
        let _ = p.window.destroy();
    }
    save_settings(app);
    let _ = app.emit("pets:changed", ());
}

pub fn set_scale(app: &AppHandle, scale: u32) {
    let state = app.state::<AppState>();
    state.settings.lock().unwrap().scale = scale;
    for p in state.pets.lock().unwrap().iter_mut() {
        p.brain.set_scale(scale);
        p.last_ppt = 0.0; // force a resize on the next tick
        let _ = app.emit_to(p.label.as_str(), "pet:scale", scale);
    }
    save_settings(app);
}

pub fn set_hidden(app: &AppHandle, hidden: bool) {
    let state = app.state::<AppState>();
    state.settings.lock().unwrap().hidden = hidden;
    for p in state.pets.lock().unwrap().iter() {
        let _ = if hidden { p.window.hide() } else { p.window.show() };
    }
    save_settings(app);
}

/// Writes settings, including every live pet's position.
pub fn save_settings(app: &AppHandle) {
    let state = app.state::<AppState>();
    let pets: Vec<SavedPet> = state
        .pets
        .lock()
        .unwrap()
        .iter()
        .map(|p| SavedPet { variant: p.variant.clone(), x: p.brain.x, y: p.brain.y })
        .collect();
    let snapshot = {
        let mut s = state.settings.lock().unwrap();
        s.pets = pets;
        s.clone()
    };
    if let Err(e) = persist::save(&state.settings_path, &snapshot) {
        eprintln!("Couldn't save settings: {e}");
    }
}

fn step(app: &AppHandle, pet: &mut Pet, dt: f64, monitors: &[MonitorInfo], scale: u32, hidden: bool) {
    let g = ground_for(pet.brain.x, pet.brain.y, monitors).unwrap_or(Ground { left: 0.0, right: 1280.0, y: 800.0, px_per_pt: 1.0 });
    pet.brain.tick(dt, &g);
    let k = scale as f64 * g.px_per_pt;
    if (g.px_per_pt - pet.last_ppt).abs() > 1e-9 {
        let _ = pet.window.set_size(window_size(&pet.content, scale, g.px_per_pt));
        pet.last_ppt = g.px_per_pt;
    }
    let left = (pet.brain.x - (-pet.content.min_x) as f64 * k).round() as i32;
    let top = (pet.brain.y - pet.content.height() as f64 * k).round() as i32;
    if (left, top) != pet.last_pos {
        let _ = pet.window.set_position(PhysicalPosition::new(left, top));
        pet.last_pos = (left, top);
    }
    if !pet.shown && !hidden {
        let _ = pet.window.show();
        pet.shown = true;
    }
    let v = pet.brain.visual();
    if pet.last_visual.as_ref() != Some(&v) {
        let _ = app.emit_to(pet.label.as_str(), "pet:anim", &v);
        pet.last_visual = Some(v);
    }
}

pub fn start_loop(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last = Instant::now();
        let mut last_monitor_refresh = Instant::now() - Duration::from_secs(10);
        let mut last_save = Instant::now();
        loop {
            std::thread::sleep(Duration::from_millis(16));
            let now = Instant::now();
            let dt = (now - last).as_secs_f64().min(0.1);
            last = now;
            let state = app.state::<AppState>();
            if now - last_monitor_refresh > Duration::from_secs(1) {
                *state.monitors.lock().unwrap() = monitor_infos(&app);
                last_monitor_refresh = now;
            }
            let monitors = state.monitors.lock().unwrap().clone();
            let (scale, hidden) = {
                let s = state.settings.lock().unwrap();
                (s.scale, s.hidden)
            };
            for pet in state.pets.lock().unwrap().iter_mut() {
                step(&app, pet, dt, &monitors, scale, hidden);
            }
            if now - last_save > Duration::from_secs(5) {
                save_settings(&app);
                last_save = now;
            }
        }
    });
}

/// Respawns saved pets that still exist in the index.
pub fn restore(app: AppHandle) {
    let saved: Vec<SavedPet> = {
        let state = app.state::<AppState>();
        let s = state.settings.lock().unwrap();
        persist::restorable(&s, bundled()).into_iter().cloned().collect()
    };
    tauri::async_runtime::spawn(async move {
        for p in saved {
            if let Err(e) = spawn_pet(&app, &p.variant, Some((p.x, p.y))).await {
                eprintln!("Couldn't restore {}: {e}", p.variant);
            }
        }
    });
}
```

- [ ] **Step 2: Write `commands.rs` (pet commands)**

```rust
//! Tauri commands called from the pet, picker, credits and contact windows.

use crate::bounds::BoxI;
use crate::brain::Visual;
use crate::pets::{self, AppState};
use crate::animdata::AnimInfo;
use serde::Serialize;
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::{AppHandle, Manager, WebviewWindow};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PetInit {
    pub dir: String,
    pub table: Vec<AnimInfo>,
    pub content_box: BoxI,
    pub scale: u32,
    pub visual: Visual,
}

fn with_pet<T>(window: &WebviewWindow, f: impl FnOnce(&mut pets::Pet) -> T) -> Option<T> {
    let app = window.app_handle();
    let state = app.state::<AppState>();
    let mut pets = state.pets.lock().unwrap();
    pets.iter_mut().find(|p| p.label == window.label()).map(f)
}

fn cursor(window: &WebviewWindow) -> (f64, f64) {
    window.app_handle().cursor_position().map(|p| (p.x, p.y)).unwrap_or((0.0, 0.0))
}

#[tauri::command]
pub async fn pet_ready(window: WebviewWindow) -> Result<PetInit, String> {
    let scale = window.app_handle().state::<AppState>().settings.lock().unwrap().scale;
    with_pet(&window, |p| PetInit {
        dir: p.dir.to_string_lossy().into_owned(),
        table: p.table.anims.clone(),
        content_box: p.content,
        scale,
        visual: p.brain.visual(),
    })
    .ok_or_else(|| "pet not found".to_string())
}

#[tauri::command]
pub async fn pet_click(window: WebviewWindow) {
    with_pet(&window, |p| p.brain.click());
}

#[tauri::command]
pub async fn pet_drag_start(window: WebviewWindow) {
    let c = cursor(&window);
    with_pet(&window, |p| p.brain.drag_start(c));
}

#[tauri::command]
pub async fn pet_drag_move(window: WebviewWindow) {
    let c = cursor(&window);
    with_pet(&window, |p| p.brain.drag_move(c));
}

#[tauri::command]
pub async fn pet_drop(window: WebviewWindow) {
    with_pet(&window, |p| p.brain.drop_pet());
    pets::save_settings(window.app_handle());
}

#[tauri::command]
pub async fn pet_context_menu(window: WebviewWindow) -> Result<(), String> {
    let app = window.app_handle();
    let label = window.label();
    let menu = MenuBuilder::new(app)
        .item(&MenuItemBuilder::with_id(format!("pet-move:{label}"), "Do a move").build(app).map_err(|e| e.to_string())?)
        .item(&MenuItemBuilder::with_id(format!("pet-credits:{label}"), "Credits for this Pokémon").build(app).map_err(|e| e.to_string())?)
        .separator()
        .item(&MenuItemBuilder::with_id(format!("pet-remove:{label}"), "Remove").build(app).map_err(|e| e.to_string())?)
        .item(&MenuItemBuilder::with_id("pets-remove-all", "Remove all").build(app).map_err(|e| e.to_string())?)
        .separator()
        .item(&MenuItemBuilder::with_id("open-picker", "Open picker").build(app).map_err(|e| e.to_string())?)
        .build()
        .map_err(|e| e.to_string())?;
    window.popup_menu(&menu).map_err(|e| e.to_string())
}

/// Shared by menus: make a pet do its move.
pub fn pet_move(app: &AppHandle, label: &str) {
    let state = app.state::<AppState>();
    if let Some(p) = state.pets.lock().unwrap().iter_mut().find(|p| p.label == label) {
        p.brain.click();
    }
}
```

- [ ] **Step 3: Wire up `lib.rs`**

Replace `src-tauri/src/lib.rs`:
```rust
pub mod animdata;
pub mod bounds;
pub mod brain;
pub mod commands;
pub mod ground;
pub mod index;
pub mod persist;
pub mod pets;
pub mod sprites;

use tauri::{Manager, RunEvent};

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::pet_ready,
            commands::pet_click,
            commands::pet_drag_start,
            commands::pet_drag_move,
            commands::pet_drop,
            commands::pet_context_menu,
        ])
        .setup(|app| {
            let cache_root = app.path().app_data_dir()?;
            let settings_path = app.path().app_config_dir()?.join("settings.json");
            app.manage(pets::AppState::new(cache_root, settings_path));
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            app.on_menu_event(|app, event| {
                let id = event.id().as_ref();
                if let Some(label) = id.strip_prefix("pet-move:") {
                    commands::pet_move(app, label);
                } else if let Some(label) = id.strip_prefix("pet-remove:") {
                    pets::remove_pet(app, label);
                } else if id == "pets-remove-all" {
                    pets::remove_all(app);
                }
            });
            pets::start_loop(app.handle().clone());
            pets::restore(app.handle().clone());
            // Dev convenience until the picker exists: WANDERDEX_SPAWN=0202 pnpm tauri dev
            if let Ok(variant) = std::env::var("WANDERDEX_SPAWN") {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = pets::spawn_pet(&handle, &variant, None).await {
                        eprintln!("spawn failed: {e}");
                    }
                });
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Wanderdex")
        .run(|app, event| match event {
            RunEvent::ExitRequested { api, code, .. } if code.is_none() => api.prevent_exit(),
            RunEvent::Exit => pets::save_settings(app),
            _ => {}
        });
}
```

- [ ] **Step 4: Write the pet window runtime**

Replace `src/pet/main.ts`:
```ts
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { Player } from "../shared/player";
import { drawFrame, setupCanvas } from "../shared/renderer";
import { loadSheets } from "../shared/sheets";
import type { PetInit, Visual } from "../shared/types";

const DRAG_THRESHOLD_PX = 3;

async function main() {
  const canvas = document.getElementById("pet") as HTMLCanvasElement;
  const win = getCurrentWebviewWindow();
  const init = await invoke<PetInit>("pet_ready");
  const sheets = await loadSheets(init.dir, init.table);
  const player = new Player(init.table);
  let scale = init.scale;
  let ctx = setupCanvas(canvas, init.contentBox, scale, window.devicePixelRatio || 1);
  player.play(init.visual, performance.now());

  await win.listen<Visual>("pet:anim", (e) => player.play(e.payload, performance.now()));
  await win.listen<number>("pet:scale", (e) => {
    scale = e.payload;
    ctx = setupCanvas(canvas, init.contentBox, scale, window.devicePixelRatio || 1);
  });

  const render = (now: number) => {
    const f = player.frame(now);
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    if (f) {
      const img = sheets.get(f.anim.source);
      if (img) drawFrame(ctx, img, f.anim, f.dir, f.col, init.contentBox, scale);
    }
    requestAnimationFrame(render);
  };
  requestAnimationFrame(render);

  let down: { x: number; y: number } | null = null;
  let dragging = false;
  let movePending = false;
  canvas.addEventListener("pointerdown", (e) => {
    if (e.button !== 0) return;
    canvas.setPointerCapture(e.pointerId);
    down = { x: e.screenX, y: e.screenY };
    dragging = false;
  });
  canvas.addEventListener("pointermove", (e) => {
    if (!down) return;
    if (!dragging && Math.hypot(e.screenX - down.x, e.screenY - down.y) > DRAG_THRESHOLD_PX) {
      dragging = true;
      void invoke("pet_drag_start");
    }
    if (dragging && !movePending) {
      movePending = true;
      requestAnimationFrame(() => {
        movePending = false;
        void invoke("pet_drag_move");
      });
    }
  });
  canvas.addEventListener("pointerup", (e) => {
    if (e.button !== 0 || !down) return;
    canvas.releasePointerCapture(e.pointerId);
    void invoke(dragging ? "pet_drop" : "pet_click");
    down = null;
    dragging = false;
  });
  canvas.addEventListener("contextmenu", (e) => {
    e.preventDefault();
    void invoke("pet_context_menu");
  });
}

main().catch((err) => console.error("pet window failed", err));
```

- [ ] **Step 5: Run all automated checks**

Run: `pnpm test && pnpm typecheck && pnpm build && cargo test --manifest-path src-tauri/Cargo.toml`
Expected: all pass (TS 8 tests; Rust 42 tests).

- [ ] **Step 6: Manual check, one Wobbuffet**

Run: `WANDERDEX_SPAWN=0202 pnpm tauri dev`
Expected, within about 10 s (first run downloads ~170 KB):
- Wobbuffet appears on top of the Dock/taskbar, transparent around him, with no Dock icon (macOS).
- He walks left and right, idles facing you, and sometimes plays Withdraw (salute) or naps.
- Clicking him plays Withdraw. Dragging him faces the drag direction (spins when circled); releasing drops him to the ground with a hop.
- Right-click shows the menu. "Do a move" works, "Remove" closes him.
- Quit with Ctrl+C, run again: he's restored.
- **If clicks don't register** on macOS, remove `.focusable(false)` in `spawn_pet`, re-test, and note it in the commit message.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src src/pet
git commit -m "Spawn pets in transparent windows driven by a 60 Hz behavior loop"
```

---

### Task 9: Tray menu, size, hide/show, launch at login

**Files:**
- Create: `src-tauri/src/tray.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `pets::{set_scale, set_hidden, remove_all, save_settings, AppState}`.
- Produces: `tray::build_tray(app: &AppHandle) -> tauri::Result<()>`, `tray::refresh_tray(app: &AppHandle)`, `tray::handle_menu(app: &AppHandle, id: &str) -> bool` (true if handled), `tray::open_picker(app: &AppHandle)`, `tray::open_credits(app: &AppHandle)`. Menu ids: `open-picker`, `size-2`, `size-3`, `size-4`, `toggle-hidden`, `autostart`, `credits`, `quit`.

- [ ] **Step 1: Write `tray.rs`**

```rust
//! Menu-bar / tray icon and its menu; opens the picker and credits windows.

use crate::pets::{self, AppState};
use crate::persist::SCALES;
use tauri::menu::{CheckMenuItemBuilder, Menu, MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_autostart::ManagerExt;

const TRAY_ID: &str = "main";

fn menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let (scale, hidden) = {
        let s = app.state::<AppState>().settings.lock().unwrap().clone();
        (s.scale, s.hidden)
    };
    let mut size = SubmenuBuilder::new(app, "Size");
    for s in SCALES {
        size = size.item(&CheckMenuItemBuilder::with_id(format!("size-{s}"), format!("{s}×")).checked(s == scale).build(app)?);
    }
    let autostart = app.autolaunch().is_enabled().unwrap_or(false);
    MenuBuilder::new(app)
        .item(&MenuItemBuilder::with_id("open-picker", "Open picker").build(app)?)
        .item(&size.build()?)
        .item(&MenuItemBuilder::with_id("toggle-hidden", if hidden { "Show all" } else { "Hide all" }).build(app)?)
        .item(&CheckMenuItemBuilder::with_id("autostart", "Launch at login").checked(autostart).build(app)?)
        .item(&MenuItemBuilder::with_id("credits", "Credits & license").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("quit", "Quit Wanderdex").build(app)?)
        .build()
}

pub fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(app.default_window_icon().expect("bundle icon").clone())
        .tooltip("Wanderdex")
        .menu(&menu(app)?)
        .show_menu_on_left_click(true)
        .build(app)?;
    Ok(())
}

pub fn refresh_tray(app: &AppHandle) {
    if let (Some(tray), Ok(m)) = (app.tray_by_id(TRAY_ID), menu(app)) {
        let _ = tray.set_menu(Some(m));
    }
}

fn open_window(app: &AppHandle, label: &str, page: &str, title: &str, size: (f64, f64)) {
    if let Some(w) = app.get_webview_window(label) {
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let _ = WebviewWindowBuilder::new(app, label, WebviewUrl::App(page.into()))
        .title(title)
        .inner_size(size.0, size.1)
        .min_inner_size(640.0, 480.0)
        .center()
        .focused(true)
        .build();
}

pub fn open_picker(app: &AppHandle) {
    open_window(app, "picker", "index.html", "Wanderdex", (980.0, 700.0));
}

pub fn open_credits(app: &AppHandle) {
    open_window(app, "credits", "credits.html", "Wanderdex Credits", (640.0, 600.0));
}

/// Handles tray and shared menu ids. Returns false for ids it doesn't own.
pub fn handle_menu(app: &AppHandle, id: &str) -> bool {
    match id {
        "open-picker" => open_picker(app),
        "credits" => open_credits(app),
        "toggle-hidden" => {
            let hidden = app.state::<AppState>().settings.lock().unwrap().hidden;
            pets::set_hidden(app, !hidden);
            refresh_tray(app);
        }
        "autostart" => {
            let al = app.autolaunch();
            let _ = if al.is_enabled().unwrap_or(false) { al.disable() } else { al.enable() };
            refresh_tray(app);
        }
        "quit" => {
            pets::save_settings(app);
            app.exit(0);
        }
        _ => match id.strip_prefix("size-").and_then(|s| s.parse::<u32>().ok()).filter(|s| SCALES.contains(s)) {
            Some(scale) => {
                pets::set_scale(app, scale);
                refresh_tray(app);
            }
            None => return false,
        },
    }
    true
}
```

- [ ] **Step 2: Wire the tray, autostart plugin and menu routing into `lib.rs`**

In `src-tauri/src/lib.rs`:
- Add `pub mod tray;`.
- Add `.plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))` before `.invoke_handler`.
- In `setup`, after `app.manage(...)`, add `tray::build_tray(app.handle())?;`.
- Change the menu handler's first branch so tray/shared ids route first:

```rust
            app.on_menu_event(|app, event| {
                let id = event.id().as_ref();
                if tray::handle_menu(app, id) {
                    return;
                }
                if let Some(label) = id.strip_prefix("pet-move:") {
                    commands::pet_move(app, label);
                } else if let Some(label) = id.strip_prefix("pet-remove:") {
                    pets::remove_pet(app, label);
                } else if id == "pets-remove-all" {
                    pets::remove_all(app);
                }
            });
```

- [ ] **Step 3: Build and run automated checks**

Run: `pnpm build && cargo test --manifest-path src-tauri/Cargo.toml`
Expected: all pass.

- [ ] **Step 4: Manual check**

Run: `WANDERDEX_SPAWN=0202 pnpm tauri dev`
Expected:
- A tray (Windows) or menu-bar (macOS) icon appears.
- Size → 4× makes Wobbuffet bigger and crisp, with feet still on the ground. 2× makes him smaller. The checkmark follows.
- Hide all hides him and the label changes to Show all; Show all brings him back.
- Launch at login toggles its checkmark (macOS: a LaunchAgent plist appears in `~/Library/LaunchAgents`).
- "Open picker" opens the placeholder "Wanderdex" window. Closing it doesn't quit the app.
- Quit Wanderdex exits. On relaunch, size and pets are restored.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src
git commit -m "Add tray menu with size, hide/show, launch at login and quit"
```

---

### Task 10: Picker window

**Files:**
- Modify: `src-tauri/src/commands.rs` (picker commands), `src-tauri/src/lib.rs` (register them)
- Create: `src/picker/api.ts`, `src/picker/App.tsx`, `src/picker/SpeciesGrid.tsx`, `src/picker/DetailPanel.tsx`, `src/picker/Preview.tsx`, `src/picker/DesktopStrip.tsx`, `src/picker/picker.css`
- Modify: `src/picker/main.tsx`

**Interfaces:**
- Consumes: `index::{bundled, find_variant, signature_for}`, `sprites::{ensure_variant, ensure_portrait}`, `bounds::content_box`, `brain::behavior_anims`, `pets::{spawn_pet, remove_pet, remove_all}`; TS `Player`, `drawFrame`, `setupCanvas`, `loadSheets`.
- Produces (Rust commands): `get_index() -> Index`, `ensure_portrait_cmd(dex: String) -> Result<String, String>` (path), `load_variant(path: String) -> Result<VariantPayload, String>`, `spawn(variant: String) -> Result<(), String>`, `list_pets() -> Vec<ActivePet>`, `remove_pet_cmd(label: String)`, `remove_all_cmd()`.
- Produces (TS): `api.ts` exports `getIndex`, `portraitSrc(dex)`, `loadVariant(path)`, `spawn(variant)`, `listPets()`, `removePet(label)`, `removeAll()`, `onPetsChanged(cb)`.

- [ ] **Step 1: Add the picker commands**

Append to `src-tauri/src/commands.rs`:
```rust
use crate::index::{bundled, find_variant, signature_for, Index};
use crate::sprites::{ensure_portrait, ensure_variant};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VariantPayload {
    pub dir: String,
    pub table: Vec<AnimInfo>,
    pub content_box: BoxI,
    pub signature: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivePet {
    pub label: String,
    pub variant: String,
    pub dex: String,
    pub name: String,
    pub form: String,
    pub shiny: bool,
    pub female: bool,
}

#[tauri::command]
pub fn get_index() -> Index {
    bundled().clone()
}

#[tauri::command]
pub async fn ensure_portrait_cmd(app: AppHandle, dex: String) -> Result<String, String> {
    let state = app.state::<AppState>();
    let path = ensure_portrait(&state.http, &state.cache_root, &bundled().sprite_collab_sha, &dex)
        .await
        .map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn load_variant(app: AppHandle, path: String) -> Result<VariantPayload, String> {
    let (species, _) = find_variant(bundled(), &path).ok_or_else(|| format!("Unknown variant {path}"))?;
    let signature = signature_for(&species.dex).map(String::from);
    let state = app.state::<AppState>();
    let v = ensure_variant(&state.http, &state.cache_root, &bundled().sprite_collab_sha, &path)
        .await
        .map_err(|e| e.to_string())?;
    let content_box = crate::bounds::content_box(&v.table, &v.sheets, &crate::brain::behavior_anims(signature.as_deref()));
    Ok(VariantPayload { dir: v.dir.to_string_lossy().into_owned(), table: v.table.anims, content_box, signature })
}

#[tauri::command]
pub async fn spawn(app: AppHandle, variant: String) -> Result<(), String> {
    pets::spawn_pet(&app, &variant, None).await
}

#[tauri::command]
pub fn list_pets(app: AppHandle) -> Vec<ActivePet> {
    let state = app.state::<AppState>();
    let pets = state.pets.lock().unwrap();
    pets.iter()
        .filter_map(|p| {
            let (s, v) = find_variant(bundled(), &p.variant)?;
            Some(ActivePet {
                label: p.label.clone(),
                variant: p.variant.clone(),
                dex: s.dex.clone(),
                name: s.name.clone(),
                form: v.form.clone(),
                shiny: v.shiny,
                female: v.female,
            })
        })
        .collect()
}

#[tauri::command]
pub fn remove_pet_cmd(app: AppHandle, label: String) {
    pets::remove_pet(&app, &label);
}

#[tauri::command]
pub fn remove_all_cmd(app: AppHandle) {
    pets::remove_all(&app);
}
```
Register in `lib.rs` `generate_handler!`: `commands::get_index, commands::ensure_portrait_cmd, commands::load_variant, commands::spawn, commands::list_pets, commands::remove_pet_cmd, commands::remove_all_cmd`.

- [ ] **Step 2: Write `src/picker/api.ts`**

```ts
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ActivePet, Index, VariantPayload } from "../shared/types";

export const getIndex = () => invoke<Index>("get_index");
export const portraitSrc = async (dex: string) => convertFileSrc(await invoke<string>("ensure_portrait_cmd", { dex }));
export const loadVariant = (path: string) => invoke<VariantPayload>("load_variant", { path });
export const spawn = (variant: string) => invoke<void>("spawn", { variant });
export const listPets = () => invoke<ActivePet[]>("list_pets");
export const removePet = (label: string) => invoke<void>("remove_pet_cmd", { label });
export const removeAll = () => invoke<void>("remove_all_cmd");
export const onPetsChanged = (cb: () => void) => listen("pets:changed", cb);
```

- [ ] **Step 3: Write the components**

`src/picker/SpeciesGrid.tsx`:
```tsx
import { useEffect, useRef, useState } from "react";
import { portraitSrc } from "./api";
import type { Species } from "../shared/types";

function Tile({ s, selected, onSelect }: { s: Species; selected: boolean; onSelect: () => void }) {
  const ref = useRef<HTMLButtonElement>(null);
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    if (!s.hasPortrait || !ref.current) return;
    const io = new IntersectionObserver(([entry]) => {
      if (entry.isIntersecting) {
        io.disconnect();
        portraitSrc(s.dex).then(setSrc).catch(() => setSrc(null));
      }
    });
    io.observe(ref.current);
    return () => io.disconnect();
  }, [s.dex, s.hasPortrait]);
  return (
    <button ref={ref} className={`tile${selected ? " selected" : ""}`} onClick={onSelect}>
      {src ? <img src={src} alt="" width={80} height={80} /> : <div className="noportrait">#{Number(s.dex)}</div>}
      <span className="name">{s.name}</span>
      <span className="dex">#{s.dex}</span>
    </button>
  );
}

export function SpeciesGrid({ species, selected, onSelect }: { species: Species[]; selected: string | null; onSelect: (dex: string) => void }) {
  return (
    <div className="grid">
      {species.map((s) => (
        <Tile key={s.dex} s={s} selected={s.dex === selected} onSelect={() => onSelect(s.dex)} />
      ))}
    </div>
  );
}
```
`src/picker/Preview.tsx`:
```tsx
import { useEffect, useRef } from "react";
import { Player } from "../shared/player";
import { drawFrame, setupCanvas } from "../shared/renderer";
import { loadSheets } from "../shared/sheets";
import { DIR, type VariantPayload, type Visual } from "../shared/types";

const FLOURISHES = ["Pose", "Hop", "Rotate", "Charge", "Double", "Swing", "Attack"];
const PREVIEW_SCALE = 3;

/** Cycles Idle → Walk → signature/flourish using the same renderer as the pets. */
export function Preview({ payload }: { payload: VariantPayload }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    let alive = true;
    let raf = 0;
    const has = (n: string) => payload.table.some((a) => a.name === n);
    const move = payload.signature && has(payload.signature) ? payload.signature : FLOURISHES.find(has);
    const steps: { v: Omit<Visual, "seq">; ms: number }[] = [
      { v: { anim: "Idle", dir: DIR.Down, looped: true }, ms: 2500 },
      { v: { anim: "Walk", dir: DIR.Right, looped: true }, ms: 2500 },
    ];
    if (move) {
      const secs = payload.table.find((a) => a.name === move)!.durations.reduce((a, b) => a + b, 0) / 60;
      steps.push({ v: { anim: move, dir: DIR.Down, looped: false }, ms: Math.max(1000, secs * 1000 + 500) });
    }
    loadSheets(payload.dir, payload.table).then((sheets) => {
      if (!alive || !canvasRef.current) return;
      const canvas = canvasRef.current;
      const ctx = setupCanvas(canvas, payload.contentBox, PREVIEW_SCALE, window.devicePixelRatio || 1);
      const player = new Player(payload.table);
      let step = 0;
      let seq = 0;
      let stepStart = performance.now();
      player.play({ ...steps[0].v, seq: seq++ }, stepStart);
      const render = (now: number) => {
        if (now - stepStart > steps[step].ms) {
          step = (step + 1) % steps.length;
          stepStart = now;
          player.play({ ...steps[step].v, seq: seq++ }, now);
        }
        ctx.clearRect(0, 0, canvas.width, canvas.height);
        const f = player.frame(now);
        const img = f && sheets.get(f.anim.source);
        if (f && img) drawFrame(ctx, img, f.anim, f.dir, f.col, payload.contentBox, PREVIEW_SCALE);
        raf = requestAnimationFrame(render);
      };
      raf = requestAnimationFrame(render);
    });
    return () => {
      alive = false;
      cancelAnimationFrame(raf);
    };
  }, [payload]);
  return <canvas ref={canvasRef} className="preview" />;
}
```
`src/picker/DetailPanel.tsx`:
```tsx
import { useEffect, useMemo, useState } from "react";
import { loadVariant, spawn } from "./api";
import { Preview } from "./Preview";
import type { Species, VariantPayload } from "../shared/types";

export function DetailPanel({ species }: { species: Species }) {
  const forms = useMemo(() => [...new Map(species.variants.map((v) => [v.formId, v.form])).entries()], [species]);
  const [formId, setFormId] = useState(forms[0][0]);
  const [shiny, setShiny] = useState(false);
  const [female, setFemale] = useState(false);
  const [payload, setPayload] = useState<VariantPayload | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    setFormId(forms[0][0]);
    setShiny(false);
    setFemale(false);
  }, [species, forms]);

  const inForm = species.variants.filter((v) => v.formId === formId);
  const canShiny = inForm.some((v) => v.shiny);
  const canFemale = inForm.some((v) => v.female);
  const variant =
    inForm.find((v) => v.shiny === (shiny && canShiny) && v.female === (female && canFemale)) ??
    inForm.find((v) => v.shiny === (shiny && canShiny)) ??
    inForm[0];

  useEffect(() => {
    let alive = true;
    setPayload(null);
    setError(null);
    loadVariant(variant.path)
      .then((p) => alive && setPayload(p))
      .catch((e) => alive && setError(String(e)));
    return () => {
      alive = false;
    };
  }, [variant.path, attempt]);

  const doSpawn = async () => {
    setBusy(true);
    setError(null);
    try {
      await spawn(variant.path);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <aside className="detail">
      <h2>
        {species.name} <span className="dex">#{species.dex}</span>
      </h2>
      <div className="stage">{payload ? <Preview payload={payload} /> : !error && <div className="loading">Loading…</div>}</div>
      {forms.length > 1 && (
        <label>
          Form{" "}
          <select value={formId} onChange={(e) => setFormId(e.target.value)}>
            {forms.map(([id, name]) => (
              <option key={id} value={id}>{name}</option>
            ))}
          </select>
        </label>
      )}
      {canShiny && (
        <label><input type="checkbox" checked={shiny} onChange={(e) => setShiny(e.target.checked)} /> Shiny</label>
      )}
      {canFemale && (
        <label><input type="checkbox" checked={female} onChange={(e) => setFemale(e.target.checked)} /> Female</label>
      )}
      <p className="credits">Sprites: {variant.credits.length ? variant.credits.join(", ") : "Uncredited"}</p>
      {error && (
        <p className="error">
          Couldn't download sprites. Check your connection. <button onClick={() => setAttempt((a) => a + 1)}>Retry</button>
        </p>
      )}
      <button className="spawn" disabled={busy || !payload} onClick={doSpawn}>
        {busy ? "Spawning…" : "Spawn"}
      </button>
    </aside>
  );
}
```
`src/picker/DesktopStrip.tsx`:
```tsx
import { useEffect, useState } from "react";
import { listPets, onPetsChanged, portraitSrc, removeAll, removePet } from "./api";
import type { ActivePet } from "../shared/types";

function PetChip({ pet }: { pet: ActivePet }) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    portraitSrc(pet.dex).then(setSrc).catch(() => setSrc(null));
  }, [pet.dex]);
  const label = `${pet.shiny ? "Shiny " : ""}${pet.name}${pet.form !== "Normal" ? ` (${pet.form})` : ""}`;
  return (
    <button className="chip" title={`Remove ${label}`} onClick={() => removePet(pet.label)}>
      {src ? <img src={src} alt="" width={40} height={40} /> : <span>#{pet.dex}</span>}
    </button>
  );
}

export function DesktopStrip() {
  const [pets, setPets] = useState<ActivePet[]>([]);
  useEffect(() => {
    const refresh = () => listPets().then(setPets);
    refresh();
    const un = onPetsChanged(refresh);
    return () => {
      un.then((f) => f());
    };
  }, []);
  return (
    <footer className="strip">
      <span>On your desktop:</span>
      {pets.length === 0 && <em>nobody yet</em>}
      {pets.map((p) => <PetChip key={p.label} pet={p} />)}
      {pets.length > 0 && <button className="removeall" onClick={removeAll}>Remove all</button>}
    </footer>
  );
}
```
`src/picker/App.tsx`:
```tsx
import { useEffect, useMemo, useState } from "react";
import { getIndex } from "./api";
import { DesktopStrip } from "./DesktopStrip";
import { DetailPanel } from "./DetailPanel";
import { SpeciesGrid } from "./SpeciesGrid";
import type { Index } from "../shared/types";

export function App() {
  const [index, setIndex] = useState<Index | null>(null);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  useEffect(() => {
    getIndex().then(setIndex);
  }, []);
  const shown = useMemo(() => {
    if (!index) return [];
    const q = query.trim().toLowerCase();
    if (!q) return index.species;
    const n = Number(q);
    return index.species.filter((s) => s.name.toLowerCase().includes(q) || (!Number.isNaN(n) && Number(s.dex) === n));
  }, [index, query]);
  const species = index?.species.find((s) => s.dex === selected) ?? null;
  return (
    <div className="app">
      <header>
        <h1>Wanderdex</h1>
        <input autoFocus placeholder="Search by name or number" value={query} onChange={(e) => setQuery(e.target.value)} />
      </header>
      <main>
        {index ? <SpeciesGrid species={shown} selected={selected} onSelect={setSelected} /> : <p className="loading">Loading…</p>}
        {species && <DetailPanel key={species.dex} species={species} />}
      </main>
      <DesktopStrip />
    </div>
  );
}
```
`src/picker/main.tsx`:
```tsx
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./picker.css";

createRoot(document.getElementById("root")!).render(<App />);
```
`src/picker/picker.css`:
```css
:root { --bg: #fff8f0; --card: #ffffff; --ink: #2b2b3a; --muted: #8a8698; --accent: #4a90d9; --line: #eadfd3; color-scheme: light dark; }
@media (prefers-color-scheme: dark) { :root { --bg: #1d1b26; --card: #282534; --ink: #f1eef8; --muted: #a39fb3; --line: #3a3648; } }
* { box-sizing: border-box; }
body { margin: 0; background: var(--bg); color: var(--ink); font: 14px/1.4 -apple-system, "Segoe UI", system-ui, sans-serif; }
.app { display: grid; grid-template-rows: auto 1fr auto; height: 100vh; }
header { display: flex; gap: 16px; align-items: center; padding: 12px 16px; border-bottom: 1px solid var(--line); }
header h1 { margin: 0; font-size: 20px; }
header input { flex: 1; padding: 8px 12px; border-radius: 999px; border: 1px solid var(--line); background: var(--card); color: var(--ink); font-size: 14px; }
main { display: grid; grid-template-columns: 1fr 300px; min-height: 0; }
main:has(> .grid:only-child) { grid-template-columns: 1fr; }
.grid { overflow-y: auto; padding: 16px; display: grid; grid-template-columns: repeat(auto-fill, minmax(104px, 1fr)); gap: 10px; align-content: start; }
.tile { display: flex; flex-direction: column; align-items: center; gap: 2px; padding: 8px; border-radius: 14px; border: 2px solid transparent; background: var(--card); color: var(--ink); cursor: pointer; }
.tile:hover { border-color: var(--line); }
.tile.selected { border-color: var(--accent); }
.tile img { image-rendering: pixelated; border-radius: 10px; }
.noportrait { width: 80px; height: 80px; display: grid; place-items: center; color: var(--muted); border-radius: 10px; background: var(--bg); }
.name { font-weight: 600; font-size: 12px; text-align: center; }
.dex { color: var(--muted); font-size: 11px; font-weight: 400; }
.detail { border-left: 1px solid var(--line); padding: 16px; display: flex; flex-direction: column; gap: 10px; overflow-y: auto; }
.detail h2 { margin: 0; }
.stage { min-height: 200px; display: grid; place-items: center; background: var(--card); border-radius: 14px; }
.preview { image-rendering: pixelated; }
.credits { color: var(--muted); font-size: 12px; margin: 0; }
.error { color: #d9534f; margin: 0; }
.spawn { padding: 10px; border-radius: 999px; border: none; background: var(--accent); color: white; font-weight: 700; font-size: 15px; cursor: pointer; }
.spawn:disabled { opacity: 0.5; cursor: default; }
.strip { display: flex; gap: 8px; align-items: center; padding: 8px 16px; border-top: 1px solid var(--line); color: var(--muted); min-height: 56px; }
.chip { border: none; background: var(--card); border-radius: 10px; padding: 0; cursor: pointer; }
.chip img { image-rendering: pixelated; display: block; border-radius: 10px; }
.removeall { margin-left: auto; border: 1px solid var(--line); background: transparent; color: var(--ink); border-radius: 999px; padding: 4px 12px; cursor: pointer; }
.loading { color: var(--muted); padding: 16px; }
```

- [ ] **Step 4: Run automated checks**

Run: `pnpm typecheck && pnpm test && pnpm build && cargo test --manifest-path src-tauri/Cargo.toml`
Expected: all pass.

- [ ] **Step 5: Manual check**

Run: `pnpm tauri dev`, then use the tray menu's "Open picker".
Expected:
- The grid shows about 980 species, and portraits fade in as you scroll.
- Searching "wob" or "202" shows Wobbuffet only.
- Selecting Wobbuffet shows the preview cycling Idle → Walk → Withdraw. Shiny and Female toggles appear, and the preview changes color for Shiny.
- Credits read "Sprites: CHUNSOFT" for the base and "Tacocoa, Noo" for Shiny.
- Spawn adds him to the desktop and to the "On your desktop" strip. Clicking the chip removes him.
- Rattata shows a Form dropdown with Alola. Pikachu shows multiple forms.
- Turn off Wi-Fi and pick a never-spawned species: the error with Retry shows. Wobbuffet (cached) still spawns.

- [ ] **Step 6: Commit**

```bash
git add src/picker src-tauri/src
git commit -m "Add picker: searchable grid, variant panel with live preview, desktop strip"
```

---

### Task 11: Credits & license window

**Files:**
- Modify: `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`
- Modify: `src/credits/main.tsx`

**Interfaces:**
- Consumes: `persist::Settings.credited`, `index::{bundled, find_variant}`, `tray::open_credits`.
- Produces: command `get_credits() -> Vec<CreditEntry { name, form, shiny, female, credits }>`. The pet menu id `pet-credits:<label>` opens the credits window.

- [ ] **Step 1: Add the command and menu route**

Append to `src-tauri/src/commands.rs`:
```rust
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreditEntry {
    pub name: String,
    pub form: String,
    pub shiny: bool,
    pub female: bool,
    pub credits: Vec<String>,
}

#[tauri::command]
pub fn get_credits(app: AppHandle) -> Vec<CreditEntry> {
    let credited = app.state::<AppState>().settings.lock().unwrap().credited.clone();
    credited
        .iter()
        .filter_map(|path| {
            let (s, v) = find_variant(bundled(), path)?;
            Some(CreditEntry { name: s.name.clone(), form: v.form.clone(), shiny: v.shiny, female: v.female, credits: v.credits.clone() })
        })
        .collect()
}
```
Register `commands::get_credits` in `generate_handler!`. In the `on_menu_event` closure in `lib.rs`, add a branch before the `pet-remove:` check:
```rust
                } else if id.starts_with("pet-credits:") {
                    tray::open_credits(app);
```

- [ ] **Step 2: Write the credits page**

Replace `src/credits/main.tsx`:
```tsx
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import type { CreditEntry } from "../shared/types";
import "../picker/picker.css";

function Credits() {
  const [entries, setEntries] = useState<CreditEntry[]>([]);
  useEffect(() => {
    invoke<CreditEntry[]>("get_credits").then(setEntries);
  }, []);
  return (
    <div style={{ padding: 24, overflowY: "auto", height: "100vh" }}>
      <h1>Credits &amp; license</h1>
      <p>
        Pokémon sprites come from the <a href="https://github.com/PMDCollab/SpriteCollab" target="_blank">PMD Sprite Repository (SpriteCollab)</a> and
        are licensed under <a href="https://creativecommons.org/licenses/by-nc/4.0/" target="_blank">CC BY-NC 4.0</a>. They may not be used commercially.
      </p>
      <p>Wanderdex is a non-commercial fan project. Pokémon is © Nintendo / Creatures Inc. / GAME FREAK inc. Wanderdex is not affiliated with or endorsed by them.</p>
      <h2>Artists for Pokémon you've spawned</h2>
      {entries.length === 0 && <p className="credits">Spawn a Pokémon to see its artists here.</p>}
      <ul>
        {entries.map((e, i) => (
          <li key={i}>
            <strong>{e.shiny ? "Shiny " : ""}{e.name}</strong>
            {e.form !== "Normal" ? ` (${e.form})` : ""}
            {e.female ? " ♀" : ""}: {e.credits.length ? e.credits.join(", ") : "Uncredited"}
          </li>
        ))}
      </ul>
    </div>
  );
}

createRoot(document.getElementById("root")!).render(<Credits />);
```

- [ ] **Step 3: Run checks and verify manually**

Run: `pnpm typecheck && pnpm build && cargo test --manifest-path src-tauri/Cargo.toml`, then `pnpm tauri dev`.
Expected: after spawning Wobbuffet and Shiny Wobbuffet, tray → Credits & license lists "Wobbuffet: CHUNSOFT" and "Shiny Wobbuffet: Tacocoa, Noo". A pet's right-click "Credits for this Pokémon" opens the same window. The links open in the default browser.

- [ ] **Step 4: Commit**

```bash
git add src/credits src-tauri/src
git commit -m "Add credits & license window"
```

---

### Task 12: Contact-sheet rendering check

**Files:**
- Modify: `src/contact/main.ts`, `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `load_variant` (Task 10), `frameSource`/`frameDest`/`drawFrame` (Task 5).
- Produces: `wanderdex --contact <variant>` opens `contact.html?variant=<variant>`. That page renders frame 0, row 0 of every animation at 4×, checks every 4×4 block is one color, and invokes `save_contact_sheet(dataUrl, report, ok)`. That writes `contact-sheet.png` to the current directory, prints the report, and exits with code 0 (pass) or 1 (fail).

- [ ] **Step 1: Add the command and the `--contact` flag**

Append to `src-tauri/src/commands.rs`:
```rust
#[tauri::command]
pub fn save_contact_sheet(app: AppHandle, data_url: String, report: String, ok: bool) -> Result<(), String> {
    use std::io::Write;
    let b64 = data_url.split_once(',').map(|(_, b)| b).ok_or("bad data URL")?;
    let bytes = base64_decode(b64)?;
    std::fs::write("contact-sheet.png", bytes).map_err(|e| e.to_string())?;
    println!("{report}");
    let _ = std::io::stdout().flush();
    app.exit(if ok { 0 } else { 1 });
    Ok(())
}

fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut buf, mut bits) = (0u32, 0u32);
    for c in s.bytes().filter(|c| *c != b'=') {
        let v = ALPHABET.iter().position(|a| *a == c).ok_or("bad base64")? as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Ok(out)
}
```
Register `commands::save_contact_sheet`. In `lib.rs` `setup`, before `tray::build_tray`, add:
```rust
            let args: Vec<String> = std::env::args().collect();
            if let Some(i) = args.iter().position(|a| a == "--contact") {
                let variant = args.get(i + 1).cloned().unwrap_or_else(|| "0202".into());
                tauri::WebviewWindowBuilder::new(app, "contact", tauri::WebviewUrl::App(format!("contact.html?variant={variant}").into()))
                    .title("Contact sheet")
                    .inner_size(1400.0, 900.0)
                    .build()?;
                return Ok(());
            }
```
(Contact mode skips the tray, restore and loop.)

- [ ] **Step 2: Write the contact page**

Replace `src/contact/main.ts`:
```ts
import { invoke } from "@tauri-apps/api/core";
import { loadSheets } from "../shared/sheets";
import { drawFrame, frameDest } from "../shared/renderer";
import type { VariantPayload } from "../shared/types";

const SCALE = 4;
const PAD = 12;
const LABEL = 18;
const COLUMNS = 5;

async function main() {
  const variant = new URLSearchParams(location.search).get("variant") ?? "0202";
  const payload = await invoke<VariantPayload>("load_variant", { path: variant });
  const sheets = await loadSheets(payload.dir, payload.table);
  const cellW = Math.max(...payload.table.map((a) => a.frameWidth)) * SCALE + PAD * 2;
  const cellH = Math.max(...payload.table.map((a) => a.frameHeight)) * SCALE + PAD * 2 + LABEL;
  const rows = Math.ceil(payload.table.length / COLUMNS);
  const canvas = document.getElementById("sheet") as HTMLCanvasElement;
  canvas.width = cellW * COLUMNS;
  canvas.height = cellH * rows;
  const ctx = canvas.getContext("2d", { willReadFrequently: true })!; // 1 canvas px = 1 CSS px: exact checks
  ctx.fillStyle = "#fff";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  const lines: string[] = [`Contact sheet for ${variant} (${payload.table.length} animations, ${SCALE}x)`];
  let ok = true;

  payload.table.forEach((anim, i) => {
    const x0 = (i % COLUMNS) * cellW + PAD;
    const y0 = Math.floor(i / COLUMNS) * cellH + PAD;
    const fw = anim.frameWidth * SCALE;
    const fh = anim.frameHeight * SCALE;
    ctx.fillStyle = "#eee";
    ctx.fillRect(x0, y0, fw, fh);
    // Draw with the app renderer, using a box whose anchor puts the frame exactly in this cell.
    const box = { minX: -anim.frameWidth / 2, minY: -anim.frameHeight / 2, maxX: anim.frameWidth / 2, maxY: anim.frameHeight / 2 };
    const d = frameDest(anim, box, SCALE);
    ctx.save();
    ctx.translate(x0 - d.dx, y0 - d.dy);
    drawFrame(ctx, sheets.get(anim.source)!, anim, 0, 0, box, SCALE);
    ctx.restore();
    ctx.strokeStyle = "#d33";
    ctx.strokeRect(x0 - 0.5, y0 - 0.5, fw + 1, fh + 1);
    ctx.fillStyle = "#000";
    ctx.font = "12px system-ui";
    ctx.fillText(`${anim.name}${anim.source !== anim.name ? ` → ${anim.source}` : ""} ${anim.frameWidth}×${anim.frameHeight} rows ${anim.rows}`, x0, y0 + fh + 14);

    const px = ctx.getImageData(x0, y0, fw, fh).data;
    let off = 0;
    for (let y = 0; y < fh; y++) {
      for (let x = 0; x < fw; x++) {
        const a = (y * fw + x) * 4;
        const b = ((y - (y % SCALE)) * fw + (x - (x % SCALE))) * 4;
        if (px[a] !== px[b] || px[a + 1] !== px[b + 1] || px[a + 2] !== px[b + 2] || px[a + 3] !== px[b + 3]) off++;
      }
    }
    if (off > 0) ok = false;
    lines.push(`${anim.name.padEnd(10)} ${off === 0 ? "crisp" : `${off} non-uniform pixels`}`);
  });
  lines.push(ok ? "PASS" : "FAIL");
  document.getElementById("report")!.textContent = lines.join("\n");
  await invoke("save_contact_sheet", { dataUrl: canvas.toDataURL("image/png"), report: lines.join("\n"), ok });
}

main().catch((e) => {
  document.getElementById("report")!.textContent = String(e);
  void invoke("save_contact_sheet", { dataUrl: "data:,", report: `ERROR ${e}`, ok: false });
});
```

- [ ] **Step 3: Run the contact sheet for Wobbuffet and for a single-row/alt-form variant**

Run: `pnpm contact-sheet 0202`
Expected: stdout lists 14 animations, each `crisp`, then `PASS`; exit code 0; `contact-sheet.png` exists. Open it: every sprite sits fully inside its red frame outline, with feet at a consistent height.

Run: `./src-tauri/target/debug/wanderdex --contact 0019/0001`
Expected: `PASS` for Alolan Rattata.

- [ ] **Step 4: Commit**

```bash
git add src/contact src-tauri/src
git commit -m "Add contact-sheet rendering check (--contact)"
```

---

### Task 13: Releases, README, and retiring the Swift prototype

**Files:**
- Create: `.github/workflows/release.yml`, `README.md`, `LICENSE`
- Modify: `CREDITS.md`, `.gitignore`
- Delete: `Sources/`, `build.sh`, `ThirdParty/`

**Interfaces:**
- Consumes: everything above.
- Produces: tag-triggered draft releases with a universal macOS `.dmg` and Windows installers.

- [ ] **Step 1: Write the release workflow**

`.github/workflows/release.yml`:
```yaml
name: Release
on:
  push:
    tags: ["v*"]
permissions:
  contents: write
jobs:
  build:
    strategy:
      fail-fast: false
      matrix:
        include:
          - platform: macos-latest
            args: --target universal-apple-darwin
          - platform: windows-latest
            args: ""
    runs-on: ${{ matrix.platform }}
    steps:
      - uses: actions/checkout@v4
      - uses: pnpm/action-setup@v4
        with: { version: 10 }
      - uses: actions/setup-node@v4
        with: { node-version: 22, cache: pnpm }
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.platform == 'macos-latest' && 'aarch64-apple-darwin,x86_64-apple-darwin' || '' }}
      - uses: Swatinem/rust-cache@v2
        with: { workspaces: src-tauri }
      - run: pnpm install --frozen-lockfile
      - uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          tagName: ${{ github.ref_name }}
          releaseName: "Wanderdex ${{ github.ref_name }}"
          releaseBody: "Download the .dmg (macOS) or the -setup.exe (Windows). See the README for first-launch steps on unsigned apps."
          releaseDraft: true
          args: ${{ matrix.args }}
```

- [ ] **Step 2: Write `LICENSE` (MIT) and `README.md`**

`LICENSE`:
```
MIT License

Copyright (c) 2026 Wanderdex contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

`README.md`:
```markdown
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
```

- [ ] **Step 3: Replace `CREDITS.md` and delete the Swift prototype**

`CREDITS.md`:
```markdown
# Credits

- Sprites and portraits: PMD Sprite Repository (SpriteCollab), https://github.com/PMDCollab/SpriteCollab, CC BY-NC 4.0.
  Per-sprite artist credits come from SpriteCollab's tracker.json and are shown in the app.
- Pokémon is © Nintendo / Creatures Inc. / GAME FREAK inc. Wanderdex is not affiliated with or endorsed by them.
```
```bash
git rm -r -q Sources build.sh ThirdParty/mmpx ThirdParty/scalers.h ThirdParty/xbrz_bridge.cpp
rm -rf ThirdParty Wobbuffet.app
```
Remove the now-irrelevant prototype lines from `.gitignore` (the `*.app`, `*-preview.png`, `style-comparison.png` and `/ThirdParty/xbrz/` entries). Keep `/assets/`.

- [ ] **Step 4: Run every check**

Run: `pnpm typecheck && pnpm test && pnpm build && cargo test --manifest-path src-tauri/Cargo.toml && pnpm tauri build`
Expected: all tests pass; `src-tauri/target/release/bundle/dmg/Wanderdex_0.1.0_aarch64.dmg` (or similar) exists.

- [ ] **Step 5: Manual and memory checklist (record the results in the PR or commit message)**

Run the built app from the `.dmg` on macOS, and the installer from a CI or locally built Windows bundle on the Windows PC:
- [ ] Spawn 1, 5 and 10 pets. Record memory: Activity Monitor (WebKit processes plus Wanderdex) on macOS; Task Manager (WebView2 processes plus Wanderdex) on Windows. **If 10 pets exceed about 500 MB, open a follow-up issue for the no-web-view renderer fallback (spec §4.1).**
- [ ] Drag a pet to a second monitor and drop it: it lands on that monitor's Dock/taskbar edge.
- [ ] Unplug the second monitor with a pet on it: the pet reappears on the main monitor.
- [ ] Move or resize the Dock or taskbar: pets follow the new edge within about 1 s.
- [ ] Size 2×/3×/4× on a 125%/150% Windows display: pets stay crisp.
- [ ] Quit and relaunch: pets and size are restored. Launch at login works after a reboot.
- [ ] Offline: cached pets spawn; an uncached one shows the error with Retry.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "Add release workflow, README and license; retire the Swift prototype"
```

- [ ] **Step 7: First release (user-approved step)**

Ask the user before pushing anything. With approval:
```bash
gh repo create wanderdex --public --source . --push
git tag v0.1.0 && git push origin v0.1.0
```
Expected: the Release workflow produces a draft release with the macOS `.dmg` and the Windows installers. Review the draft, then publish it.
