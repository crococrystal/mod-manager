import { useEffect } from 'react';
import { useProviderVersions } from '../../hooks/useProviderVersions.js';
import { modModalSubtitle } from '../../lib/modMeta.jsx';
import { ModCover } from './ModCover.jsx';
import { UpdatesVersionPanelEmpty } from './UpdatesVersionPanelEmpty.jsx';
import { VersionList } from './VersionList.jsx';

export function UpdatesVersionPanel({
  mod,
  cacheScope,
  busy,
  updateCount = 0,
  updatingAll = false,
  updatesLoading = false,
  updateAllError = '',
  installingVersionId = null,
  onUpdateAllRequest,
  onInstallRequest,
  onClearMod
}) {
  const { loading, error, target, versions } = useProviderVersions({ mod, cacheScope });
  const uiLocked = busy || loading || Boolean(installingVersionId);

  useEffect(() => {
    if (!mod || !onClearMod || uiLocked) return undefined;
    function handleEscape(event) {
      if (event.key === 'Escape') onClearMod();
    }
    document.addEventListener('keydown', handleEscape);
    return () => document.removeEventListener('keydown', handleEscape);
  }, [mod, onClearMod, uiLocked]);

  const subtitle = mod
    ? modModalSubtitle(mod, {
        parts: [target?.minecraftVersion, target?.loader]
      })
    : null;
  const visibleError = mod ? error : '';
  const showUpdateAll = updateCount > 0 && !updatingAll && !updatesLoading;

  return (
    <div className={`updatesVersionPanel${mod ? ' updatesVersionPanel--withMod' : ''}`}>
      {mod ? (
        <>
          <div className="updatesVersionHead">
            <ModCover mod={mod} size="tile" />
            <div className="updatesVersionHeadText">
              {subtitle ? <p className="updatesVersionSubtitle">{subtitle}</p> : null}
              <h2>{mod.displayName}</h2>
            </div>
          </div>
          {visibleError ? <p className="providerSearchError">{visibleError}</p> : null}
          <div className="updatesVersionList scrollArea">
            <VersionList
              mod={mod}
              versions={versions}
              loading={loading}
              error={visibleError}
              busy={uiLocked}
              installingId={installingVersionId}
              compact
              onInstall={onInstallRequest}
            />
          </div>
        </>
      ) : null}
      {showUpdateAll ? (
        <UpdatesVersionPanelEmpty
          placement={mod ? 'footer' : 'center'}
          updateCount={updateCount}
          busy={busy}
          error={updateAllError}
          onUpdateAllRequest={onUpdateAllRequest}
        />
      ) : null}
    </div>
  );
}
