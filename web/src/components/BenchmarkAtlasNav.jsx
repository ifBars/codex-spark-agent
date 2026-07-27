export function BenchmarkAtlasNav({ views }) {
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
      </nav>
      <a className="atlas-nav__method" href="#methodology">
        About this atlas
      </a>
    </aside>
  );
}
