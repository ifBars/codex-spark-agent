export function BenchmarkAtlasNav({ views, scenarioCount = 0 }) {
  return (
    <aside className="atlas-nav" aria-label="Benchmark sections">
      <p>Benchmark atlas</p>
      <nav>
        {views.map((view, index) => (
          <a
            href={`#benchmark-${view.id}`}
            key={view.id}
            data-primary={index === 0}
          >
            <i aria-hidden="true" />
            <span>{view.label}</span>
            {view.scenarioCount && <small>{view.scenarioCount}</small>}
          </a>
        ))}
        {scenarioCount > 0 ? (
          <a href="#benchmark-scenarios">
            <i aria-hidden="true" />
            <span>Scenario lens</span>
            <small>{scenarioCount}</small>
          </a>
        ) : null}
      </nav>
      <a className="atlas-nav__method" href="#methodology">
        About this atlas
      </a>
    </aside>
  );
}
