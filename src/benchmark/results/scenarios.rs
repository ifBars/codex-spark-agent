pub(super) fn scenario_family(scenario: &str) -> &'static str {
    match scenario {
        "repo-survey"
        | "repo-architecture-survey"
        | "benchmark-design-survey"
        | "steamnetworklib-survey"
        | "s1api-survey" => "Survey",
        "tool-recovery" | "shell-recovery" => "Terminal and tool recovery",
        "file-edit"
        | "precise-patch"
        | "github-issue-bugfix"
        | "rust-failing-test-bugfix"
        | "typescript-reducer-bugfix"
        | "multi-module-bugfix" => "Precise edit",
        "file-ops" | "multi-file-patch" | "config-migration" => "Multi-file coordination",
        "github-issue-triage" => "Issue triage",
        "technical-essay" => "Long-form writing",
        "ops-report" | "multi-hop-analysis" => "Data analysis",
        "terminal-repair" => "Terminal and tool recovery",
        "policy-support-agent" => "Policy compliance",
        "react-calculator-scaffold" | "rust-log-analyzer-scaffold" | "rust-notes-tui-scaffold" => {
            "Project scaffold"
        }
        "natural-compaction" | "compaction-pressure" => "Context pressure",
        _ => "General",
    }
}

pub(super) fn scenario_question(scenario: &str) -> &'static str {
    match scenario {
        "repo-survey" => "Can it inspect a repo and answer with grounded evidence?",
        "repo-architecture-survey" => "Can it explain architecture without wandering?",
        "benchmark-design-survey" => {
            "Can it inspect benchmark taxonomy and propose realistic gaps?"
        }
        "steamnetworklib-survey" | "s1api-survey" => {
            "Can it explore a broader external-style code surface?"
        }
        "tool-recovery" => "Can it recover from a failed native tool path?",
        "shell-recovery" => {
            "Can it run shell commands, inspect errors, recover, and verify output?"
        }
        "file-edit" => "Can it make a scoped edit and verify the changed file?",
        "precise-patch" => "Can it patch one branch without over-editing nearby logic?",
        "github-issue-bugfix" => "Can it solve a GitHub-style issue with a scoped tested fix?",
        "rust-failing-test-bugfix" => {
            "Can it fix a Rust bug with failing tests and Cargo validation?"
        }
        "typescript-reducer-bugfix" => {
            "Can it fix a TypeScript reducer bug with failing tests and Bun validation?"
        }
        "github-issue-triage" => "Can it investigate an issue and write a grounded triage note?",
        "file-ops" => "Can it create, rename, search, and verify files?",
        "multi-file-patch" => "Can it update code and docs consistently across files?",
        "config-migration" => "Can it migrate config shape across JSON, code, and docs?",
        "technical-essay" => "Can it write a sourced essay from local evidence?",
        "ops-report" => "Can it compute metrics and write an operational readout?",
        "multi-module-bugfix" => {
            "Can it fix a cross-module bug with failing tests and Bun validation?"
        }
        "terminal-repair" => {
            "Can it diagnose a broken service through the terminal and repair its config?"
        }
        "multi-hop-analysis" => "Can it join policy and data files into one exact grounded answer?",
        "policy-support-agent" => "Can it apply a multi-rule policy consistently across turns?",
        "react-calculator-scaffold" => "Can it build and browser-verify a React TypeScript app?",
        "rust-log-analyzer-scaffold" => "Can it scaffold and validate a small Rust CLI project?",
        "rust-notes-tui-scaffold" => "Can it scaffold and validate a vim-style Rust notes CLI?",
        "natural-compaction" | "compaction-pressure" => {
            "Can it keep useful context under pressure?"
        }
        _ => "Can it complete the requested real-world task?",
    }
}
