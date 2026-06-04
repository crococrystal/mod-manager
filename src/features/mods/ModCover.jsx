import { useEffect, useState } from 'react';
import { Box } from 'lucide-react';

export function ModCover({ mod, size = 'table', onClick, title }) {
  const src = mod?.coverUrl;
  const clickable = Boolean(onClick);
  const isLarge = size === 'hero' || size === 'editor';
  const className =
    size === 'hero'
      ? 'modCover hero clickable'
      : size === 'editor'
        ? 'modCover editor clickable'
        : size === 'tile'
          ? `modCover tile${clickable ? ' clickable' : ''}`
          : 'modCover';
  const iconSize = size === 'hero' ? 40 : size === 'editor' ? 20 : size === 'tile' ? 18 : 16;

  const [failed, setFailed] = useState(false);
  useEffect(() => {
    setFailed(false);
  }, [src]);

  const hasImage = Boolean(src) && !failed;
  const showFallback = !hasImage;

  return (
    <div
      className={className}
      onClick={onClick}
      onKeyDown={clickable ? (event) => event.key === 'Enter' && onClick(event) : undefined}
      role={clickable ? 'button' : undefined}
      tabIndex={clickable ? 0 : undefined}
      title={title}
    >
      {hasImage ? (
        <img
          src={src}
          alt=""
          loading="eager"
          decoding="async"
          onError={() => setFailed(true)}
          fetchPriority={isLarge ? 'high' : undefined}
        />
      ) : null}
      {showFallback ? (
        <span className={isLarge ? 'modCoverPlaceholder' : 'modCoverFallback'} aria-hidden="true">
          <Box size={iconSize} />
        </span>
      ) : null}
    </div>
  );
}
