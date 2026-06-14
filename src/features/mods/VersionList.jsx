import { Check, Download, LoaderCircle } from 'lucide-react';
import { formatDate } from '../../lib/modMeta.jsx';
import { isInstalledVersion, versionMeta, versionTypeClass, versionTypeLabel } from './versionListUtils.js';

export function VersionList({
  mod,
  versions,
  loading,
  error = '',
  busy = false,
  installingId = null,
  compact = false,
  onInstall
}) {
  if (loading) {
    return (
      <div className={`versionState versionLoading${compact ? ' versionLoadingCompact' : ''}`}>
        <LoaderCircle className="spin" size={compact ? 22 : 28} />
        <span>Загрузка...</span>
      </div>
    );
  }

  if (error) {
    return null;
  }

  if (!versions.length) {
    return <p className="versionState">Нет версий под текущую сборку.</p>;
  }

  return (
    <ul className={`versionList${compact ? ' versionListCompact' : ''}`}>
      {versions.map((version) => {
        const installing = installingId === version.id;
        const installed = isInstalledVersion(mod, version);
        const meta = versionMeta(version);
        return (
          <li key={version.id}>
            <button
              type="button"
              className={`versionRow${compact ? ' versionRowCompact' : ''}${installed ? ' installed' : ''}`}
              onClick={() => {
                if (!installed && onInstall) onInstall(version);
              }}
              disabled={busy || installing}
              aria-current={installed ? 'true' : undefined}
            >
              <span className={`versionType ${versionTypeClass(version.releaseType)}`}>
                {versionTypeLabel(version.releaseType)}
              </span>
              <span className="versionText">
                <strong>{version.versionNumber}</strong>
                {!compact && version.name !== version.versionNumber ? (
                  <span>{version.name}</span>
                ) : null}
                {compact ? (
                  <small className={installed ? 'versionInstalledLabel' : 'versionInstalledLabel versionInstalledLabelEmpty'}>
                    {installed ? 'Установлена' : '\u00a0'}
                  </small>
                ) : (
                  <small>{installed ? `Установлена${meta ? ` · ${meta}` : ''}` : meta}</small>
                )}
              </span>
              {!compact ? <span className="versionDate">{formatDate(version.datePublished)}</span> : null}
              <span className="versionInstall">
                {installed ? (
                  <Check size={compact ? 16 : 18} />
                ) : installing ? (
                  <LoaderCircle className="spin" size={compact ? 16 : 18} />
                ) : (
                  <Download size={compact ? 16 : 17} />
                )}
              </span>
            </button>
          </li>
        );
      })}
    </ul>
  );
}
