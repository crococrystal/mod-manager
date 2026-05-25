# Mod Manager

Native Tauri app for keeping Minecraft modpack metadata outside jar filenames.

## Download

Installers for **Windows** and **macOS (Apple Silicon)**:

**https://github.com/crococrystal/mod-manager-releases/releases/latest**

Direct links (version may change after each build — the Releases page always has the latest):

- [macOS `.dmg`](https://github.com/crococrystal/mod-manager-releases/releases/latest)
- [Windows `.exe`](https://github.com/crococrystal/mod-manager-releases/releases/latest)

In-app updates: **Settings → Update** after install.

## Current MVP

- Pick a PrismLauncher instance or `minecraft/mods` folder.
- Scan jar files and `.index/*.pw.toml` metadata natively in Rust.
- Store labels in `<instance>/.mod-manager/mod-tags.json`.
- Edit side, library/optimization flags, description, and manual dependencies.
- Compute `usedBy` from dependencies instead of storing reverse links by hand.
- Keep service actions such as opening `mods` and checking the pack inside Settings.

## Development

```bash
npm install
npm run tauri dev
```

Useful checks:

```bash
npm run build
cd src-tauri && cargo check
```
