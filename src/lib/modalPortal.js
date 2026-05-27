/** Контейнер внутри .windowFrame — модалки не перекрывают скругление окна. */
export function getModalPortalRoot() {
  return document.getElementById('app-modal-root') ?? document.body;
}
