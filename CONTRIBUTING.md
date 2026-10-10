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

### Other useful commands

```sh
pnpm contact-sheet 0202     # render-check one sprite variant into contact-sheet.png (macOS)
./scripts/update-index.sh   # refresh the bundled sprite index from SpriteCollab HEAD
```

Never commit downloaded sprites. They are CC BY-NC and are fetched at runtime; `/assets/` is git-ignored.

## Making a change

1. Fork the repository and create a branch from `main`.
2. Make your change, with tests where the behavior can be tested (the pet behavior in `src-tauri/src/brain.rs`
   and the renderer in `src/shared/` both have unit tests to extend).
3. Run the checks above.
4. Open a pull request against `main` and fill in the template. Link the issue it fixes.

`main` is protected: pull requests need passing CI and a review from a code owner before they merge,
and they are squash-merged.

## License

By contributing, you agree that your contributions are licensed under the [MIT License](LICENSE).
