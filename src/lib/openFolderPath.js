import { revealItemInDir } from '@tauri-apps/plugin-opener';

function withPathSeparator(path, reference) {
  if (reference.includes('\\')) return path.replace(/\//g, '\\');
  return path;
}

export function resolveInstanceExplorerPath(instanceRoot, modsDir) {
  const root = instanceRoot?.trim();
  if (!root) return null;

  const mods = modsDir?.trim();
  if (mods) {
    const normalized = mods.replace(/\\/g, '/');
    const match = normalized.match(/^(.*)\/minecraft\/mods$/i);
    if (match) {
      return withPathSeparator(`${match[1]}/minecraft`, mods);
    }
  }

  return root;
}

export async function openFolderPath(path) {
  const trimmed = path?.trim();
  if (!trimmed) return false;
  try {
    await revealItemInDir(trimmed);
    return true;
  } catch (err) {
    console.error('revealItemInDir failed', err);
    return false;
  }
}
