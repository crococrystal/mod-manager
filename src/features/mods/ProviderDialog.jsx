import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { ArrowLeft, ExternalLink } from 'lucide-react';
import {
  lookupProviderFingerprint,
  searchProviderCandidates,
  switchModSource
} from '../../api.js';
import { sourceIcons } from '../../lib/modMeta.jsx';
import { ModCover } from './ModCover.jsx';

const providerOptions = [
  { id: 'modrinth', label: 'Modrinth' },
  { id: 'curseforge', label: 'CurseForge' }
];

const STRONG_MATCH_SCORE = 800;

function prependCandidate(list, exact) {
  if (!exact) return list;
  const rest = list.filter((item) => item.id !== exact.id);
  return [exact, ...rest];
}

export function ProviderDialog({
  mod,
  busy,
  curseforgeApiKeySet,
  onClose,
  onApplied
}) {
  const [step, setStep] = useState('providers');
  const [platform, setPlatform] = useState(null);
  const [candidates, setCandidates] = useState([]);
  const [searchLoading, setSearchLoading] = useState(false);
  const [applying, setApplying] = useState(false);
  const [searchError, setSearchError] = useState('');
  const searchRunRef = useRef(0);

  useEffect(() => {
    if (!mod) return;
    searchRunRef.current += 1;
    setStep('providers');
    setPlatform(null);
    setCandidates([]);
    setSearchError('');
    setSearchLoading(false);
  }, [mod?.key]);

  if (!mod) return null;

  function openPlatform(source) {
    if (source === 'curseforge' && !curseforgeApiKeySet) {
      setSearchError('Для CurseForge нужен API key в настройках.');
      return;
    }
    setPlatform(source);
    setStep('results');
    setSearchError('');
    setCandidates([]);
    setSearchLoading(true);
    const runId = searchRunRef.current + 1;
    searchRunRef.current = runId;

    const request = {
      source,
      displayName: mod.displayName,
      filename: mod.filename
    };
    let fingerprintStarted = false;
    const lookupFingerprint = () => {
      if (fingerprintStarted) return;
      fingerprintStarted = true;
      void lookupProviderFingerprint(request)
        .then((exact) => {
          if (searchRunRef.current !== runId) return;
          if (exact) {
            setSearchError('');
            setCandidates((current) => prependCandidate(current, exact));
          }
        })
        .catch(() => {});
    };

    void searchProviderCandidates(request)
      .then((items) => {
        if (searchRunRef.current !== runId) return;
        setCandidates(items);
        if ((items[0]?.matchScore ?? 0) < STRONG_MATCH_SCORE) {
          lookupFingerprint();
        }
      })
      .catch((err) => {
        if (searchRunRef.current !== runId) return;
        setSearchError(String(err));
        lookupFingerprint();
      })
      .finally(() => {
        if (searchRunRef.current !== runId) return;
        setSearchLoading(false);
      });
  }

  async function applyCandidate(candidate) {
    setApplying(true);
    setSearchError('');
    try {
      const payload = await switchModSource({
        key: mod.key,
        source: platform,
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

  const uiLocked = busy || applying;
  const showList = candidates.length > 0;
  const hasProviderSource = mod.source === 'modrinth' || mod.source === 'curseforge';
  const showProviderPageLink = step === 'providers' && hasProviderSource && mod.sourceUrl;

  return createPortal(
    <div className="dependencyModalBackdrop" onMouseDown={() => !uiLocked && onClose()}>
      <div className="dependencyModalStack providerModalStack" onMouseDown={(event) => event.stopPropagation()}>
        <div className="dependencyModalHead">
          <ModCover mod={mod} size="tile" />
          <div className="dependencyModalHeadText">
            <p className="dependencyModalSubtitle">{mod.displayName}</p>
            <div className="dependencyModalTitleRow">
              <h3 className="dependencyModalTitle">
                {step === 'providers' ? 'Поставщик' : providerOptions.find((item) => item.id === platform)?.label}
              </h3>
              {showProviderPageLink ? (
                <a className="providerCurrentLink" href={mod.sourceUrl} target="_blank" rel="noreferrer">
                  <ExternalLink size={14} />
                  Открыть страницу
                </a>
              ) : null}
            </div>
          </div>
        </div>

        {searchError ? <p className="providerSearchError">{searchError}</p> : null}

        {step === 'providers' ? (
          <div className="dependencyModal providerModal" role="dialog" aria-modal="true" aria-label="Поставщик мода">
            {providerOptions.map((item) => {
              const icon = sourceIcons[item.id]?.icon;
              const active = mod.source === item.id;
              return (
                <button
                  key={item.id}
                  type="button"
                  className={`providerOption${active ? ' active' : ''}`}
                  onClick={() => openPlatform(item.id)}
                  disabled={busy || searchLoading}
                >
                  {icon ? <img src={icon} alt="" /> : null}
                  <span>{item.label}</span>
                  {active ? <strong>Выбран</strong> : <span className="providerOptionHint">Выбрать</span>}
                </button>
              );
            })}
          </div>
        ) : (
          <div className="providerResults" role="dialog" aria-modal="true" aria-label="Выбор мода на поставщике">
            <button
              type="button"
              className="providerBack"
              onClick={() => {
                if (uiLocked) return;
                setStep('providers');
                setPlatform(null);
                setCandidates([]);
                setSearchError('');
                setSearchLoading(false);
                searchRunRef.current += 1;
              }}
              disabled={uiLocked}
            >
              <ArrowLeft size={14} />
              Назад
            </button>

            {showList ? (
              <ul className="providerCandidateList">
                {candidates.map((candidate) => (
                  <li key={`${platform}-${candidate.id}`}>
                    <button
                      type="button"
                      className="providerCandidate"
                      onClick={() => applyCandidate(candidate)}
                      disabled={uiLocked}
                    >
                      {candidate.iconUrl ? (
                        <img src={candidate.iconUrl} alt="" className="providerCandidateIcon" />
                      ) : (
                        <span className="providerCandidateIcon placeholder" aria-hidden="true" />
                      )}
                      <span className="providerCandidateText">
                        <strong>{candidate.title}</strong>
                        {candidate.slug ? <span>{candidate.slug}</span> : null}
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            ) : null}
          </div>
        )}
      </div>
    </div>,
    document.body
  );
}
