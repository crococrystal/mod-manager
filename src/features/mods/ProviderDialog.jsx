import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { DependencyModalBackdrop } from '../../components/DependencyModalBackdrop.jsx';
import { ModalModNavRail } from '../../components/ModalModNavRail.jsx';
import { getModalPortalRoot } from '../../lib/modalPortal.js';
import { LoaderCircle } from 'lucide-react';
import {
  lookupProviderFingerprint,
  searchProviderCandidates,
  switchModSource
} from '../../api.js';
import { sourceIcons, modModalSubtitle } from '../../lib/modMeta.jsx';
import { CatalogProjectDescriptionPanel } from '../catalog/CatalogProjectDescriptionPanel.jsx';
import { useCatalogProjectDetails } from '../catalog/useCatalogProjectDetails.js';
import { ModModalHead } from './ModModalHead.jsx';

const providerOptions = [
  { id: 'modrinth', label: 'Modrinth' },
  { id: 'curseforge', label: 'CurseForge' }
];

function providerLabel(source) {
  return providerOptions.find((item) => item.id === source)?.label ?? 'Поставщик';
}

function projectIdForSource(mod, source) {
  if (!mod) return null;
  if (source === 'modrinth') return mod.modrinthId;
  if (source === 'curseforge') return mod.curseforgeId;
  return null;
}

function projectCandidateForSource(mod, source) {
  const id = projectIdForSource(mod, source);
  if (!id) return null;
  return {
    id,
    title: mod.providerTitle || mod.displayName,
    iconUrl: mod.coverUrl
  };
}

function ProviderSearchStatus({ state, uiLocked, onPick }) {
  if (!state) return null;
  const icon = sourceIcons[state.source]?.icon;

  return (
    <div className="providerSearchStatus">
      {icon ? <img src={icon} alt="" className="providerNotFoundIcon" /> : null}
      <strong>Мод не найден</strong>
      <p>{state.detail}</p>
      {!state.loading && state.candidates?.length ? (
        <>
          <p className="providerFallbackHint">
            Выберите подходящий проект. Мы привяжем поставщика, текущий файл не заменяем.
          </p>
          <div className="providerFallbackList">
            {state.candidates.map((candidate) => (
              <button
                key={candidate.id}
                type="button"
                className="providerFallbackOption"
                onClick={() => onPick(state.source, candidate)}
                disabled={uiLocked}
              >
                {candidate.iconUrl ? (
                  <img src={candidate.iconUrl} alt="" />
                ) : (
                  <div className="providerFallbackIconPlaceholder" />
                )}
                <span>{candidate.title}</span>
                <small>Привязать</small>
              </button>
            ))}
          </div>
        </>
      ) : null}
      {!state.loading && !state.candidates?.length && !state.error ? (
        <p className="providerFallbackHint">Похожих проектов не нашли.</p>
      ) : null}
      {!state.loading && state.error ? <p className="providerFallbackError">{state.error}</p> : null}
    </div>
  );
}

function noFileMatchMessage(source) {
  return `На ${providerLabel(source)} нет точного совпадения по\u00a0файлу.`;
}

function noFileOrNameMatchMessage(source) {
  return `На ${providerLabel(source)} нет совпадения по\u00a0файлу и\u00a0имени.`;
}

