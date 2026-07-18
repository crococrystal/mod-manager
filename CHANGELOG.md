# Changelog

## 0.5.18 - 2026-07-18

- Added a setting to include or ignore AutoModPack-managed mod folders.
- AutoModPack cache folders are now ignored when the AutoModPack jar is disabled, so server-only cached mods no longer appear as installed.
- Fixed duplicate update entries and invalidated stale update-cache data from removed or disabled mod jars.

## 0.5.16 - 2026-07-18

- Keep catalog search open after installing a mod instead of returning to the installed-mods list.
- Preserve the catalog scroll position for each provider and search query during the session.

## 0.5.9 - 2026-06-04

- Added catalog search and install flow for Modrinth and CurseForge projects, including project details, dependency preview, and installed-state handling.
- Added external page opening through the app opener plugin, including links from catalog surfaces and the mod context menu.
- Added row context menu actions for copying files, opening a mod page, and deleting selected mods.
- Added multi-select table interactions, including range selection and command/control toggling for non-adjacent mods.
- Added guarded mod deletion so dependency mods cannot be removed while still used by installed mods.
- Improved provider, version, and source handling with stronger project matching and richer metadata refresh support.
- Improved UI structure with extracted toolbar, stats, catalog, and context-menu components.
