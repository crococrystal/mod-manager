import { useEffect } from 'react';
import { createPortal } from 'react-dom';
import { BookOpen, RefreshCw, Tag, Wrench } from 'lucide-react';
import { DependencyModalBackdrop } from '../../components/DependencyModalBackdrop.jsx';
import { ModalModNavRail } from '../../components/ModalModNavRail.jsx';
import { getModalPortalRoot } from '../../lib/modalPortal.js';
import { canRefreshProviderLabels, tagsForMode } from '../../lib/labelDisplay.js';
import { sideOptions, sourceIcons, modModalSubtitle } from '../../lib/modMeta.jsx';
import { ModCover } from './ModCover.jsx';

function currentTags(mod) {
  return {
    side: mod.side === 'unknown' ? 'universal' : mod.side ?? 'universal',
    sideMode: mod.sideMode === 'manual' ? 'manual' : 'auto',
    library: Boolean(mod.library),
    technical: Boolean(mod.technical)
  };
}

export function TagsDialog({
  mod,
  modNav,
  savingKey,
  labelsRefreshing = false,
  onClose,
  onSave,
  onRefresh
}) {
  useEffect(() => {
    if (!mod) return undefined;
    function handleEscape(event) {
      if (event.key === 'Escape') {
        onClose();
      }
    }
    document.addEventListener('keydown', handleEscape);
    return () => document.removeEventListener('keydown', handleEscape);
  }, [mod, onClose]);

  if (!mod) return null;

  const tags = currentTags(mod);
  const saving = savingKey === mod.key;
  const refreshing = labelsRefreshing === mod.key;
  const uiLocked = saving || refreshing;
  const providerMode = tags.sideMode === 'auto';
  const refreshAllowed = canRefreshProviderLabels(mod);
  const providerIcon =
    mod.source === 'modrinth' || mod.source === 'curseforge' ? sourceIcons[mod.source]?.icon : null;

  async function apply(patch) {
    if (uiLocked) return;
    await onSave(mod.key, patch);
  }

  async function toggleTagSourceMode() {
    if (uiLocked) return;
    const nextMode = providerMode ? 'manual' : 'auto';
    if (!providerMode && !refreshAllowed) return;
    await onSave(
      mod.key,
      { sideMode: nextMode },
      { optimistic: tagsForMode(mod, nextMode) }
    );
  }

  return createPortal(
    <DependencyModalBackdrop uiLocked={uiLocked} onClose={onClose}>
      <div className="dependencyModalStack tagsModalStack" onMouseDown={(event) => event.stopPropagation()}>
        <div className="dependencyModalHead tagsModalHead">
          <ModCover mod={mod} size="tile" />
          <div className="dependencyModalHeadText">
            <p className="dependencyModalSubtitle">{modModalSubtitle(mod, { section: 'Метки' })}</p>
            <h3 className="dependencyModalTitle">{mod.displayName}</h3>
          </div>
          <div className="tagsModalHeadActions">
            {providerMode && onRefresh ? (
              <button
                type="button"
                className="coverActionIcon"
                onClick={() => void onRefresh(mod.key)}
                disabled={uiLocked || !refreshAllowed}
                title="Обновить с поставщика"
                aria-label="Обновить с поставщика"
              >
                <RefreshCw size={20} className={refreshing ? 'spin' : ''} />
              </button>
            ) : null}
            <button
              type="button"
              className={`coverActionIcon tagSourceModeButton${providerMode ? ' active' : ''}`}
              onClick={() => void toggleTagSourceMode()}
              disabled={uiLocked || (!providerMode && !refreshAllowed)}
              title={providerMode ? 'Свои метки' : 'Метки поставщика'}
              aria-label={providerMode ? 'Свои метки' : 'Метки поставщика'}
              aria-pressed={providerMode}
            >
              {providerMode && providerIcon ? (
                <img src={providerIcon} alt="" className="tagSourceModeIcon" />
              ) : (
                <Tag size={20} />
              )}
            </button>
          </div>
        </div>

        <ModalModNavRail modNav={modNav} uiLocked={uiLocked}>
          <div className="dependencyModal tagsModal" role="dialog" aria-modal="true" aria-label="Метки мода">
            <div className="tagsModalRow">
            <div className={`tagsModalGroup${providerMode ? ' tagButtonsProvider' : ''}`}>
              {sideOptions.map((item) => {
                const Icon = item.icon;
                return (
                  <button
                    key={item.id}
                    type="button"
                    className={tags.side === item.id ? 'active' : ''}
                    onClick={() => {
                      if (providerMode) return;
                      void apply({ side: item.id, sideMode: 'manual' });
                    }}
                    disabled={uiLocked}
                    title={item.label}
                  >
                    <Icon className={`tagIcon ${item.tone}`} size={24} />
                  </button>
                );
              })}
            </div>
            <div className={`tagsModalGroup${providerMode ? ' tagButtonsProvider' : ''}`}>
              <button
                type="button"
                className={tags.library ? 'active' : ''}
                onClick={() => {
                  if (providerMode) return;
                  void apply({ library: !tags.library, sideMode: 'manual' });
                }}
                disabled={uiLocked}
                title="Библиотеки"
              >
                <BookOpen className="tagIcon library" size={24} />
              </button>
              <button
                type="button"
                className={tags.technical ? 'active' : ''}
                onClick={() => {
                  if (providerMode) return;
                  void apply({ technical: !tags.technical, sideMode: 'manual' });
                }}
                disabled={uiLocked}
                title="Оптимизации"
              >
                <Wrench className="tagIcon technical" size={24} />
              </button>
            </div>
            </div>
          </div>
        </ModalModNavRail>
      </div>
    </DependencyModalBackdrop>,
    getModalPortalRoot()
  );
}
