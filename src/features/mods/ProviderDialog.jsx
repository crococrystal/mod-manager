import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { ExternalLink, LoaderCircle } from 'lucide-react';
import {
  lookupProviderFingerprint,
  switchModSource
} from '../../api.js';
import { sourceIcons, modModalSubtitle } from '../../lib/modMeta.jsx';
import { ModModalHead } from './ModModalHead.jsx';

const providerOptions = [
  { id: 'modrinth', label: 'Modrinth' },
  { id: 'curseforge', label: 'CurseForge' }
];

function providerLabel(source) {
  return providerOptions.find((item) => item.id === source)?.label ?? 'Поставщик';
}

export function ProviderDialog({
  mod,
  busy,
  curseforgeApiKeySet,
  onClose,
  onApplied
}) {
  const [checkingProvider, setCheckingProvider] = useState(null);
  const [applying, setApplying] = useState(false);
  const [searchError, setSearchError] = useState('');
  const [notFound, setNotFound] = useState(null);
  const searchRunRef = useRef(0);

  useEffect(() => {
    if (!mod) return;
    searchRunRef.current += 1;
    setCheckingProvider(null);
    setSearchError('');
    setNotFound(null);
    setApplying(false);
  }, [mod?.key]);

  useEffect(() => {
    if (!notFound) return undefined;
    const timer = window.setTimeout(() => setNotFound(null), 3000);
    return () => window.clearTimeout(timer);
  }, [notFound]);

  if (!mod) return null;

  async function applyCandidate(source, candidate) {
    setApplying(true);
    setSearchError('');
    try {
      const payload = await switchModSource({
        key: mod.key,
        source,
        displayName: mod.displayName,
        filename: mod.filename,
        projectId: candidate.id,
        slug: candidate.slug ?? undefined,
        title: candidate.title ?? undefined,
        iconUrl: candidate.iconUrl ?? undefined
      });
      onApplied(payload);
    } catch (err) {
      setSearchError(String(err));
    } finally {
      setApplying(false);
    }
  }

  function showNotFound(source, detail) {
    setNotFound({
      source,
      detail: detail || `На ${providerLabel(source)} нет точного совпадения по\u00a0файлу.`
    });
  }

  function openPlatform(source) {
    if (busy || applying || checkingProvider) return;
    if (source === 'curseforge' && !curseforgeApiKeySet) {
      showNotFound(source, 'Для проверки CurseForge нужен API key в настройках.');
      return;
    }
    setSearchError('');
    setNotFound(null);
    setCheckingProvider(source);
    const runId = searchRunRef.current + 1;
    searchRunRef.current = runId;

    const request = {
      source,
      displayName: mod.displayName,
      filename: mod.filename
    };
    void (async () => {
      try {
        const exact = await lookupProviderFingerprint(request);
        if (searchRunRef.current !== runId) return;
        if (exact) {
          await applyCandidate(source, exact);
          return;
        }
        showNotFound(source);
      } catch (err) {
        if (searchRunRef.current !== runId) return;
        showNotFound(source, String(err));
      } finally {
        if (searchRunRef.current !== runId) return;
        setCheckingProvider(null);
      }
    })();
  }

  const uiLocked = busy || applying || Boolean(checkingProvider);
  const hasProviderSource = mod.source === 'modrinth' || mod.source === 'curseforge';
  const showProviderPageLink = hasProviderSource && mod.sourceUrl;

  const providerSubtitle = modModalSubtitle(mod, { section: 'Поставщик' });

  return createPortal(
    <div className="dependencyModalBackdrop" onMouseDown={() => !uiLocked && onClose()}>
      <div className="dependencyModalStack providerModalStack" onMouseDown={(event) => event.stopPropagation()}>
        <ModModalHead
          mod={mod}
          subtitle={providerSubtitle}
          actions={
            showProviderPageLink ? (
              <a className="providerCurrentLink" href={mod.sourceUrl} target="_blank" rel="noreferrer">
                <ExternalLink size={14} />
                Открыть страницу
              </a>
            ) : null
          }
        />

        {searchError ? <p className="providerSearchError">{searchError}</p> : null}

        <div className="dependencyModal providerModal" role="dialog" aria-modal="true" aria-label="Поставщик мода">
          {providerOptions.map((item) => {
            const icon = sourceIcons[item.id]?.icon;
            const active = mod.source === item.id;
            const checking = checkingProvider === item.id;
            return (
              <button
                key={item.id}
                type="button"
                className={`providerOption${active ? ' active' : ''}`}
                onClick={() => openPlatform(item.id)}
                disabled={uiLocked}
              >
                {icon ? <img src={icon} alt="" /> : null}
                <span>{item.label}</span>
                {checking ? (
                  <LoaderCircle size={18} className="spin providerOptionSpinner" />
                ) : active ? (
                  <strong>Выбран</strong>
                ) : (
                  <span className="providerOptionHint">Выбрать</span>
                )}
              </button>
            );
          })}
        </div>
      </div>
      {notFound ? (
        <div
          className="providerNotFoundLayer"
          onMouseDown={(event) => {
            event.stopPropagation();
            if (event.target === event.currentTarget) {
              setNotFound(null);
            }
          }}
        >
          <div className="providerNotFoundModal" role="alertdialog" aria-modal="true" aria-label="Мод не найден">
            {sourceIcons[notFound.source]?.icon ? (
              <img src={sourceIcons[notFound.source].icon} alt="" className="providerNotFoundIcon" />
            ) : null}
            <strong>Мод не найден</strong>
            <p>{notFound.detail}</p>
          </div>
        </div>
      ) : null}
    </div>,
    document.body
  );
}
