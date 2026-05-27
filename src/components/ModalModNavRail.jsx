import { ChevronLeft, ChevronRight } from 'lucide-react';

export function ModalModNavRail({ children, modNav, uiLocked = false }) {
  if (!modNav) {
    return children;
  }

  const navDisabled = uiLocked;

  function handleNav(event, action) {
    event.stopPropagation();
    if (navDisabled) return;
    action();
  }

  return (
    <div className="modalModNavRail">
      <button
        type="button"
        className="modalModNavBtn modalModNavBtnPrev"
        disabled={!modNav.canPrev || navDisabled}
        onMouseDown={(event) => handleNav(event, modNav.onPrev)}
        aria-label="Предыдущий мод"
      >
        <ChevronLeft size={34} strokeWidth={1.25} />
      </button>
      <div className="modalModNavRailBody">{children}</div>
      <button
        type="button"
        className="modalModNavBtn modalModNavBtnNext"
        disabled={!modNav.canNext || navDisabled}
        onMouseDown={(event) => handleNav(event, modNav.onNext)}
        aria-label="Следующий мод"
      >
        <ChevronRight size={34} strokeWidth={1.25} />
      </button>
    </div>
  );
}
