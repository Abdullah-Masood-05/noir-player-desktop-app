# Changelog

Release notes are scoped to the version in `Cargo.toml`. A section here does not establish that its artifacts have been built, tested or published.

## [1.1.3]

### First desktop release

This is the first documented desktop release. Version 1.1.3 follows the current Cargo manifest; no earlier desktop release history is asserted.

### Downloads

- Windows (x86_64): `noir-player-1.1.3-windows-x64.zip`, containing `noir_player.exe`
- macOS (Apple Silicon): `noir-player-1.1.3-macos-arm64.tar.gz`, containing `Noir Player.app`
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
