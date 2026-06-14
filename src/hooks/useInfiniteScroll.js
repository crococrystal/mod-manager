import { useEffect, useRef } from 'react';

function isSentinelNearViewport(root, sentinel, marginPx = 240) {
  if (!root || !sentinel) return false;
  const rootRect = root.getBoundingClientRect();
  const sentinelRect = sentinel.getBoundingClientRect();
  return sentinelRect.top <= rootRect.bottom + marginPx;
}

export function useInfiniteScroll({
  enabled,
  rootRef,
  hasMore,
  loading,
  loadingMore,
  onLoadMore,
  watchKey = 0
}) {
  const sentinelRef = useRef(null);
  const onLoadMoreRef = useRef(onLoadMore);
  onLoadMoreRef.current = onLoadMore;

  useEffect(() => {
    if (!enabled || !hasMore) return undefined;
    const root = rootRef.current;
    const sentinel = sentinelRef.current;
    if (!root || !sentinel) return undefined;

    const observer = new IntersectionObserver(
      (entries) => {
        if (!entries.some((entry) => entry.isIntersecting)) return;
        onLoadMoreRef.current?.();
      },
      { root, rootMargin: '240px 0px', threshold: 0 }
    );

    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [enabled, hasMore, loading, loadingMore, rootRef, watchKey]);

  useEffect(() => {
    if (!enabled || !hasMore || loading || loadingMore) return undefined;
    const root = rootRef.current;
    const sentinel = sentinelRef.current;
    if (!root || !sentinel) return undefined;

    const id = requestAnimationFrame(() => {
      if (isSentinelNearViewport(root, sentinel)) {
        onLoadMoreRef.current?.();
      }
    });
    return () => cancelAnimationFrame(id);
  }, [enabled, hasMore, loading, loadingMore, rootRef, watchKey]);

  return sentinelRef;
}
