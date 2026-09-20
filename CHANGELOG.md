# Changelog

Release notes are scoped to the version in `Cargo.toml`. A section here does not establish that its artifacts have been built, tested or published.

## [Unreleased]

## [2.0.0]

### Desktop Shell, Letter-Grouped Songs, Up Next Queue & Now Playing Page

Version 2.0.0 is a major milestone release that completely transforms Noir Player into a modern, 3-column desktop music experience with alphabetical letter headings, an Up Next queue rail, a persistent full-width transport bar, a dedicated full-window Now Playing screen with dual waveforms, and embedded lyrics support.

### Highlights & New Features

- **3-Column Desktop Layout**:
  - **Left Navigation Rail**: Quick access to Library, Favorites, Albums, Artists, Folders, Playlists, Discover, Recently Played, and Settings pinned at the bottom.
  - **Center Main View**: Expansive content view with dynamic greeting hero banner, statistics (songs, albums, artists), recently played shelf, and rich song tables.
  - **Right "Up Next" Queue Rail**: Dockable queue rail displaying upcoming songs with remove and clear actions, smoothly collapsible to a slim reopen strip.
  - **Bottom Transport Bar**: Full-width persistent player bar with large artwork, track metadata, interactive click-to-seek scrubber with elapsed/remaining times, volume slider, and toggles for lyrics, equalizer, and queue.
- **Alphabetical Song Grouping & Letter Headings**:
  - Songs are sorted A to Z by default, grouped under small letter section headings (`#`, `A`–`Z`) with track counts for each section.
  - Titles starting with digits or symbols are neatly filed under a leading `#` section.
  - Comprehensive sort picker supporting: A to Z, Z to A, Artist, Album, Duration, and Recently Added (based on file modified time).
  - Media folder picker in the header to filter songs by individual scan folders.
- **Full-Window "Now Playing" Page**:
  - Centered high-resolution album artwork between two dynamic crimson audio waveforms.
  - Quick 5-second skip backward (`-5s`) and forward (`+5s`) controls.
  - Embedded lyrics panel read directly from file tags on demand.
  - Floating "Up Next" preview card and bottom action bar with liked, queue, lyrics, equalizer, and song menu buttons.
- **Recently Played Shelf**:
  - Horizontal shelf of recently played tracks with artwork cards and a "See all" shortcut that opens the full history page.
  - Play history persists across application restarts in `playlists.json`.
- **Enhanced Media Metadata**:
  - Scanner now extracts release year tags and file modification times.
  - Song table displays "Artist • Album (Year)" and duration.
- **Upgraded Playlists & Discover**:
  - Both pages adopt the modern desktop header shell and consistent styling.
  - Playlists reuse the sorted song table with alphabetical letter headings.

### Downloads

- Windows (x64 MSI): `noir-player-2.0.0-windows-x64.msi`
- Windows (x64 Setup): `noir-player-2.0.0-windows-x64-setup.exe`
- macOS (Apple Silicon): `noir-player-2.0.0-macos-arm64.dmg`
- Linux (Debian & Ubuntu): `noir-player-2.0.0-linux-x64.deb`
- Linux (Fedora & RHEL): `noir-player-2.0.0-linux-x64.rpm`
- Linux (Arch): `noir-player-2.0.0-linux-x64.pkg.tar.zst`
- Verify any asset with the matching `.sha256` file or `SHA256SUMS`

## [1.2.1]

### Automatic & Manual Update Checks, Settings Toggle & Update Modal

Version 1.2.1 introduces automatic update checking on application launch, a configurable toggle and manual check action in Settings, and an in-app Update Modal dialog for notifying users of new releases.

### Highlights & New Features

- **Automatic Update Checking on Launch**: Automatically queries GitHub Releases in the background 3 seconds after startup without blocking the UI or audio playback.
- **Configurable Settings Toggle**: Users can enable or disable automatic update checks at any time under Settings > About (and All). The preference is persisted in `playlists.json`.
- **Manual "Check Now" Action**: Added a Software Update row in Settings showing real-time status (Up to date, Checking, Update Available, or Error) with a manual "Check Now" button.
- **In-App Update Modal**: When a newer version is detected, a dialog presents the new version and its release highlights, with "Download update" and "Later" buttons.
- **Download, Verify and Install in the App**: The update dialog downloads the installer for the running platform with a progress bar, checks it against the SHA256 published with the release, and deletes it if the digest does not match. "Restart and install" then hands the verified file to a helper that waits for the app to close, installs it and starts the new build. On Windows that is the NSIS setup run silently or the MSI through msiexec; on macOS the app bundle is replaced from the disk image; Linux packages need root, so the download is passed to the system package installer.
- **Semver Release Detection**: Intelligent semantic version parsing and comparison ignoring pre-release metadata and tags.

### Downloads

- Windows (x64 MSI): `noir-player-1.2.1-windows-x64.msi`
- Windows (x64 Setup): `noir-player-1.2.1-windows-x64-setup.exe`
- macOS (Apple Silicon): `noir-player-1.2.1-macos-arm64.dmg`
- Linux (Debian & Ubuntu): `noir-player-1.2.1-linux-x64.deb`
- Linux (Fedora & RHEL): `noir-player-1.2.1-linux-x64.rpm`
- Linux (Arch): `noir-player-1.2.1-linux-x64.pkg.tar.zst`
- Verify any asset with the matching `.sha256` file or `SHA256SUMS`

## [1.2.0]

### Multi-folder Library, Equalizer, Light Theme & Installers

Version 1.2.0 brings major enhancements across audio, library organization, themes, user controls, and distribution packaging.

### Highlights & New Features

