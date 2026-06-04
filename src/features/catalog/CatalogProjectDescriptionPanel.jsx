import { LoaderCircle } from 'lucide-react';
import { CatalogProjectDescription } from './CatalogProjectDescription.jsx';

export function CatalogProjectDescriptionPanel({
  children,
  description,
  loading,
  emptyMessage = 'Описание не найдено.',
  className = ''
}) {
  const classes = ['catalogProjectDescriptionPanel', className].filter(Boolean).join(' ');

  return (
    <div className={classes}>
      {children || (description ? (
        <CatalogProjectDescription description={description} />
      ) : (
        <div className="catalogDescriptionPlaceholder">
          {loading ? <LoaderCircle className="spin" size={24} /> : emptyMessage}
        </div>
      ))}
    </div>
  );
}
