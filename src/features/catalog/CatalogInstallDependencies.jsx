import { Check, Download } from 'lucide-react';

function dependencyGroups(dependencies) {
  const installed = [];
  const pending = [];
  for (const item of dependencies ?? []) {
    if (item.status === 'installed') installed.push(item);
    else pending.push(item);
  }
  return { installed, pending };
}

function DependencyList({ items, icon: Icon, showFilename }) {
  return (
    <ul>
      {items.map((dep) => (
        <li key={dep.projectId}>
          <Icon size={14} />
          <span>{dep.title}</span>
          {showFilename && dep.filename ? <small>{dep.filename}</small> : null}
        </li>
      ))}
    </ul>
  );
}

export function splitCatalogDependencies(dependencies) {
  return dependencyGroups(dependencies);
}

export function CatalogInstallDependencies({ installed, pending }) {
  if (!installed.length && !pending.length) return null;

  return (
    <div
      className={`catalogInstallDeps${installed.length && pending.length ? '' : ' catalogInstallDepsSingle'}`}
    >
      {installed.length ? (
        <section className="catalogInstallDepsInstalled">
          <DependencyList items={installed} icon={Check} showFilename />
        </section>
      ) : null}
      {pending.length ? (
        <section>
          <h4>Скачаем вместе с модом</h4>
          <DependencyList items={pending} icon={Download} />
        </section>
      ) : null}
    </div>
  );
}
