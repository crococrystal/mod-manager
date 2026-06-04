# Changelog

## 0.5.9 - 2026-06-04

- Added catalog search and install flow for Modrinth and CurseForge projects, including project details, dependency preview, and installed-state handling.
- Added external page opening through the app opener plugin, including links from catalog surfaces and the mod context menu.
- Added row context menu actions for copying files, opening a mod page, and deleting selected mods.
- Added multi-select table interactions, including range selection and command/control toggling for non-adjacent mods.
- Added guarded mod deletion so dependency mods cannot be removed while still used by installed mods.
- Improved provider, version, and source handling with stronger project matching and richer metadata refresh support.
- Improved UI structure with extracted toolbar, stats, catalog, and context-menu components.
