# Contributing to Wanderdex

Thanks for helping out! This covers everything you need to go from a fresh clone to an open pull request.
By taking part you agree to the [Code of Conduct](CODE_OF_CONDUCT.md).

## Questions and ideas

- **Questions:** ask in [Discussions](https://github.com/nooikko/wanderdex/discussions).
- **Bugs and feature requests:** open an [issue](https://github.com/nooikko/wanderdex/issues/new/choose).
- **Security problems:** see [SECURITY.md](SECURITY.md). Never in a public issue.

## Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 22 or newer
- [pnpm](https://pnpm.io/installation) 10
- The [Tauri system prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS
  (Xcode Command Line Tools on macOS; Microsoft C++ Build Tools and WebView2 on Windows)

Wanderdex targets macOS and Windows.

## Setup

```sh
git clone https://github.com/nooikko/wanderdex.git
cd wanderdex
pnpm install
pnpm tauri dev
```

`pnpm tauri dev` builds the Rust side and starts the app with hot reload for the web views.
The app lives in the menu bar (macOS) or system tray (Windows); open the picker from there.
Sprites download from SpriteCollab on first use, so you need a network connection.

## Checks

CI runs these on every pull request. Run them locally first:

```sh
pnpm typecheck                                  # TypeScript
pnpm test                                       # frontend tests (Vitest)
pnpm build                                      # frontend production build
cargo test --manifest-path src-tauri/Cargo.toml # Rust tests
```

CodeQL also scans the Rust, TypeScript and workflow code.

Dependency checks (not in CI; needs `cargo install --locked cargo-deny`). `deny.toml` covers advisories,
allowed licenses and crate sources for both Rust crates:

```sh
cargo deny --manifest-path src-tauri/Cargo.toml check
cargo deny --manifest-path setup/src-tauri/Cargo.toml check
pnpm audit
```

### Other useful commands

```sh
pnpm contact-sheet 0202     # render-check one sprite variant into contact-sheet.png (macOS)
./scripts/update-index.sh   # refresh the bundled sprite index from SpriteCollab HEAD
```

The installer artwork lives in `src-tauri/dmg/` (macOS `.dmg` background and layout) and `src-tauri/nsis/`
(the Windows uninstaller's sidebar and header).

On Windows, people download **Wanderdex Setup** (`setup/`), a small Tauri app with its own pixel-art screen.
It embeds the regular NSIS installer, runs it silently, then applies the chosen options (desktop shortcut,
launch at sign-in). Its UI is plain HTML/JS in `setup/ui/`, no build step. To work on it on any OS (the install
step is simulated outside Windows):

```sh
cd setup && ../node_modules/.bin/tauri dev
```

Wobbuffet on the setup screen is downloaded from SpriteCollab when setup runs, like every other sprite, and
setup falls back to a plain progress bar when it can't be fetched. To build the macOS installer locally the way the release does
(needs [uv](https://docs.astral.sh/uv/)):

```sh
pnpm tauri build --bundles app
uvx --from dmgbuild==1.6.5 dmgbuild -s src-tauri/dmg/settings.py \
  -D app=src-tauri/target/release/bundle/macos/Wanderdex.app Wanderdex Wanderdex.dmg
```

Never commit downloaded sprites. They are CC BY-NC; the app fetches them at runtime into its own data folder.

## Making a change

1. Fork the repository and create a branch from `main`.
2. Make your change, with tests where the behavior can be tested (the pet behavior in `src-tauri/src/brain.rs`
   and the renderer in `src/shared/` both have unit tests to extend). On macOS, pets are drawn natively
   (`src-tauri/src/native.rs`) so clicks pass through their transparent pixels; on Windows, and in the picker
   preview, they're drawn by the web renderer in `src/shared/`.
3. Run the checks above.
4. Open a pull request against `main` and fill in the template. Link the issue it fixes.

`main` is protected: pull requests need passing CI and a review from a code owner before they merge,
and they are squash-merged.

## Releasing

Releases are cut by maintainers from `main` by pushing a version tag; GitHub Actions does the rest.
Only repository admins can create, move or delete `v*` tags, and published releases are immutable.
Versions follow [Semantic Versioning](https://semver.org/): bump the patch for fixes, the minor for new
features, and the major for anything that breaks saved settings or existing behavior.

1. In a pull request, set the new version in `package.json` and `src-tauri/Cargo.toml`, then run
   `cargo check --manifest-path src-tauri/Cargo.toml` to update `Cargo.lock`. (`tauri.conf.json` reads
   its version from `package.json`.) Merge it.
2. Tag the merge commit on `main` and push the tag:
   ```sh
   git switch main && git pull
   git tag -a v0.2.0 -m "v0.2.0"
   git push origin v0.2.0
   ```
3. The [Release workflow](.github/workflows/release.yml) checks that the tag is on `main` and matches both
   version files, runs the tests, creates a **draft** release with generated notes, then builds and uploads
   the universal macOS `.dmg` and the Windows `-setup.exe` (Wanderdex Setup with the installer inside).
4. Download both installers and smoke-test them. (To try installers before tagging, run the
   [Preview build](.github/workflows/preview.yml) workflow from the Actions tab on any branch; it attaches
   them to the run as artifacts.) Edit the generated notes (they list the merged pull requests),
   then publish the draft.

If the workflow fails, fix it on `main`, delete the tag (`git push --delete origin v0.2.0 && git tag -d v0.2.0`)
and any draft release, and tag again. Never move a tag after its release is published; ship a new patch version instead.

## License

By contributing, you agree that your contributions are licensed under the [MIT License](LICENSE).
