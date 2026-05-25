#!/usr/bin/env bash
set -euo pipefail

version="$(node -p "require('./package.json').version")"
releases_repo="${RELEASES_REPO:-crococrystal/mod-manager-releases}"
source_repo="${SOURCE_REPO:-crococrystal/mod-manager}"
base="https://github.com/${releases_repo}/releases/download/latest"
releases_page="https://github.com/${releases_repo}/releases/latest"

cat >"${1:-README.releases.md}" <<EOF
# Mod Manager — Downloads

Публичные сборки [**Mod Manager**](https://github.com/${source_repo}) (менеджер метаданных модов для Minecraft).

> Исходный код — в основном репозитории. **Здесь только установщики.**

## Скачать

### [→ Открыть страницу Releases (Latest build)](${releases_page})

| Платформа | Установщик |
|-----------|------------|
| **macOS** (Apple Silicon) | [Mod.Manager_${version}_aarch64.dmg](${base}/Mod.Manager_${version}_aarch64.dmg) |
| **Windows** (x64) | [Mod.Manager_${version}_x64-setup.exe](${base}/Mod.Manager_${version}_x64-setup.exe) |

**Текущая версия:** \`${version}\`

### Установка

- **macOS:** открой \`.dmg\` → перетащи в «Программы»
- **Windows:** запусти \`.exe\`

### Обновления в приложении

После установки: **Настройки → Обновить** (in-app updater).

---

*Сборки публикуются автоматически при push в \`main\`.*
EOF

echo "Wrote ${1:-README.releases.md} for v${version}"
