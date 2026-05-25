import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Check, Download, LoaderCircle } from 'lucide-react';
import { installProviderVersion, listProviderVersions } from '../../api.js';
import { formatDate, sourceIcons } from '../../lib/modMeta.jsx';
import { ModCover } from './ModCover.jsx';

function projectIdFor(mod) {
  if (mod?.source === 'modrinth') return mod.modrinthId;
  if (mod?.source === 'curseforge') return mod.curseforgeId;
  return null;
}

function formatNumber(value) {
  if (value == null) return '';
  return new Intl.NumberFormat('ru', { notation: 'compact', maximumFractionDigits: 1 }).format(value);
}

function formatSize(value) {
  if (!value) return '';
  const units = ['Б', 'КБ', 'МБ', 'ГБ'];
  let size = value;
  let index = 0;
  while (size >= 1024 && index < units.length - 1) {
    size /= 1024;
    index += 1;
  }
  return `${size.toFixed(index === 0 ? 0 : 1)} ${units[index]}`;
}

function versionMeta(version) {
  const parts = [];
  if (version.gameVersions?.length) parts.push(version.gameVersions.slice(0, 3).join(', '));
  if (version.loaders?.length) parts.push(version.loaders.join(', '));
  if (version.size) parts.push(formatSize(version.size));
  if (version.downloads != null) parts.push(formatNumber(version.downloads));
  return parts.join(' · ');
}

function normalizeVersion(value) {
  return String(value ?? '')
    .trim()
    .toLowerCase();
}

function isInstalledVersion(mod, version) {
  if (!mod || !version) return false;
  if (mod.source === 'modrinth' && mod.modrinthVersionId && mod.modrinthVersionId === version.id) {
    return true;
  }
  const fileId = version.fileId ?? version.id;
  if (mod.source === 'curseforge' && mod.curseforgeFileId && mod.curseforgeFileId === fileId) {
    return true;
  }
  if (mod.filename && version.filename && mod.filename === version.filename) {
    return true;
  }
  const installedVersion = normalizeVersion(mod.installedVersion);
  const providerVersion = normalizeVersion(version.versionNumber);
  const providerFilename = normalizeVersion(version.filename);
  return (
    installedVersion !== '' &&
    (installedVersion === providerVersion ||
      providerVersion.includes(installedVersion) ||
      providerFilename.includes(installedVersion))
  );
}

export function VersionDialog({ mod, busy, onClose, onInstalled }) {
  const [payload, setPayload] = useState(null);
  const [loading, setLoading] = useState(false);
  const [installingId, setInstallingId] = useState(null);
  const [error, setError] = useState('');
  const runRef = useRef(0);

  useEffect(() => {
    if (!mod) return;
    const projectId = projectIdFor(mod);
    const runId = runRef.current + 1;
    runRef.current = runId;
    setPayload(null);
    setError('');

    if (!projectId) {
      setError('Сначала выбери проект мода у поставщика.');
      return;
    }

    setLoading(true);
    void listProviderVersions({
      key: mod.key,
      source: mod.source,
      projectId,
      filename: mod.filename
    })
      .then((next) => {
        if (runRef.current !== runId) return;
        setPayload(next);
      })
      .catch((err) => {
        if (runRef.current !== runId) return;
        setError(String(err));
      })
      .finally(() => {
        if (runRef.current !== runId) return;
        setLoading(false);
      });
  }, [mod]);

  if (!mod) return null;

  const projectId = projectIdFor(mod);
  const provider = sourceIcons[mod.source]?.label ?? mod.source;
  const target = payload?.target;
  const targetLabel = [target?.minecraftVersion, target?.loader].filter(Boolean).join(' · ');
  const uiLocked = busy || loading || Boolean(installingId);

  async function install(version) {
    if (!projectId) return;
    setInstallingId(version.id);
    setError('');
    try {
      const next = await installProviderVersion({
        key: mod.key,
        source: mod.source,
        projectId,
        filename: mod.filename,
        versionId: version.id,
        fileId: version.fileId ?? undefined,
        downloadUrl: version.downloadUrl ?? undefined,
        downloadFilename: version.filename,
        versionNumber: version.versionNumber
      });
      onInstalled(next);
    } catch (err) {
      setError(String(err));
    } finally {
      setInstallingId(null);
    }
  }

  return createPortal(
    <div className="dependencyModalBackdrop" onMouseDown={() => !uiLocked && onClose()}>
      <div className="dependencyModalStack versionModalStack" onMouseDown={(event) => event.stopPropagation()}>
        <div className="dependencyModalHead">
          <ModCover mod={mod} size="tile" />
          <div className="dependencyModalHeadText">
            <p className="dependencyModalSubtitle">{provider}{targetLabel ? ` · ${targetLabel}` : ''}</p>
            <h3 className="dependencyModalTitle">{mod.displayName}</h3>
          </div>
        </div>

        {error ? <p className="providerSearchError">{error}</p> : null}

        <div className="versionModal" role="dialog" aria-modal="true" aria-label="Версии мода">
          {loading ? (
            <div className="versionState versionLoading">
              <LoaderCircle className="spin" size={28} />
              <span>Загрузка...</span>
            </div>
          ) : null}
          {!loading && payload?.versions?.length === 0 ? <p className="versionState">Нет версий под текущую сборку.</p> : null}
          {payload?.versions?.length ? (
            <ul className="versionList">
              {payload.versions.map((version) => {
                const installing = installingId === version.id;
                const installed = isInstalledVersion(mod, version);
                const meta = versionMeta(version);
                return (
                  <li key={version.id}>
                    <button
                      type="button"
                      className={`versionRow${installed ? ' installed' : ''}`}
                      onClick={() => {
                        if (!installed) install(version);
                      }}
                      disabled={uiLocked}
                      aria-current={installed ? 'true' : undefined}
                    >
                      <span className="versionType">{(version.releaseType || 'R').slice(0, 1).toUpperCase()}</span>
                      <span className="versionText">
                        <strong>{version.versionNumber}</strong>
                        <span>{version.name !== version.versionNumber ? version.name : version.filename}</span>
                        <small>{installed ? `Установлена${meta ? ` · ${meta}` : ''}` : meta}</small>
                      </span>
                      <span className="versionDate">{formatDate(version.datePublished)}</span>
                      <span className="versionInstall">
                        {installed ? <Check size={18} /> : installing ? <LoaderCircle className="spin" size={18} /> : <Download size={17} />}
                      </span>
                    </button>
                  </li>
                );
              })}
            </ul>
          ) : null}
        </div>
      </div>
    </div>,
    document.body
  );
}