export function ProviderDialog({
  mod,
  modNav,
  busy,
  curseforgeApiKeySet,
  onClose,
  onApplied
}) {
  const [checkingProvider, setCheckingProvider] = useState(null);
  const [applying, setApplying] = useState(false);
  const [searchError, setSearchError] = useState('');
  const [notFound, setNotFound] = useState(null);
  const [selectedSource, setSelectedSource] = useState(null);
  const searchRunRef = useRef(0);
  const descriptionSource = selectedSource ?? mod?.source ?? null;
  const descriptionCandidate = projectCandidateForSource(mod, descriptionSource);
  const {
    details: descriptionDetails,
    loading: descriptionLoading,
    error: descriptionError
  } = useCatalogProjectDetails({
    candidate: descriptionCandidate,
    source: descriptionCandidate ? descriptionSource : null,
    cacheScope: mod?.key
  });

  useEffect(() => {
    if (!mod) return;
    searchRunRef.current += 1;
    setCheckingProvider(null);
    setSearchError('');
    setNotFound(null);
    setApplying(false);
    setSelectedSource(mod.source);
  }, [mod?.key, mod?.source]);

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
      setNotFound(null);
    } catch (err) {
      setSearchError(String(err));
    } finally {
      setApplying(false);
    }
  }

  function showNotFound(source, detail) {
    setNotFound({
      source,
      detail: detail || noFileMatchMessage(source),
      loading: false,
      candidates: [],
      error: ''
    });
  }

  async function searchByNameFallback(source, request, runId) {
    try {
      const candidates = await searchProviderCandidates(request);
      if (searchRunRef.current !== runId) return;
      const exactCandidates = candidates.filter((candidate) => candidate.matchScore >= 1000);
      if (exactCandidates.length === 1) {
        await applyCandidate(source, exactCandidates[0]);
        return;
      }
      setNotFound({
        source,
        detail: noFileMatchMessage(source),
        loading: false,
        candidates: exactCandidates.length ? exactCandidates : candidates,
        error: ''
      });
    } catch (err) {
      if (searchRunRef.current !== runId) return;
      setNotFound({
        source,
        detail: noFileOrNameMatchMessage(source),
        loading: false,
        candidates: [],
        error: ''
      });
    }
  }

  function openPlatform(source) {
    if (busy || applying || checkingProvider) return;
    const knownCandidate = projectCandidateForSource(mod, source);
    if (knownCandidate && source !== mod.source) {
      setSelectedSource(source);
      void applyCandidate(source, knownCandidate);
      return;
    }
    if (source === mod.source) {
      setSelectedSource(source);
      setNotFound(null);
      return;
    }
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
        await searchByNameFallback(source, request, runId);
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
  const providerIcon = sourceIcons[mod.source]?.icon;

  const providerSubtitle = modModalSubtitle(mod, { section: 'Поставщик' });
  const description = descriptionDetails?.description;

  return createPortal(
    <DependencyModalBackdrop uiLocked={uiLocked} onClose={onClose}>
      <div className="dependencyModalStack providerModalStack" onMouseDown={(event) => event.stopPropagation()}>
        <ModModalHead
          mod={mod}
          subtitle={providerSubtitle}
          align="end"
          titleAlign="end"
          actions={
            showProviderPageLink ? (
              <a className="providerCurrentLink" href={mod.sourceUrl} target="_blank" rel="noreferrer">
                {providerIcon ? <img src={providerIcon} alt="" className="providerCurrentLinkIcon" /> : null}
                Открыть страницу
              </a>
            ) : null
          }
        />

        <ModalModNavRail modNav={modNav} uiLocked={uiLocked}>
          {searchError ? <p className="providerSearchError">{searchError}</p> : null}

          <div className="dependencyModal providerModal" role="dialog" aria-modal="true" aria-label="Поставщик мода">
            {providerOptions.map((item) => {
              const icon = sourceIcons[item.id]?.icon;
              const active = mod.source === item.id;
              const selected = Boolean(descriptionCandidate) && descriptionSource === item.id;
              const checking = checkingProvider === item.id;
              return (
                <button
                  key={item.id}
                  type="button"
                  className={`providerOption${active ? ' active' : ''}${selected ? ' selected' : ''}`}
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
            <CatalogProjectDescriptionPanel
              className="providerDescriptionPanel"
              description={description}
              loading={descriptionLoading}
              emptyMessage={
                descriptionError ||
                (descriptionCandidate
                  ? 'Описание не найдено.'
                  : 'Сначала привяжите проект на этой площадке.')
              }
            >
              {notFound ? (
                <ProviderSearchStatus state={notFound} uiLocked={uiLocked} onPick={applyCandidate} />
              ) : null}
            </CatalogProjectDescriptionPanel>
          </div>
        </ModalModNavRail>
      </div>
    </DependencyModalBackdrop>,
    getModalPortalRoot()
  );
}