- **Multi-Folder Library Management**: Add and manage multiple folders to scan for music. Automatically scans all configured folders recursively and consolidates them into a unified, deduplicated library.
- **Configurable Download Folder**: Choose custom storage locations for discovered songs downloaded from Last.fm / YouTube services, with sensible default fallbacks.
- **5-Band Graphic Equalizer**: Built-in 5-band audio equalizer with custom gain controls and presets (Flat, Bass Boost, Treble Boost, Vocal Boost, Rock, Electronic). Accessible via `Ctrl+E` or header quick action.
- **Adaptive Red & White Light Theme**: Introduced a light theme matching the mobile NoirPlayer app with elegant white cards, clean `#FAFAFA` background, `#1A1A1A` charcoal typography, and crimson accents.
- **Vibrant Dark Theme Top Glow**: Restored the rich red aura wash at the top of Library, Discover, Playlists, and Player screens.
- **Global Keyboard Shortcuts & In-App Sound Control**:
  - `P` or `Space` — Play / Pause current track
  - `Left Arrow` / `Right Arrow` — Seek backward / forward by configured interval (default 10s)
  - `Shift + Left Arrow` / `Shift + Right Arrow` — Previous / Next track
  - `Up Arrow` / `Down Arrow` — Increase / decrease volume (±5%) with real-time in-app sound HUD indicator
  - `Ctrl+,` (`Cmd+,`) — Open / close Settings modal
  - `Ctrl+E` (`Cmd+E`) — Open / close 5-Band Graphic Equalizer
  - `Escape` — Dismiss open dialogs and overlays
- **Fluid Modal Transitions & Scroll Isolation**: Smooth backdrop and dialog animations with scroll wheel containment preventing background songs from scrolling behind open modals.
- **Rich Settings & About View**: Integrated quick category navigation, customizable seek intervals, theme switching, download and folder pickers, and app version details.
- **Robust Storage & Backward Compatibility**: Removed strict unknown-field constraints on `playlists.json` to allow seamless forward and backward schema evolution across updates.
- **Windows Installers**: Added WiX MSI (`noir-player-1.2.0-windows-x64.msi`) and NSIS setup (`noir-player-1.2.0-windows-x64-setup.exe`) generation support via `cargo installer`.

### Downloads

- Windows (x64 MSI): `noir-player-1.2.0-windows-x64.msi`
- Windows (x64 Setup): `noir-player-1.2.0-windows-x64-setup.exe`
- macOS (Apple Silicon): `noir-player-1.2.0-macos-arm64.dmg`
- Linux (Debian & Ubuntu): `noir-player-1.2.0-linux-x64.deb`
- Linux (Fedora & RHEL): `noir-player-1.2.0-linux-x64.rpm`
- Linux (Arch): `noir-player-1.2.0-linux-x64.pkg.tar.zst`
- Verify any asset with the matching `.sha256` file or `SHA256SUMS`

## [1.1.3]

### First desktop release

This is the first documented desktop release. Version 1.1.3 follows the current Cargo manifest; no earlier desktop release history is asserted.

### Downloads

- Windows (x86_64): `noir-player-1.1.3-windows-x64.zip`, containing `noir_player.exe`
- macOS (Apple Silicon): `noir-player-1.1.3-macos-arm64.dmg`
- Linux (Debian and Ubuntu): `noir-player-1.1.3-linux-x64.deb`
- Linux (Fedora and RHEL): `noir-player-1.1.3-linux-x64.rpm`
- Linux (Arch): `noir-player-1.1.3-linux-x64.pkg.tar.zst`
- Verify any asset with the matching `.sha256` file or the combined `SHA256SUMS`

Packages are unsigned. Windows may show a SmartScreen warning and macOS Gatekeeper will ask before the first launch.

### What it includes

- Native GPUI Kit interface with Library, Player, Playlists and Discover views.
- Recursive scanning of the system Music folder, metadata and embedded artwork, title/artist/album search, and album and artist collections.
- Local audio playback with seeking, volume, previous/next, shuffle and repeat-all.
- Playlists and favourites persisted as local file paths in `noir-player/playlists.json` under the platform's local data directory.
- Last.fm charts and track search using a user-supplied API key.
- Discover audio preparation through YouTube search and the RapidAPI audio service, with progress, cancellation, bounded retries and transfer limits. Play downloads to a temporary cache before playback rather than streaming. Download saves tagged audio to Music.
- PNG-derived Windows executable resources and platform bundle icons, with the GPUI application identity `app.noirplayer.desktop`.
- Three-platform CI definitions for formatting, typechecking, build, unit tests and Clippy, without API credentials.
- Tag and manual release workflow definitions for Windows x64 ZIP, macOS arm64 application archive and Ubuntu 24.04 x64 Debian package, with artifact checksums and draft-only GitHub release creation.

### Limitations and verification status

- Linux and macOS builds and runtime behavior are unverified until their runners and manual package smoke tests pass. Workflow configuration alone is not a successful test result.
- Release packages are unsigned; macOS notarization and automatic updates are not configured.
- Library scanning is limited to the system Music folder. Linked directories and Windows reparse points are skipped. File extensions do not guarantee codec compatibility.
- Saved playlists refer to paths and do not track moved files. Playback state is not restored after restarting.
- Discover requires personal API credentials and provider access. Quotas, charges, incorrect video matches and service failures remain possible. CI does not test live API integration.
- Downloads require a writable hard-link-capable filesystem and reject linked destination components. Transfer and validation limits can reject large or slow-to-decode audio.
- Prepared cache files are cleaned up during normal ownership teardown, but interrupted processes can leave files behind.
- Review Cargo maintainer, repository and platform metadata before public release. The draft workflow does not replace manual installation, icon, audio and launch checks.
