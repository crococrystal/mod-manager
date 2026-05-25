# mod-manager

Native Tauri app for keeping Minecraft modpack metadata outside jar filenames.

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
