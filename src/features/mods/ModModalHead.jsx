import { ModCover } from './ModCover.jsx';

export function ModModalHead({
  mod,
  subtitle,
  title,
  actions,
  titleFirst = false,
  align = 'center',
  titleAlign = 'center'
}) {
  if (!mod) return null;

  const heading = actions ? (
    <div className={`dependencyModalTitleRow${titleAlign === 'end' ? ' dependencyModalTitleRowBottom' : ''}`}>
      <h3 className="dependencyModalTitle">{title ?? mod.displayName}</h3>
      {actions}
    </div>
  ) : (
    <h3 className="dependencyModalTitle">{title ?? mod.displayName}</h3>
  );

  const caption = subtitle ? <p className="dependencyModalSubtitle">{subtitle}</p> : null;

  return (
    <div className={`dependencyModalHead${align === 'end' ? ' dependencyModalHeadBottom' : ''}`}>
      <ModCover mod={mod} size="tile" />
      <div className="dependencyModalHeadText">
        {titleFirst ? (
          <>
            {heading}
            {caption}
          </>
        ) : (
          <>
            {caption}
            {heading}
          </>
        )}
      </div>
    </div>
  );
}
