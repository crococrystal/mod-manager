import { useState } from 'react';
import { createPortal } from 'react-dom';
import { DependencyModalBackdrop } from '../../components/DependencyModalBackdrop.jsx';
import { ModalModNavRail } from '../../components/ModalModNavRail.jsx';
import { getModalPortalRoot } from '../../lib/modalPortal.js';
import { installProviderVersion } from '../../api.js';
import { useProviderVersions } from '../../hooks/useProviderVersions.js';
import { modModalSubtitle } from '../../lib/modMeta.jsx';
import { ModModalHead } from './ModModalHead.jsx';
import { projectIdFor } from './versionListUtils.js';
import { VersionList } from './VersionList.jsx';

export function VersionDialog({ mod, modNav, busy, cacheScope, onClose, onInstalled }) {
  const { loading, error, target, versions } = useProviderVersions({ mod, cacheScope });
  const [installingId, setInstallingId] = useState(null);
  const [installError, setInstallError] = useState('');

  if (!mod) return null;

  const projectId = projectIdFor(mod);
  const subtitle = modModalSubtitle(mod, {
    parts: [target?.minecraftVersion, target?.loader]
  });
  const uiLocked = busy || loading || Boolean(installingId);
  const visibleError = error || installError;

  async function install(version) {
    if (!projectId) return;
    setInstallingId(version.id);
    setInstallError('');
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
      setInstallError(String(err));
    } finally {
      setInstallingId(null);
    }
  }

  return createPortal(
    <DependencyModalBackdrop uiLocked={uiLocked} onClose={onClose}>
      <div className="dependencyModalStack versionModalStack" onMouseDown={(event) => event.stopPropagation()}>
        <ModModalHead mod={mod} subtitle={subtitle} />

        <ModalModNavRail modNav={modNav} uiLocked={uiLocked}>
          {visibleError ? <p className="providerSearchError">{visibleError}</p> : null}

          <div className="versionModal" role="dialog" aria-modal="true" aria-label="Версии мода">
            <VersionList
              mod={mod}
              versions={versions}
              loading={loading}
              error={visibleError}
              busy={uiLocked}
              installingId={installingId}
              onInstall={install}
            />
          </div>
        </ModalModNavRail>
      </div>
    </DependencyModalBackdrop>,
    getModalPortalRoot()
  );
}
