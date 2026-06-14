import { Check, Download } from 'lucide-react';

function DependencyIcon({ status }) {
  if (status === 'installed') {
    return <Check size={14} className="catalogInstallDepIconInstalled" />;
  }
  return <Download size={14} className="catalogInstallDepIconPending" />;
}

export function CatalogInstallDependencies({ dependencies }) {
  if (!dependencies?.length) return null;

  return (
    <div className="catalogInstallDeps catalogInstallDepsSingle">
      <ul>
        {dependencies.map((dep) => (
          <li
            key={dep.projectId}
            className={dep.status === 'installed' ? 'catalogInstallDepInstalled' : 'catalogInstallDepPending'}
          >
            <DependencyIcon status={dep.status} />
            <span>{dep.title}</span>
            {dep.status === 'installed' && dep.filename ? <small>{dep.filename}</small> : null}
          </li>
        ))}
      </ul>
    </div>
  );
}
