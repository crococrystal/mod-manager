import { ModCover } from './ModCover.jsx';

export function ModModalHead({ mod, subtitle, title, actions, titleFirst = false }) {
  if (!mod) return null;

  const heading = actions ? (
    <div className="dependencyModalTitleRow">
      <h3 className="dependencyModalTitle">{title ?? mod.displayName}</h3>
      {actions}
    </div>
  ) : (
    <h3 className="dependencyModalTitle">{title ?? mod.displayName}</h3>
  );

  const caption = subtitle ? <p className="dependencyModalSubtitle">{subtitle}</p> : null;

  return (
    <div className="dependencyModalHead">
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
