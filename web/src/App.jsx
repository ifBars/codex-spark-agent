import { ArrowSquareOut, GithubLogo } from "@phosphor-icons/react";
import { BenchmarkExplorer } from "./components/BenchmarkExplorer.jsx";

const repositoryUrl = "https://github.com/ifBars/codex-spark-agent";

export function App() {
  return (
    <div className="app-shell">
      <a className="skip-link" href="#explorer">
        Skip to benchmark explorer
      </a>

      <header className="site-header">
        <div className="site-header__inner">
          <a className="wordmark" href="#explorer" aria-label="Spark Bench home">
            Spark <span>Bench</span>
          </a>

          <nav className="site-nav" aria-label="Primary navigation">
            <a href="#explorer" aria-current="page">
              Explorer
            </a>
            <a href="#methodology">Methodology</a>
          </nav>

          <a className="repository-link" href={repositoryUrl} target="_blank" rel="noreferrer">
            <GithubLogo aria-hidden="true" weight="fill" />
            <span>Source</span>
            <ArrowSquareOut aria-hidden="true" />
          </a>
        </div>
      </header>

      <main id="explorer">
        <BenchmarkExplorer />
      </main>
    </div>
  );
}
