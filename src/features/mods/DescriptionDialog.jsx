import { useCallback, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { DependencyModalBackdrop } from '../../components/DependencyModalBackdrop.jsx';
import { ModalModNavRail } from '../../components/ModalModNavRail.jsx';
import { getModalPortalRoot } from '../../lib/modalPortal.js';
import { modModalSubtitle } from '../../lib/modMeta.jsx';
import { ModModalHead } from './ModModalHead.jsx';

const SAVE_DELAY_MS = 400;

export function DescriptionDialog({ mod, modNav, busy, savingKey, onClose, onSave }) {
  const [description, setDescription] = useState(mod?.description ?? '');
  const textareaRef = useRef(null);
  const saveTimerRef = useRef(null);
  const lastSavedRef = useRef('');
  const modKeyRef = useRef(mod?.key);

  modKeyRef.current = mod?.key;

  useEffect(() => {
    const value = mod?.description ?? '';
    setDescription(value);
    lastSavedRef.current = value;
    window.setTimeout(() => textareaRef.current?.focus(), 0);
  }, [mod?.key, mod?.description]);

  useEffect(
    () => () => {
      if (saveTimerRef.current) {
        window.clearTimeout(saveTimerRef.current);
      }
    },
    []
  );

  const flushSave = useCallback(async () => {
    if (!modKeyRef.current) return;
    if (saveTimerRef.current) {
      window.clearTimeout(saveTimerRef.current);
      saveTimerRef.current = null;
    }
    const value = textareaRef.current?.value ?? description;
    if (value === lastSavedRef.current) return;
    await onSave(modKeyRef.current, { description: value });
    lastSavedRef.current = value;
  }, [description, onSave]);

  const scheduleSave = useCallback(
    (value) => {
      if (saveTimerRef.current) {
        window.clearTimeout(saveTimerRef.current);
      }
      saveTimerRef.current = window.setTimeout(() => {
        saveTimerRef.current = null;
        if (value === lastSavedRef.current || !modKeyRef.current) return;
        void onSave(modKeyRef.current, { description: value }).then(() => {
          lastSavedRef.current = value;
        });
      }, SAVE_DELAY_MS);
    },
    [onSave]
  );

  if (!mod) return null;

  const saving = savingKey === mod.key;
  const uiLocked = busy || saving;

  async function handleClose() {
    if (uiLocked) return;
    await flushSave();
    onClose();
  }

  return createPortal(
    <DependencyModalBackdrop uiLocked={busy} onClose={() => void handleClose()}>
      <div className="dependencyModalStack descriptionModalStack" onMouseDown={(event) => event.stopPropagation()}>
        <ModModalHead mod={mod} subtitle={modModalSubtitle(mod, { section: 'Описание' })} />
        <ModalModNavRail modNav={modNav} uiLocked={busy}>
          <textarea
            ref={textareaRef}
            className="descriptionModalInput"
            value={description}
            onChange={(event) => {
              const value = event.target.value;
              setDescription(value);
              scheduleSave(value);
            }}
            onBlur={() => void flushSave()}
            disabled={busy}
            placeholder="Что это за мод, зачем он в сборке"
            aria-label="Описание мода"
          />
        </ModalModNavRail>
      </div>
    </DependencyModalBackdrop>,
    getModalPortalRoot()
  );
}
