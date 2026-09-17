# Noir Player

[![CI](https://github.com/Abdullah-Masood-05/noir-player-desktop-app/actions/workflows/ci.yml/badge.svg)](https://github.com/Abdullah-Masood-05/noir-player-desktop-app/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/Abdullah-Masood-05/noir-player-desktop-app)](https://github.com/Abdullah-Masood-05/noir-player-desktop-app/releases)
[![Rust](https://img.shields.io/badge/built_with-Rust-dea584?logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![Build targets](https://img.shields.io/badge/build_targets-Windows%20%7C%20macOS%20%7C%20Linux-555)](#packaging-and-releases)

This is the desktop version of [NoirPlayer for Android](https://github.com/Abdullah-Masood-05/NoirPlayer), rebuilt in Rust with GPUI Kit. It uses the same red-and-dark theme and Discover services, with a local music library and playlists for desktop.

Version 1.1.3. See [packaging and releases](#packaging-and-releases) for target platforms and testing status.

## Playback and library

The library scans the system Music folder recursively on startup and when rescanned. It reads title, artist, album, duration and embedded artwork, groups tracks by album and artist, and searches across title, artist and album. It does not follow symbolic links or Windows reparse points. There is no configurable library-folder picker.

The scanner accepts `.mp3`, `.flac`, `.wav`, `.ogg`, `.oga`, `.m4a`, `.mp4`, `.aac`, `.aif` and `.aiff`. Extensions are not a guarantee of codec support. Playback uses Rodio's enabled decoders, including the additional ALAC and AIFF features. Files with unreadable metadata can fall back to a filename title if audio decoding succeeds.

Controls include play/pause, previous/next, seeking, volume, shuffle and repeat-all. Playlists and favourites store local file paths, not copies of the audio. Moving a file breaks its saved reference; rescanning does not relocate it automatically. Playback position, queue, volume and shuffle state are not saved across restarts.

## Discover and API configuration

Local playback does not need API keys. Discover uses these runtime environment variables:

| Variable | Used for |
| --- | --- |
| `LASTFM_API_KEY` | Last.fm top tracks and track search, with up to 30 results per request |
| `YOUTUBE_API_KEY` | YouTube Data API v3 search for the first video matching title and artist |
| `RAPIDAPI_KEY` | The `youtube-mp36.p.rapidapi.com` audio-resolution service |

Supply your own keys. Enable YouTube Data API v3 for the Google Cloud project and ensure the RapidAPI account has access to this specific service. A generic RapidAPI key without the required subscription may not work. There is no built-in shared key, login flow or settings editor.

`src/config.rs` calls `dotenvy::dotenv()` on first API access. It searches for `.env` in the process working directory and its parents. Existing environment variables take precedence. It does not read a dedicated configuration file under AppData, Application Support or XDG config directories, and does not look next to the executable unless that is the working directory. Restart after changing keys.

For development, copy `.env.example` to `.env` in the repository root and fill in only the keys you need. For an installed build, set variables in the launching process or launch from a private directory containing `.env`. Desktop launchers may use a different working directory and environment than a terminal.

Do not commit `.env`, attach it to bug reports, put keys in Cargo metadata, or add them as build secrets. Before copying the template, ensure `.env` is excluded from version control. The workflows reject a tracked root `.env` without reading it. Release archives contain no environment file.

Discover Play downloads and validates the complete audio file into a local cache before playback. It is not streaming. Download saves audio to the system Music folder and writes the discovered title and artist into its tags. A prepared cached copy can be reused. The first YouTube result can be the wrong recording; neither matching nor service availability is guaranteed. Only download audio you are permitted to download and follow the providers' terms.

### Quotas and service failures

Each uncached audio resolution makes one YouTube `search.list` call and up to four RapidAPI requests, with two-second waits while processing. Last.fm search and chart requests also consume service quota. Repeated retries can exhaust quotas or incur charges under your plan. The app does not track remaining quota or enforce a spending cap.

[Google's current quota documentation](https://developers.google.com/youtube/v3/determine_quota_cost) lists a default separate limit of 100 `search.list` calls per day, at one quota per call, resetting at midnight Pacific Time. Provider rules change; check your project's actual quota rather than assuming the older 100-unit search pricing. Last.fm limits and RapidAPI quotas, rate limits and prices depend on provider policy and your account. No free or unlimited access is promised.

Missing keys, rejected requests, exhausted quota and malformed responses are shown as errors. Audio transfers are limited to 100 MiB and 180 seconds, with at most five redirects. Validation rejects unsupported audio, more than three hours of samples, and decoding that exceeds its validation deadline. Download destinations must support hard links, allow writing and have no linked directory components. These restrictions can reject redirected home folders, removable filesystems or network storage.

## Local data

| Data | Windows | macOS | Linux |
| --- | --- | --- | --- |
| Playlists and favourites | `%LOCALAPPDATA%\noir-player\playlists.json` | `~/Library/Application Support/noir-player/playlists.json` | `$XDG_DATA_HOME/noir-player/playlists.json`, default `~/.local/share/noir-player/playlists.json` |
| Prepared Discover audio | `%LOCALAPPDATA%\noir-player\discover` | `~/Library/Caches/noir-player/discover` | `$XDG_CACHE_HOME/noir-player/discover`, default `~/.cache/noir-player/discover` |
| Downloads and scanned library | System Music folder | System Music folder | System Music folder from user-directory configuration |

Paths are resolved through `dirs`. Prepared audio is removed when its in-memory owner is dropped; an interrupted process can leave cache or temporary files behind. Downloads explicitly kept by the app remain in Music. A missing or malformed playlist store produces an error rather than overwriting the existing data. There is no cloud sync.

## Development

Install stable Rust through rustup. The checked-in `Cargo.lock` is used by CI. No minimum Rust version is declared; use the current stable toolchain rather than assuming Rust 2021 means an older compiler is sufficient.

### Windows x64

Install Visual Studio 2022 Build Tools with Desktop development with C++ and a Windows SDK. Use the MSVC Rust toolchain. Windows icon resources are generated from `Noir_Player_Logo.png` by `build.rs`; no external audio converter is required.

### macOS arm64

Use an Apple Silicon Mac with full Xcode, the macOS SDK and Metal tools, not only standalone command-line tools. Select the installed Xcode developer directory and accept its license. Check:

```sh
xcodebuild -version
xcrun --sdk macosx --find metal
xcrun --sdk macosx --find metallib
export BINDGEN_EXTRA_CLANG_ARGS="--sysroot=$(xcrun --sdk macosx --show-sdk-path)"
export MACOSX_DEPLOYMENT_TARGET=14.0
```

GPUI's Apple build script invokes `metal` and `metallib`. Native audio uses CoreAudio. The release workflow targets macOS 14 or later and sets the bundle minimum to 14.0. No Homebrew audio library is required by this project's current dependency configuration.

### Linux x64, Ubuntu 24.04

```sh
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  build-essential pkg-config clang libclang-dev \
  libfontconfig-dev libwayland-dev libwebkit2gtk-4.1-dev \
  libxkbcommon-x11-dev libx11-xcb-dev libssl-dev libzstd-dev \
  vulkan-validationlayers libvulkan1 libasound2-dev
```

The GPUI Kit upstream Ubuntu recipe supplies the windowing, font, WebKit, TLS and Vulkan packages. `libasound2-dev` is required by Rodio's CPAL backend. `pkg-config` and Clang development libraries support native dependency discovery and bindings. The upstream recipe is a conservative build baseline, not a claim that every package is directly used by Noir Player.

Running the UI requires a graphical X11 or Wayland session, a working GPU driver and an audio output device. Headless compilation and unit tests do not establish that rendering or playback works. Vulkan's loader alone is not a GPU driver.

### Commands

Run from the repository root:

```sh
cargo run --locked
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo build --locked
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo build --release --locked
```

Tests cover local parsing, scanning, storage and audio-related helpers. CI does not provide API credentials or exercise live services. Development commands do not require an `.env` file.

### Dependency sources

The manifest uses `gpui-kit 0.6`, `rodio 0.21`, `lofty 0.22`, `ureq 3`, `serde`, `serde_json`, `dotenvy`, `urlencoding`, `walkdir`, `dirs`, `smol` and `anyhow`. Build dependencies are `winresource` and `image`; Linux also uses `image` for the window icon. The inspected lockfile resolves GPUI Kit 0.6.1, GPUI platform snapshots 0.3.5, Rodio 0.21.1 and Lofty 0.22.4.

Build and packaging decisions are based on:

- [GPUI Kit Ubuntu dependency installer](https://github.com/longbridge/gpui-kit/blob/main/script/install-linux.sh).
- [GPUI Linux 0.3.5 manifest](https://docs.rs/crate/gpui-pre-linux/0.3.5/source/Cargo.toml.orig), including X11 and Wayland defaults.
- [GPUI Apple 0.3.5 build script](https://docs.rs/crate/gpui-pre-apple/0.3.5/source/build.rs) and [Zed's macOS build guide](https://github.com/zed-industries/zed/blob/main/docs/src/development/macos.md).
- [CPAL 0.16 platform requirements](https://github.com/RustAudio/cpal/blob/v0.16.0/README.md).
- [cargo-bundle 0.11.0 release metadata](https://crates.io/api/v1/crates/cargo-bundle/0.11.0), [manifest documentation](https://crates.io/api/v1/crates/cargo-bundle/0.11.0/readme) and [Linux desktop generation](https://docs.rs/crate/cargo-bundle/0.11.0/source/src/bundle/linux/common.rs).

## Packaging and releases

The current workflow produces the following assets, where `VERSION` is the version in `Cargo.toml`. Windows installers are an Unreleased change; this does not change the historical 1.1.3 ZIP release notes.

| Target | Runner | Artifact |
| --- | --- | --- |
| `x86_64-pc-windows-msvc` | `windows-2022` | `noir-player-VERSION-windows-x64.msi` and `noir-player-VERSION-windows-x64-setup.exe` |
| `aarch64-apple-darwin` | `macos-14` | `noir-player-VERSION-macos-arm64.dmg` |
| `x86_64-unknown-linux-gnu` | `ubuntu-24.04` | `noir-player-VERSION-linux-x64.deb`, `.rpm` and `.pkg.tar.zst` |

These are configured targets, not claims of successful builds or runtime tests. Linux and macOS remain unverified until their runners pass and the resulting packages receive manual smoke tests. Windows packages also need release-candidate testing. No installer signing, Apple notarization or automatic updates are configured. OS security warnings are expected for unsigned downloads.

CI checks formatting, typechecking, build, tests and Clippy on all three platforms. Windows also checks the `installer` feature and runs Clippy with `-D warnings`, builds the release application, generates both installers, validates exactly one nonempty MSI and NSIS EXE, and uploads `ci-windows-x64-installers` for 14 days. Branch pushes, pull requests and manual CI dispatch exercise generation without a release tag; they do not install or launch the application.

Release runs on `v*` tags and manual dispatch. Dispatch must select an existing tag matching the version in `Cargo.toml`. Manual dispatch builds artifacts by default; enable `create_draft` to request a draft release. Before the next release, bump the version and move the relevant Unreleased notes into its version-scoped section; do not rewrite or retag 1.1.3.

The release workflow calls the separate CI workflow at the same revision before packaging. Every platform must pass. The release caller skips CI's extra Windows release build and installer generation because its packaging job performs them, but retains installer feature checks. Windows packaging reuses the built application and normalizes the MSI and NSIS filenames listed above. Artifacts and SHA256 sidecars are uploaded first and retained for 14 days. The draft job requires all six package assets and their sidecars, verifies each checksum, combines them into `SHA256SUMS`, and extracts only that version's section from `CHANGELOG.md`. Draft release uploads include the packages, sidecars and `SHA256SUMS`.

Configure the GitHub environment `release-draft` with required reviewers and tag deployment restrictions before enabling release creation. Merely naming an environment does not configure approval rules. Build jobs have read-only repository access; only the draft job has `contents: write`. It uses `gh release create --draft --verify-tag`, refuses to replace an existing release and never publishes a live release automatically. Inspect assets, run installation and playback smoke tests, and review notes before manually publishing the draft.

### Local package commands

On Windows x64, run from the repository root:

```sh
cargo build --release
cargo installer
```

The default input is `target/release/noir_player.exe`. The `.cargo/config.toml` alias expands to `cargo run --release --locked --features installer --bin windows_installer --`; the optional installer binary is skipped by default builds. It packages the existing application executable without rebuilding or installing the application locally. Its packaging-only dependencies are pinned to `tauri-bundler 2.9.4` and `tauri-utils 2.9.3`; no Tauri or WebView runtime is added to Noir Player.

```text
target/release/
  noir_player.exe
  bundle/
    msi/*.msi
    nsis/*.exe
```

To package an explicitly targeted build instead:

```sh
cargo build --release --locked --target x86_64-pc-windows-msvc
cargo installer --release-dir "target/x86_64-pc-windows-msvc/release"
```

Both installers are written below `<release-dir>/bundle/msi/*.msi` and `<release-dir>/bundle/nsis/*.exe`. Generation requires a Windows host and an x64 PE application executable. WiX and NSIS are downloaded automatically and cached in `target/installer-tools/.tauri`; allow HTTPS access for the initial download. No system WiX or NSIS installation is needed. MSI generation requires Windows PowerShell, .NET Framework 4.5 or later, and VBScript enabled in Windows Optional Features. These are packaging prerequisites, not a requirement to install Noir Player on the build machine. Installers are unsigned; SmartScreen or other Windows security warnings are expected. Generation is not an installation or uninstall smoke test.

On macOS or Linux, install the pinned packaging tool:

```sh
cargo install cargo-bundle --version 0.11.0 --locked
```

Version 0.11.0 is a released, non-yanked crates.io version. Its API metadata declares Rust edition 2024 but no `rust_version`; this is not evidence of compatibility with arbitrary older Rust versions. Its CLI has no `--locked` option. Build with `--locked` first, then skip the bundler's implicit rebuild and resolve metadata offline:

```sh
cargo build --release --locked --target aarch64-apple-darwin
cargo fetch --locked
CARGO_BUNDLE_SKIP_BUILD=true CARGO_NET_OFFLINE=true cargo bundle --release --bin noir_player --target aarch64-apple-darwin --format osx
```

```sh
cargo build --release --locked --target x86_64-unknown-linux-gnu
cargo fetch --locked
CARGO_BUNDLE_SKIP_BUILD=true CARGO_NET_OFFLINE=true cargo bundle --release --bin noir_player --target x86_64-unknown-linux-gnu --format deb
```

These commands run on their respective native platforms and produce bundles below `target/<triple>/release/bundle`. Explicit `--bin noir_player` selects the application rather than the optional installer binary. For the distributable package, use the release workflow's postprocessing too. It sets the macOS minimum version, checks arm64 architecture and the bundle icon, and archives the `.app` with Unix permissions preserved. Windows uses `cargo installer` instead of cargo-bundle and validates both installer outputs before copying them to normalized release asset names.

For Linux packaging, additionally install `dpkg-dev`, `fakeroot` and `desktop-file-utils`. cargo-bundle 0.11.0 names the desktop entry and icons after the binary, not `identifier`. The workflow renames them to `app.noirplayer.desktop.desktop` and `app.noirplayer.desktop.png`, updates `Icon`, adds `StartupWMClass`, and verifies the identifier against GPUI's `app_id`. The repeated `.desktop` is intentional: the application ID itself ends in `.desktop`.

The Debian dependency list is generated with `dpkg-shlibdeps` from the actual ELF binary. It additionally includes the dynamically loaded Vulkan, Wayland and xkbcommon libraries, which ELF dependency inspection alone can miss. Package structure, architecture, version and the desktop entry are checked before upload. This is not an installation test on a clean desktop. Packages built on Ubuntu 24.04 are not promised to run on older distributions.

Download `SHA256SUMS` alongside release assets and check it with `sha256sum -c SHA256SUMS` on Linux or `shasum -a 256 -c SHA256SUMS` on macOS. On Windows, compare `Get-FileHash -Algorithm SHA256` output with the corresponding entry. Checksums detect damaged or mismatched downloads; they are not code signatures.

### Cargo metadata follow-up

The current bundle metadata already provides `name`, `identifier = "app.noirplayer.desktop"`, the PNG icon, category, short description and terminal setting. Keep the identifier synchronized with GPUI's `app_id`.

Before public packaging, the manifest owner should add:

- `[package]` `repository` and `homepage` pointing to the public repository, plus an owner-approved `authors` entry. Without authors, cargo-bundle uses `Unknown <unknown@localhost>` as Debian maintainer. Do not invent a maintainer email.
- `[package.metadata.bundle]` `long_description` and `osx_minimum_system_version = "14.0"`, matching the workflow deployment target.
- A reviewed `deb_depends` list only after checking runner-generated dependencies. The workflow calculates the release package's dependencies itself; copying build-time `-dev` packages into runtime metadata is incorrect.

Do not add broad `resources` globs that could include `.env` or local data. The existing icon metadata is sufficient for cargo-bundle to generate platform icons. The manifest declares MIT; release preparation should also review the project's license file and dependency redistribution notices.
