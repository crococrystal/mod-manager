import { useCallback, useEffect, useState } from 'react';
import { Save } from 'lucide-react';
import { Modal } from '../../components/Modal.jsx';
import { IconButton } from '../../components/Button.jsx';
import {
  readServerLaunchScript,
  writeServerLaunchScript
} from './serverControlApi.js';

export function LaunchScriptEditorModal({
  open,
  sshHost,
  serverRootPath,
  scriptName,
  onClose
}) {
  const [content, setContent] = useState('');
  const [path, setPath] = useState('');
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');
  const [dirty, setDirty] = useState(false);

  const loadScript = useCallback(async () => {
    setLoading(true);
    setError('');
    try {
      const result = await readServerLaunchScript({ sshHost, serverRootPath });
      setContent(result?.content ?? '');
      setPath(result?.path ?? '');
      setDirty(false);
    } catch (err) {
      setContent('');
      setPath('');
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, [serverRootPath, sshHost]);

  useEffect(() => {
    if (!open) return;
    void loadScript();
  }, [loadScript, open]);

  const handleSave = useCallback(async () => {
    setSaving(true);
    setError('');
    try {
      const result = await writeServerLaunchScript({
        sshHost,
        serverRootPath,
        content
      });
      setPath(result?.path ?? path);
      setDirty(false);
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  }, [content, path, serverRootPath, sshHost]);

  if (!open) {
    return null;
  }

  const title = scriptName?.trim() ? scriptName.trim() : 'Скрипт запуска';

  return (
    <Modal
      title={title}
      subtitle={path || undefined}
      size="wide"
      className="launchScriptModal"
      onClose={onClose}
      footer={
        <div className="launchScriptModalFooter">
          {error ? (
            <p className="launchScriptModalError" role="alert">
              {error}
            </p>
          ) : null}
          <IconButton
            icon={Save}
            label={saving ? 'Сохранение…' : 'Сохранить'}
            className="launchScriptModalSave"
            disabled={loading || saving || !dirty}
            onClick={() => void handleSave()}
          />
        </div>
      }
    >
      <textarea
        className="launchScriptEditor"
        value={content}
        disabled={loading || saving}
        spellCheck={false}
        autoCapitalize="off"
        autoCorrect="off"
        placeholder={loading ? 'Загрузка…' : '@echo off\ncd /d "%~dp0"\ncall run.bat nogui'}
        onChange={(event) => {
          setContent(event.target.value);
          setDirty(true);
          setError('');
        }}
      />
    </Modal>
  );
}
