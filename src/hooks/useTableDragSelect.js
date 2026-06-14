import { useEffect, useRef, useState } from 'react';

export function isInteractiveTableTarget(target) {
  return target instanceof HTMLElement
    && Boolean(target.closest('button, a, input, textarea, select, [role="button"]'));
}

export function useTableDragSelect({ enabled = true, wrapRef, onSelectDrag, getItemKey = (item) => item?.key }) {
  const [dragSelecting, setDragSelecting] = useState(false);
  const dragSelectRef = useRef({
    active: false,
    dragged: false,
    visited: new Set(),
    startItem: null
  });

  useEffect(() => {
    function handleMouseUp() {
      dragSelectRef.current.active = false;
      setDragSelecting(false);
    }

    window.addEventListener('mouseup', handleMouseUp);
    return () => window.removeEventListener('mouseup', handleMouseUp);
  }, []);

  useEffect(() => {
    const wrap = wrapRef.current;
    if (!wrap || !dragSelecting || !enabled) return undefined;

    function handleWheel(event) {
      event.preventDefault();
    }

    wrap.addEventListener('wheel', handleWheel, { passive: false });
    return () => wrap.removeEventListener('wheel', handleWheel);
  }, [dragSelecting, enabled, wrapRef]);

  function handleRowMouseDown(item, event) {
    if (!enabled || event.button !== 0 || isInteractiveTableTarget(event.target)) return;

    setDragSelecting(true);
    dragSelectRef.current = {
      active: true,
      dragged: false,
      visited: new Set(),
      startItem: item
    };
  }

  function handleRowMouseEnter(item, event) {
    if (!enabled || !onSelectDrag) return;

    const state = dragSelectRef.current;
    if (!state.active || !(event.buttons & 1)) return;

    const itemKey = getItemKey(item);
    if (!itemKey) return;

    if (!state.dragged) {
      state.dragged = true;
      const startKey = state.startItem ? getItemKey(state.startItem) : null;
      if (startKey && !state.visited.has(startKey)) {
        state.visited.add(startKey);
        onSelectDrag(state.startItem, true, { reset: true });
      }
    }

    if (state.visited.has(itemKey)) return;
    state.visited.add(itemKey);
    onSelectDrag(item, true);
  }

  function handleRowClick(item, event, onSelect) {
    if (dragSelectRef.current.dragged) {
      dragSelectRef.current.dragged = false;
      return;
    }
    onSelect?.(item, event);
  }

  return {
    dragSelecting,
    handleRowMouseDown,
    handleRowMouseEnter,
    handleRowClick
  };
}
