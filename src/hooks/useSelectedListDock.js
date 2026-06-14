import { useLayoutEffect, useRef } from 'react';

export function useSelectedListDock({
  active,
  wrapRef,
  dockRef,
  rowSelector,
  topLimitSelector,
  fallbackTopInset = 0,
  scrollIntoViewKey = null,
  deps = []
}) {
  const dockFrameRef = useRef(0);

  useLayoutEffect(() => {
    const wrap = wrapRef.current;
    if (!scrollIntoViewKey || !wrap) return;

    const row = rowSelector(wrap);
    if (!row) return;

    const topLimitElement = topLimitSelector?.(wrap);
    const wrapRect = wrap.getBoundingClientRect();
    const rowRect = row.getBoundingClientRect();
    const topLimit = (topLimitElement?.getBoundingClientRect().bottom ?? wrapRect.top + fallbackTopInset) + 1;
    const bottomLimit = wrapRect.bottom;

    if (rowRect.top < topLimit) {
      wrap.scrollTop -= topLimit - rowRect.top;
    } else if (rowRect.bottom > bottomLimit) {
      wrap.scrollTop += rowRect.bottom - bottomLimit;
    }
  }, [scrollIntoViewKey, wrapRef, rowSelector, topLimitSelector, fallbackTopInset]);

  useLayoutEffect(() => {
    const wrap = wrapRef.current;
    const dock = dockRef.current;
    if (!active || !wrap || !dock) {
      dock?.classList.remove('selectedListDockVisible', 'selectedListDockTop', 'selectedListDockBottom');
      return undefined;
    }

    let currentPlacement = null;

    function setDockPlacement(placement) {
      if (currentPlacement === placement) return;
      currentPlacement = placement;
      dock.classList.toggle('selectedListDockVisible', Boolean(placement));
      dock.classList.toggle('selectedListDockTop', placement === 'top');
      dock.classList.toggle('selectedListDockBottom', placement === 'bottom');
    }

    function updateSelectedDock() {
      dockFrameRef.current = 0;

      const row = rowSelector(wrap);
      const topLimitElement = topLimitSelector?.(wrap);
      if (!row) {
        setDockPlacement(null);
        return;
      }

      const wrapRect = wrap.getBoundingClientRect();
      const rowRect = row.getBoundingClientRect();
      const topLimit = topLimitElement?.getBoundingClientRect().bottom ?? wrapRect.top + fallbackTopInset;
      const bottomLimit = wrapRect.bottom;
      let placement = null;

      if (rowRect.top < topLimit) {
        placement = 'top';
      } else if (rowRect.bottom > bottomLimit) {
        placement = 'bottom';
      }

      if (!placement) {
        setDockPlacement(null);
        return;
      }

      const rowHeight = Math.max(1, Math.round(rowRect.height));
      const x = Math.round(wrapRect.left);
      const y =
        placement === 'top' ? Math.round(topLimit - 1) : Math.round(bottomLimit - rowHeight);

      dock.style.width = `${Math.round(wrap.clientWidth)}px`;
      dock.style.transform = `translate3d(${x}px, ${y}px, 0)`;
      setDockPlacement(placement);
    }

    function scheduleSelectedDockUpdate() {
      if (dockFrameRef.current) return;
      dockFrameRef.current = window.requestAnimationFrame(updateSelectedDock);
    }

    updateSelectedDock();
    wrap.addEventListener('scroll', scheduleSelectedDockUpdate, { passive: true });
    window.addEventListener('resize', scheduleSelectedDockUpdate);

    const resizeObserver = new ResizeObserver(scheduleSelectedDockUpdate);
    resizeObserver.observe(wrap);

    return () => {
      wrap.removeEventListener('scroll', scheduleSelectedDockUpdate);
      window.removeEventListener('resize', scheduleSelectedDockUpdate);
      resizeObserver.disconnect();
      setDockPlacement(null);
      if (dockFrameRef.current) {
        window.cancelAnimationFrame(dockFrameRef.current);
        dockFrameRef.current = 0;
      }
    };
  }, [active, wrapRef, dockRef, rowSelector, topLimitSelector, fallbackTopInset, ...deps]);
}
