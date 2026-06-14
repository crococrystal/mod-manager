import { useCallback, useEffect, useState } from 'react';
import { LoaderUpdateRow } from './LoaderUpdateRow.jsx';
import {
  applyNeoForgeUpdate,
  getNeoForgeVersionCatalog,
  refreshNeoForgeRow
} from './loaderUpdateApi.js';
import { loaderLabel, loaderLogo } from './loaderUpdateLogos.js';
import { pickDefaultTargetVersion } from './loaderUpdateUi.js';

export function LoaderUpdatePanel({ sshHost, disabled, actionBusy }) {
  const [catalog, setCatalog] = useState(null);
  const [catalogError, setCatalogError] = useState('');
  const [catalogLoading, setCatalogLoading] = useState(false);
  const [targetVersion, setTargetVersion] = useState('');
  const [clientVersion, setClientVersion] = useState(null);
  const [serverVersion, setServerVersion] = useState(null);
  const [clientChecked, setClientChecked] = useState(false);
  const [serverChecked, setServerChecked] = useState(false);
  const [clientChecking, setClientChecking] = useState(false);
  const [serverChecking, setServerChecking] = useState(false);
  const [clientApplying, setClientApplying] = useState(false);
  const [serverApplying, setServerApplying] = useState(false);
  const [clientError, setClientError] = useState('');
  const [serverError, setServerError] = useState('');

  const hasSsh = Boolean(sshHost?.trim());
  const loaderKey = catalog?.loader || 'neoforge';
  const logo = loaderLogo(loaderKey);
  const loaderName = loaderLabel(loaderKey);
  const applySupported = catalog?.applySupported !== false;
  const versions = catalog?.availableVersions ?? [];
  const rowBusy =
    clientChecking || serverChecking || clientApplying || serverApplying;
  const busy = catalogLoading || rowBusy || actionBusy;
  const panelDisabled = disabled || busy;
  const selectDisabled = panelDisabled || catalogLoading || !versions.length;
  const selectPlaceholder = catalogLoading
    ? `Загрузка версий ${loaderName}…`
    : catalogError && !versions.length
      ? catalogError
      : '—';

  const loadCatalog = useCallback(async () => {
    setCatalogLoading(true);
    setCatalogError('');
    try {
      const result = await getNeoForgeVersionCatalog();
      setCatalog(result);
      setTargetVersion((current) => current || pickDefaultTargetVersion(result));
      if (!result?.ok) {
        setCatalogError(
          result?.message || `Не удалось загрузить версии ${loaderLabel(result?.loader)}.`
        );
      }
    } catch (err) {
      setCatalog(null);
      setCatalogError(String(err));
    } finally {
      setCatalogLoading(false);
    }
  }, []);

  const refreshRow = useCallback(
    async (row) => {
      const setChecking = row === 'client' ? setClientChecking : setServerChecking;
      const setVersion = row === 'client' ? setClientVersion : setServerVersion;
      const setChecked = row === 'client' ? setClientChecked : setServerChecked;
      const setError = row === 'client' ? setClientError : setServerError;
      setChecking(true);
      setError('');
      try {
        const result = await refreshNeoForgeRow({
          row,
          sshHost: sshHost?.trim() || undefined
        });
        setVersion(result?.version ?? null);
        setChecked(true);
      } catch (err) {
        setError(String(err));
      } finally {
        setChecking(false);
      }
    },
    [sshHost]
  );

  const updateRow = useCallback(
    async (row) => {
      if (!targetVersion) return;
      const setApplying = row === 'client' ? setClientApplying : setServerApplying;
      const setVersion = row === 'client' ? setClientVersion : setServerVersion;
      const setChecked = row === 'client' ? setClientChecked : setServerChecked;
      const setError = row === 'client' ? setClientError : setServerError;
      setApplying(true);
      setError('');
      try {
        const result = await applyNeoForgeUpdate({
          targetVersion,
          updateClient: row === 'client',
          updateServer: row === 'server',
          sshHost: sshHost?.trim() || undefined
        });
        const nextVersion =
          row === 'client' ? result?.clientVersion : result?.serverVersion;
        if (nextVersion) setVersion(nextVersion);
        setChecked(true);
      } catch (err) {
        setError(String(err));
      } finally {
        setApplying(false);
      }
    },
    [sshHost, targetVersion]
  );

  const handleRowAction = useCallback(
    (row) => {
      const checked = row === 'client' ? clientChecked : serverChecked;
      const currentVersion = row === 'client' ? clientVersion : serverVersion;
      const upToDate =
        checked &&
        Boolean(currentVersion?.trim()) &&
        Boolean(targetVersion?.trim()) &&
        currentVersion === targetVersion;
      const needsUpdate =
        checked &&
        Boolean(currentVersion?.trim()) &&
        Boolean(targetVersion?.trim()) &&
        currentVersion !== targetVersion;

      if (upToDate) {
        return;
      }

      if (needsUpdate && applySupported) {
        void updateRow(row);
        return;
      }

      void refreshRow(row);
    },
    [
      applySupported,
      clientChecked,
      clientVersion,
      refreshRow,
      serverChecked,
      serverVersion,
      targetVersion,
      updateRow
    ]
  );

  useEffect(() => {
    void loadCatalog();
  }, [loadCatalog]);

  return (
    <div className="field loaderUpdateField">
      <div className="fieldHeader loaderUpdateFieldHeader">
        <img src={logo.src} alt={logo.alt} className="loaderUpdateLogo" />
        <select
          id="loaderUpdateTargetVersion"
          className={[
            'loaderUpdateVersionSelect',
            catalogError && !versions.length && !catalogLoading
              ? 'loaderUpdateVersionSelect--error'
              : ''
          ]
            .filter(Boolean)
            .join(' ')}
          aria-label={`Целевая версия ${loaderName}`}
          value={targetVersion}
          disabled={selectDisabled}
          onChange={(event) => setTargetVersion(event.target.value)}
        >
          {versions.length ? (
            versions.map((version) => (
              <option key={version} value={version}>
                {version}
                {version === catalog?.latestVersion ? ' · latest' : ''}
              </option>
            ))
          ) : (
            <option value="">{selectPlaceholder}</option>
          )}
        </select>
      </div>

      <div className="loaderUpdateRows">
        <LoaderUpdateRow
          label="Клиент"
          currentVersion={clientVersion}
          targetVersion={targetVersion}
          checked={clientChecked}
          applySupported={applySupported}
          checking={clientChecking}
          applying={clientApplying}
          error={clientError}
          actionDisabled={panelDisabled}
          onAction={() => handleRowAction('client')}
        />
        {hasSsh && applySupported ? (
          <LoaderUpdateRow
            label="Сервер"
            currentVersion={serverVersion}
            targetVersion={targetVersion}
            checked={serverChecked}
            applySupported={applySupported}
            checking={serverChecking}
            applying={serverApplying}
            error={serverError}
            actionDisabled={panelDisabled}
            onAction={() => handleRowAction('server')}
          />
        ) : null}
      </div>
    </div>
  );
}
