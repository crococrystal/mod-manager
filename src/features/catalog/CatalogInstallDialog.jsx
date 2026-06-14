import { useEffect, useState } from 'react';
import { createPortal } from 'react-dom';
import { DependencyModalBackdrop } from '../../components/DependencyModalBackdrop.jsx';
import { getModalPortalRoot } from '../../lib/modalPortal.js';
import { Check, Download, LoaderCircle } from 'lucide-react';
import { installFromCatalog } from '../../api.js';
import { catalogProviderPageUrl } from '../../lib/modMeta.jsx';
import { CatalogInstallDependencies } from './CatalogInstallDependencies.jsx';
import { CatalogInstallHeader } from './CatalogInstallHeader.jsx';
import { CatalogProjectDescriptionPanel } from './CatalogProjectDescriptionPanel.jsx';
import { useCatalogInstallPreview } from './useCatalogInstallPreview.js';
import { useCatalogProjectDetails } from './useCatalogProjectDetails.js';

const STALE_DEPENDENCY_REFRESH_MS = 60 * 60 * 1000;

export function CatalogInstallDialog({
  candidate,
  source,
  busy,
  cacheScope,
  alreadyInstalled,
  installedStateKey,
  onClose,
  onInstalled
}) {
  const [installing, setInstalling] = useState(false);
  const [installError, setInstallError] = useState('');
  const [refreshing, setRefreshing] = useState(false);
  const { preview, loading, error: previewError, reload: reloadPreview } = useCatalogInstallPreview({
    candidate,
    source,
    cacheScope,
    installedStateKey
  });
  const {
    details,
    loading: detailsLoading,
    error: detailsError,
    reload: reloadDetails
  } = useCatalogProjectDetails({
    candidate,
    source,
    cacheScope
  });

  const uiLocked = busy || installing || refreshing;
  const installDisabled = uiLocked || loading || !preview || alreadyInstalled;

  useEffect(() => {
    setInstalling(false);
    setInstallError('');
    setRefreshing(false);
  }, [candidate, source]);

  useEffect(() => {
    if (!candidate || !source) return undefined;
    function handleEscape(event) {
      if (event.key !== 'Escape') return;
      if (uiLocked) return;
      onClose?.();
    }
    document.addEventListener('keydown', handleEscape);
    return () => document.removeEventListener('keydown', handleEscape);
  }, [candidate, source, uiLocked, onClose]);

  if (!candidate || !source) return null;

  const dependencies = preview?.dependencies ?? [];
  const error = installError || previewError || detailsError;
  const project = details ?? preview;
  const description = details?.description ?? preview?.description;
  const targetParts = [
    project?.target?.minecraftVersion,
    project?.target?.loader
  ].filter(Boolean);
  const pageUrl = catalogProviderPageUrl(
    source,
    candidate.id,
    project?.slug ?? candidate.slug
  );

  async function handleInstall() {
    setInstalling(true);
    setInstallError('');
    try {
      let nextPreview = preview;
      const previewAgeMs = preview?.checkedAtMs ? Date.now() - preview.checkedAtMs : null;
      if (previewAgeMs == null || previewAgeMs > STALE_DEPENDENCY_REFRESH_MS) {
        nextPreview = (await reloadPreview(true)) ?? nextPreview;
      }
      const result = await installFromCatalog({
        source,
        projectId: candidate.id,
        versionId: nextPreview?.version?.id
      });
      onInstalled(result);
    } catch (err) {
      setInstallError(String(err));
    } finally {
      setInstalling(false);
    }
  }

  async function handleRefresh() {
    if (uiLocked) return;
    setRefreshing(true);
    setInstallError('');
    try {
      await Promise.all([reloadDetails(true), reloadPreview(true)]);
    } finally {
      setRefreshing(false);
    }
  }

  return createPortal(
    <DependencyModalBackdrop uiLocked={uiLocked} onClose={onClose}>
      <div className="dependencyModalStack catalogInstallStack">
        <CatalogInstallHeader
          source={source}
          candidate={candidate}
          preview={project}
          targetParts={targetParts}
          pageUrl={pageUrl}
          uiLocked={uiLocked}
          refreshing={refreshing}
          onRefresh={handleRefresh}
        />

        <div className="catalogInstallBody" role="dialog" aria-modal="true" aria-label="Установка мода">
          {error ? <p className="providerSearchError">{error}</p> : null}
          <div className="catalogInstallContent">
            <CatalogProjectDescriptionPanel
              className="catalogInstallDescriptionWrap"
              description={description}
              loading={detailsLoading}
            />
            {preview ? (
              <>
                {!loading ? <CatalogInstallDependencies dependencies={dependencies} /> : null}
              </>
            ) : null}
          </div>
          <button
            type="button"
            className="catalogInstallAction"
            onClick={handleInstall}
            disabled={installDisabled}
          >
            {alreadyInstalled ? (
              <Check size={18} />
            ) : installing || loading ? (
              <LoaderCircle className="spin" size={18} />
            ) : (
              <Download size={18} />
            )}
            {alreadyInstalled ? 'Уже установлен' : loading ? 'Проверка зависимостей...' : 'Установить'}
            {!alreadyInstalled && !loading && dependencies.some((dep) => dep.status !== 'installed')
              ? ` (+${dependencies.filter((dep) => dep.status !== 'installed').length} завис.)`
              : ''}
          </button>
        </div>
      </div>
    </DependencyModalBackdrop>,
    getModalPortalRoot()
  );
}
