function Stat({ label, value = 0, tone }) {
  return (
    <div className={`stat ${tone}`}>
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

export function StatsBar({ stats }) {
  return (
    <section className="stats">
      <Stat label="Клиент" value={stats.client} tone="client" />
      <Stat label="Оба" value={stats.universal} tone="universal" />
      <Stat label="Сервер" value={stats.server} tone="server" />
      <Stat label="Сторонние" value={stats.noIndex} tone="manual" />
    </section>
  );
}
