use crate::cli::ProfileScenarioKind;

use super::{ProfileScenarioValidationCheck, ProfileScenarioValidationCommand};

pub(crate) fn profile_scenario_validation_checks(
    scenario: ProfileScenarioKind,
) -> &'static [ProfileScenarioValidationCheck] {
    match scenario {
        ProfileScenarioKind::StatefulReconciliationBugfix => &[
            ProfileScenarioValidationCheck {
                name: "latest duplicate by timestamp",
                weight: 20,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$env:SPARK_VALIDATION_CHECK='duplicate-timezone'; bun test ./tests/.harness/projection.validation.ts; exit $LASTEXITCODE",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "chronological deterministic ordering",
                weight: 20,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$env:SPARK_VALIDATION_CHECK='event-order'; bun test ./tests/.harness/projection.validation.ts; exit $LASTEXITCODE",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "bounded terminal shipment",
                weight: 25,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$env:SPARK_VALIDATION_CHECK='terminal-shipment'; bun test ./tests/.harness/projection.validation.ts; exit $LASTEXITCODE",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "empty shipment remains open",
                weight: 20,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$env:SPARK_VALIDATION_CHECK='empty-shipment'; bun test ./tests/.harness/projection.validation.ts; exit $LASTEXITCODE",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "invalid quantities create no state",
                weight: 15,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$env:SPARK_VALIDATION_CHECK='invalid-quantity'; bun test ./tests/.harness/projection.validation.ts; exit $LASTEXITCODE",
                ],
            },
        ],
        ProfileScenarioKind::FeatureRolloutConsistencyBugfix => &[
            ProfileScenarioValidationCheck {
                name: "tenant-isolated config storage",
                weight: 20,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$env:SPARK_VALIDATION_CHECK='tenant-store'; bun test ./tests/.harness/rollout.validation.ts; exit $LASTEXITCODE",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "monotonic config revisions",
                weight: 15,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$env:SPARK_VALIDATION_CHECK='monotonic-revision'; bun test ./tests/.harness/rollout.validation.ts; exit $LASTEXITCODE",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "decision precedence",
                weight: 20,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$env:SPARK_VALIDATION_CHECK='decision-precedence'; bun test ./tests/.harness/rollout.validation.ts; exit $LASTEXITCODE",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "stable bounded rollout",
                weight: 15,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$env:SPARK_VALIDATION_CHECK='stable-rollout'; bun test ./tests/.harness/rollout.validation.ts; exit $LASTEXITCODE",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "tenant and revision cache isolation",
                weight: 20,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$env:SPARK_VALIDATION_CHECK='cache-isolation'; bun test ./tests/.harness/rollout.validation.ts; exit $LASTEXITCODE",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "end-to-end revision behavior",
                weight: 10,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$env:SPARK_VALIDATION_CHECK='service-revision'; bun test ./tests/.harness/rollout.validation.ts; exit $LASTEXITCODE",
                ],
            },
        ],
        ProfileScenarioKind::FrontierRuleTransfer => &[
            ProfileScenarioValidationCheck {
                name: "amber distractor transfer",
                weight: 17,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$env:SPARK_VALIDATION_CHECK='amber-distractors'; bun test ./tests/.harness/frontier.validation.ts; exit $LASTEXITCODE",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "cyan tie-break transfer",
                weight: 17,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$env:SPARK_VALIDATION_CHECK='cyan-tie-break'; bun test ./tests/.harness/frontier.validation.ts; exit $LASTEXITCODE",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "cycle avoidance",
                weight: 17,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$env:SPARK_VALIDATION_CHECK='cycle-avoidance'; bun test ./tests/.harness/frontier.validation.ts; exit $LASTEXITCODE",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "early termination",
                weight: 17,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$env:SPARK_VALIDATION_CHECK='early-stop'; bun test ./tests/.harness/frontier.validation.ts; exit $LASTEXITCODE",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "edge-weighted choice",
                weight: 16,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$env:SPARK_VALIDATION_CHECK='weighted-choice'; bun test ./tests/.harness/frontier.validation.ts; exit $LASTEXITCODE",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "invalid target resilience",
                weight: 16,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$env:SPARK_VALIDATION_CHECK='unknown-target'; bun test ./tests/.harness/frontier.validation.ts; exit $LASTEXITCODE",
                ],
            },
        ],
        ProfileScenarioKind::InventoryRebalancePlan => &[
            ProfileScenarioValidationCheck {
                name: "exact output schema",
                weight: 10,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $plan = Get-Content -LiteralPath 'plan.json' -Raw | ConvertFrom-Json; $top = @($plan.psobject.Properties.Name | Sort-Object); if (($top -join ',') -ne 'basePlan,contingencyPlan,incrementalNetBenefit') { throw 'top-level schema mismatch' }; $expected = 'budget,grossAvoidedPenalty,netBenefit,remainingBudget,selectedOptionIds,totalCost,totalUnits'; foreach ($name in @('basePlan','contingencyPlan')) { $keys = @($plan.$name.psobject.Properties.Name | Sort-Object); if (($keys -join ',') -ne $expected) { throw \"$name schema mismatch\" }; $ids = @($plan.$name.selectedOptionIds); if (($ids | Sort-Object) -join ',' -ne ($ids -join ',')) { throw \"$name option ids must be sorted\" }; if (@($ids | Select-Object -Unique).Count -ne $ids.Count) { throw \"$name option ids must be unique\" } }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "base plan optimal selection",
                weight: 25,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $p = (Get-Content -LiteralPath 'plan.json' -Raw | ConvertFrom-Json).basePlan; if ((@($p.selectedOptionIds) -join ',') -ne 'T05,T07,T08,T11,T12') { throw 'base selection is not optimal' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "contingency plan optimal selection",
                weight: 20,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $p = (Get-Content -LiteralPath 'plan.json' -Raw | ConvertFrom-Json).contingencyPlan; if ((@($p.selectedOptionIds) -join ',') -ne 'T02,T03,T11,T12') { throw 'contingency selection is not optimal' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "base plan computed metrics",
                weight: 20,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $p = (Get-Content -LiteralPath 'plan.json' -Raw | ConvertFrom-Json).basePlan; foreach ($pair in @(@('budget',325),@('totalUnits',72),@('totalCost',307),@('grossAvoidedPenalty',2950),@('netBenefit',2643),@('remainingBudget',18))) { if ([decimal]$p.($pair[0]) -ne [decimal]$pair[1]) { throw \"base $($pair[0]) mismatch\" } }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "contingency plan computed metrics",
                weight: 15,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $plan = Get-Content -LiteralPath 'plan.json' -Raw | ConvertFrom-Json; $p = $plan.contingencyPlan; foreach ($pair in @(@('budget',250),@('totalUnits',52),@('totalCost',247),@('grossAvoidedPenalty',2470),@('netBenefit',2223),@('remainingBudget',3))) { if ([decimal]$p.($pair[0]) -ne [decimal]$pair[1]) { throw \"contingency $($pair[0]) mismatch\" } }; if ([decimal]$plan.incrementalNetBenefit -ne 420) { throw 'incrementalNetBenefit mismatch' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "decision memo grounded in constraints",
                weight: 10,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $memo = Get-Content -LiteralPath 'memo.md' -Raw; foreach ($term in @('base','contingency','T14','lead','420')) { if ($memo -notmatch [regex]::Escape($term)) { throw \"memo missing $term\" } }; if ($memo -notmatch '(?i)(surplus|origin)' -or $memo -notmatch '(?i)(deficit|destination)' -or $memo -notmatch '(?i)budget') { throw 'memo missing binding constraint explanation' }",
                ],
            },
        ],
        ProfileScenarioKind::ExperimentRolloutAudit => &[
            ProfileScenarioValidationCheck {
                name: "exact audit schema",
                weight: 10,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $a = Get-Content -LiteralPath 'audit.json' -Raw | ConvertFrom-Json; if ((@($a.psobject.Properties.Name | Sort-Object) -join ',') -ne 'control,dataQuality,decision,treatment,uplifts') { throw 'top-level schema mismatch' }; $variant = 'conversionRatePct,converters,eligibleUsers,grossRevenueCents,netRevenueCents,netRevenuePerEligibleCents,orders,refundCents,refundedOrders,refundRatePct'; foreach ($name in @('control','treatment')) { if ((@($a.$name.psobject.Properties.Name | Sort-Object) -join ',') -ne $variant) { throw \"$name schema mismatch\" } }; if ((@($a.dataQuality.psobject.Properties.Name | Sort-Object) -join ',') -ne 'assignmentRows,conflictedUsers,duplicateAssignmentRows,duplicateEventRows,duplicateOrderEvents,eventRows,excludedUsers,orphanEvents,outOfWindowCheckouts') { throw 'dataQuality schema mismatch' }; if ((@($a.uplifts.psobject.Properties.Name | Sort-Object) -join ',') -ne 'conversionUpliftPercentagePoints,netRevenuePerEligibleUpliftPct,refundRateDeltaPercentagePoints,relativeConversionUpliftPct') { throw 'uplifts schema mismatch' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "assignment and event data quality",
                weight: 20,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $d = (Get-Content -LiteralPath 'audit.json' -Raw | ConvertFrom-Json).dataQuality; foreach ($pair in @(@('assignmentRows',25),@('duplicateAssignmentRows',1),@('conflictedUsers',1),@('excludedUsers',2),@('eventRows',27),@('duplicateEventRows',1),@('orphanEvents',4),@('outOfWindowCheckouts',2),@('duplicateOrderEvents',1))) { if ([decimal]$d.($pair[0]) -ne [decimal]$pair[1]) { throw \"dataQuality $($pair[0]) mismatch\" } }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "control attribution metrics",
                weight: 15,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $p = (Get-Content -LiteralPath 'audit.json' -Raw | ConvertFrom-Json).control; foreach ($pair in @(@('eligibleUsers',10),@('converters',5),@('conversionRatePct',50),@('orders',5),@('grossRevenueCents',47000),@('refundedOrders',1),@('refundRatePct',20),@('refundCents',8000),@('netRevenueCents',39000),@('netRevenuePerEligibleCents',3900))) { if ([decimal]$p.($pair[0]) -ne [decimal]$pair[1]) { throw \"control $($pair[0]) mismatch\" } }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "treatment attribution metrics",
                weight: 20,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $p = (Get-Content -LiteralPath 'audit.json' -Raw | ConvertFrom-Json).treatment; foreach ($pair in @(@('eligibleUsers',10),@('converters',7),@('conversionRatePct',70),@('orders',8),@('grossRevenueCents',65000),@('refundedOrders',2),@('refundRatePct',25),@('refundCents',24000),@('netRevenueCents',41000),@('netRevenuePerEligibleCents',4100))) { if ([decimal]$p.($pair[0]) -ne [decimal]$pair[1]) { throw \"treatment $($pair[0]) mismatch\" } }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "uplift and guardrail calculations",
                weight: 20,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $u = (Get-Content -LiteralPath 'audit.json' -Raw | ConvertFrom-Json).uplifts; foreach ($pair in @(@('conversionUpliftPercentagePoints',20),@('relativeConversionUpliftPct',40),@('netRevenuePerEligibleUpliftPct',5.13),@('refundRateDeltaPercentagePoints',5))) { if ([math]::Abs([decimal]$u.($pair[0]) - [decimal]$pair[1]) -gt 0.001) { throw \"uplifts $($pair[0]) mismatch\" } }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "decision memo evaluates launch gates",
                weight: 15,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $a = Get-Content -LiteralPath 'audit.json' -Raw | ConvertFrom-Json; if ($a.decision -ne 'hold') { throw 'decision must be hold' }; $memo = Get-Content -LiteralPath 'memo.md' -Raw; foreach ($term in @('hold','refund','5','3','conversion','revenue','duplicate','orphan')) { if ($memo -notmatch [regex]::Escape($term)) { throw \"memo missing $term\" } }",
                ],
            },
        ],
        _ => &[],
    }
}

pub(crate) fn profile_scenario_validation_command(
    scenario: ProfileScenarioKind,
) -> Option<ProfileScenarioValidationCommand> {
    match scenario {
        ProfileScenarioKind::ReactCalculatorScaffold => Some(ProfileScenarioValidationCommand {
            workdir: ".",
            program: "bun",
            args: &["test"],
        }),
        ProfileScenarioKind::RustLogAnalyzerScaffold => Some(ProfileScenarioValidationCommand {
            workdir: ".",
            program: "powershell",
            args: &[
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='Stop'; cargo test; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }; $out = cargo run --quiet -- .\\sample.log; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }; $text = $out -join \"`n\"; foreach ($term in @('INFO','WARN','ERROR','Top error code','E42')) { if ($text -notlike \"*$term*\") { throw \"missing $term\" } }",
            ],
        }),
        ProfileScenarioKind::RustNotesTuiScaffold => Some(ProfileScenarioValidationCommand {
            workdir: ".",
            program: "powershell",
            args: &["-NoProfile", "-File", "validate-notes.ps1"],
        }),
        ProfileScenarioKind::GithubIssueBugfix => Some(ProfileScenarioValidationCommand {
            workdir: ".",
            program: "bun",
            args: &["test"],
        }),
        ProfileScenarioKind::RustFailingTestBugfix => Some(ProfileScenarioValidationCommand {
            workdir: ".",
            program: "cargo",
            args: &["test"],
        }),
        ProfileScenarioKind::TypeScriptReducerBugfix => Some(ProfileScenarioValidationCommand {
            workdir: ".",
            program: "bun",
            args: &["test"],
        }),
        ProfileScenarioKind::MergeConflictResolution => Some(ProfileScenarioValidationCommand {
            workdir: ".",
            program: "powershell",
            args: &[
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='Stop'; $source = Get-Content -LiteralPath 'src/featureFlags.ts' -Raw; foreach ($marker in @('<<<<<<<','=======','>>>>>>>')) { if ($source -like \"*$marker*\") { throw \"unresolved conflict marker $marker\" } }; foreach ($term in @('dashboard-v2','data-residency','startsWith(''beta-'')','region === ''eu''')) { if ($source -notlike \"*$term*\") { throw \"missing $term\" } }; bun test",
            ],
        }),
        ProfileScenarioKind::GithubIssueTriage => Some(ProfileScenarioValidationCommand {
            workdir: ".",
            program: "powershell",
            args: &[
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='Stop'; $content = Get-Content -LiteralPath 'triage.md' -Raw; foreach ($term in @('/api/items','Cache-Control','max-age=300','stale-while-revalidate=30','src/cachePolicy.ts')) { if ($content -notlike \"*$term*\") { throw \"missing $term\" } }",
            ],
        }),
        ProfileScenarioKind::CiFailureTriage => Some(ProfileScenarioValidationCommand {
            workdir: ".",
            program: "powershell",
            args: &[
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='Stop'; $content = Get-Content -LiteralPath 'ci-triage.md' -Raw; foreach ($term in @('bun test','SAVE20','applyDiscount','src/discount.ts','tests/discount.test.ts')) { if ($content -notlike \"*$term*\") { throw \"missing $term\" } }; if ($content -notmatch '(?i)\\bExpected\\b[^\\r\\n]*\\b80\\b') { throw 'missing expected 80 assertion evidence' }; if ($content -notmatch '(?i)\\bReceived\\b[^\\r\\n]*\\b100\\b') { throw 'missing received 100 assertion evidence' }",
            ],
        }),
        ProfileScenarioKind::PullRequestReview => Some(ProfileScenarioValidationCommand {
            workdir: ".",
            program: "powershell",
            args: &[
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='Stop'; $content = Get-Content -LiteralPath 'review.md' -Raw; foreach ($term in @('read-only-admin','discountFor','src/checkout.ts','tests/checkout.test.ts')) { if ($content -notlike \"*$term*\") { throw \"missing $term\" } }; if ($content -notmatch \"includes\\s*\\(\\s*[''`\"]admin[''`\"]\\s*\\)\") { throw 'missing includes admin evidence' }; if ($content -notmatch '(?i)(blocking|must fix|p1|p0)') { throw 'missing blocking severity' }; if ($content -notmatch '(?i)(exactly\\s+admin|role\\s+exactly\\s+admin|===\\s*[''`\"]admin[''`\"]|==\\s*[''`\"]admin[''`\"]|strict equality)') { throw 'missing exact admin fix recommendation' }",
            ],
        }),
        ProfileScenarioKind::DependencyUpgradeTriage => Some(ProfileScenarioValidationCommand {
            workdir: ".",
            program: "powershell",
            args: &[
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='Stop'; $content = Get-Content -LiteralPath 'upgrade-triage.md' -Raw; foreach ($term in @('@acme/time-utils','2.0.0','parseBusinessDate','src/billingWindow.ts','tests/billingWindow.test.ts')) { if ($content -notlike \"*$term*\") { throw \"missing $term\" } }; if ($content -notmatch '(?i)\\bUTC\\b') { throw 'missing UTC risk' }; if ($content -notmatch '(?i)\\blocal\\b') { throw 'missing local timezone change' }; if ($content -notmatch \"zone\\s*:\\s*[''`\"]utc[''`\"]\") { throw 'missing zone utc fix' }; if ($content -notmatch '(?i)(test gap|missing test|add.*test|regression test)') { throw 'missing test gap recommendation' }",
            ],
        }),
        ProfileScenarioKind::TechnicalEssay => Some(ProfileScenarioValidationCommand {
            workdir: ".",
            program: "powershell",
            args: &[
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='Stop'; $content = Get-Content -LiteralPath 'essay.md' -Raw; foreach ($term in @('Operational Visibility Is a Product Feature','[S1]','[S2]','[S3]')) { if ($content -notlike \"*$term*\") { throw \"missing $term\" } }; $words = @($content -split '\\s+' | Where-Object { $_ }); if ($words.Count -lt 350) { throw \"essay too short: $($words.Count) words\" }; $headings = @($content -split \"`r?`n\" | Where-Object { $_ -like '## *' }); if ($headings.Count -lt 2) { throw 'missing section headings' }",
            ],
        }),
        ProfileScenarioKind::ConfigMigration => Some(ProfileScenarioValidationCommand {
            workdir: ".",
            program: "powershell",
            args: &[
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='Stop'; $json = Get-Content -LiteralPath 'config/app.json' -Raw | ConvertFrom-Json; if ($json.schemaVersion -ne 2) { throw 'schemaVersion not 2' }; if ($json.authentication.method -ne 'password') { throw 'authentication.method not preserved' }; if ($json.retry.maxAttempts -ne 3) { throw 'retry.maxAttempts not migrated' }; if ($json.retry.backoffMs -ne 250) { throw 'retry.backoffMs not preserved' }; $all = (Get-Content -LiteralPath 'src/config.ts' -Raw) + (Get-Content -LiteralPath 'docs/config.md' -Raw) + (Get-Content -LiteralPath 'config/app.json' -Raw); foreach ($term in @('authMode','retries: number','retry.retries')) { if ($all -like \"*$term*\") { throw \"stale term $term\" } }; foreach ($term in @('authentication','method','maxAttempts','schemaVersion: 2')) { if ($all -notlike \"*$term*\") { throw \"missing $term\" } }",
            ],
        }),
        ProfileScenarioKind::OpsReport => Some(ProfileScenarioValidationCommand {
            workdir: ".",
            program: "powershell",
            args: &[
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='Stop'; $metrics = Get-Content -LiteralPath 'metrics.json' -Raw | ConvertFrom-Json; if ($metrics.totalTickets -ne 8) { throw 'totalTickets must be 8' }; if ($metrics.openTickets -ne 5) { throw 'openTickets must be 5' }; if ($metrics.p1Open -ne 2) { throw 'p1Open must be 2' }; if ([math]::Abs([double]$metrics.averageOpenMinutes - 51.4) -gt 0.01) { throw 'averageOpenMinutes must be 51.4' }; $report = Get-Content -LiteralPath 'report.md' -Raw; $plain = (($report -replace '[*`#_]', '') -replace '\\s+', ' ').Trim(); if ($plain -notmatch '(?i)(highest-risk team\\s*(?:-|:)?\\s*team\\s*:\\s*billing|highest-risk team\\s*(:|-|is)?\\s*billing|billing\\s+(is\\s+)?(the\\s+)?highest-risk team|billing\\s+team\\s+is\\s+highest\\s+risk)') { throw 'report must identify billing as highest-risk team' }; if ($plain -match '(?i)(highest-risk team\\s*(?:-|:)?\\s*team\\s*:\\s*api|highest-risk team\\s*(:|-|is)?\\s*api|api\\s+(is\\s+)?(the\\s+)?highest-risk team|api\\s+team\\s+is\\s+highest\\s+risk)') { throw 'report incorrectly identifies api as highest-risk team' }; if ($plain -notmatch '95') { throw 'report must explain billing risk with the 95 minute open P1 age' }",
            ],
        }),
        ProfileScenarioKind::InventoryRebalancePlan => Some(ProfileScenarioValidationCommand {
            workdir: ".",
            program: "powershell",
            args: &[
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='Stop'; $plan = Get-Content -LiteralPath 'plan.json' -Raw | ConvertFrom-Json; $top = @($plan.psobject.Properties.Name | Sort-Object); if (($top -join ',') -ne 'basePlan,contingencyPlan,incrementalNetBenefit') { throw 'top-level schema mismatch' }; $expected = 'budget,grossAvoidedPenalty,netBenefit,remainingBudget,selectedOptionIds,totalCost,totalUnits'; function Check-Plan($p,$name,$budget,$ids,$units,$cost,$gross,$net,$remaining) { $keys = @($p.psobject.Properties.Name | Sort-Object); if (($keys -join ',') -ne $expected) { throw \"$name schema mismatch\" }; if ((@($p.selectedOptionIds) -join ',') -ne $ids) { throw \"$name selection mismatch\" }; foreach ($pair in @(@('budget',$budget),@('totalUnits',$units),@('totalCost',$cost),@('grossAvoidedPenalty',$gross),@('netBenefit',$net),@('remainingBudget',$remaining))) { if ([decimal]$p.($pair[0]) -ne [decimal]$pair[1]) { throw \"$name $($pair[0]) mismatch\" } } }; Check-Plan $plan.basePlan 'base' 325 'T05,T07,T08,T11,T12' 72 307 2950 2643 18; Check-Plan $plan.contingencyPlan 'contingency' 250 'T02,T03,T11,T12' 52 247 2470 2223 3; if ([decimal]$plan.incrementalNetBenefit -ne 420) { throw 'incrementalNetBenefit mismatch' }; $memo = Get-Content -LiteralPath 'memo.md' -Raw; foreach ($term in @('base','contingency','T14','lead','420')) { if ($memo -notmatch [regex]::Escape($term)) { throw \"memo missing $term\" } }; if ($memo -notmatch '(?i)(surplus|origin)' -or $memo -notmatch '(?i)(deficit|destination)' -or $memo -notmatch '(?i)budget') { throw 'memo missing constraint explanation' }",
            ],
        }),
        ProfileScenarioKind::ExperimentRolloutAudit => Some(ProfileScenarioValidationCommand {
            workdir: ".",
            program: "powershell",
            args: &[
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='Stop'; $a = Get-Content -LiteralPath 'audit.json' -Raw | ConvertFrom-Json; if ((@($a.psobject.Properties.Name | Sort-Object) -join ',') -ne 'control,dataQuality,decision,treatment,uplifts') { throw 'top-level schema mismatch' }; $variant = 'conversionRatePct,converters,eligibleUsers,grossRevenueCents,netRevenueCents,netRevenuePerEligibleCents,orders,refundCents,refundedOrders,refundRatePct'; foreach ($name in @('control','treatment')) { if ((@($a.$name.psobject.Properties.Name | Sort-Object) -join ',') -ne $variant) { throw \"$name schema mismatch\" } }; if ((@($a.dataQuality.psobject.Properties.Name | Sort-Object) -join ',') -ne 'assignmentRows,conflictedUsers,duplicateAssignmentRows,duplicateEventRows,duplicateOrderEvents,eventRows,excludedUsers,orphanEvents,outOfWindowCheckouts') { throw 'dataQuality schema mismatch' }; if ((@($a.uplifts.psobject.Properties.Name | Sort-Object) -join ',') -ne 'conversionUpliftPercentagePoints,netRevenuePerEligibleUpliftPct,refundRateDeltaPercentagePoints,relativeConversionUpliftPct') { throw 'uplifts schema mismatch' }; foreach ($pair in @(@('assignmentRows',25),@('duplicateAssignmentRows',1),@('conflictedUsers',1),@('excludedUsers',2),@('eventRows',27),@('duplicateEventRows',1),@('orphanEvents',4),@('outOfWindowCheckouts',2),@('duplicateOrderEvents',1))) { if ([decimal]$a.dataQuality.($pair[0]) -ne [decimal]$pair[1]) { throw \"dataQuality $($pair[0]) mismatch\" } }; foreach ($pair in @(@('eligibleUsers',10),@('converters',5),@('conversionRatePct',50),@('orders',5),@('grossRevenueCents',47000),@('refundedOrders',1),@('refundRatePct',20),@('refundCents',8000),@('netRevenueCents',39000),@('netRevenuePerEligibleCents',3900))) { if ([decimal]$a.control.($pair[0]) -ne [decimal]$pair[1]) { throw \"control $($pair[0]) mismatch\" } }; foreach ($pair in @(@('eligibleUsers',10),@('converters',7),@('conversionRatePct',70),@('orders',8),@('grossRevenueCents',65000),@('refundedOrders',2),@('refundRatePct',25),@('refundCents',24000),@('netRevenueCents',41000),@('netRevenuePerEligibleCents',4100))) { if ([decimal]$a.treatment.($pair[0]) -ne [decimal]$pair[1]) { throw \"treatment $($pair[0]) mismatch\" } }; foreach ($pair in @(@('conversionUpliftPercentagePoints',20),@('relativeConversionUpliftPct',40),@('netRevenuePerEligibleUpliftPct',5.13),@('refundRateDeltaPercentagePoints',5))) { if ([math]::Abs([decimal]$a.uplifts.($pair[0]) - [decimal]$pair[1]) -gt 0.001) { throw \"uplifts $($pair[0]) mismatch\" } }; if ($a.decision -ne 'hold') { throw 'decision must be hold' }; $memo = Get-Content -LiteralPath 'memo.md' -Raw; foreach ($term in @('hold','refund','5','3','conversion','revenue','duplicate','orphan')) { if ($memo -notmatch [regex]::Escape($term)) { throw \"memo missing $term\" } }",
            ],
        }),
        ProfileScenarioKind::ShellRecovery => Some(ProfileScenarioValidationCommand {
            workdir: ".",
            program: "powershell",
            args: &[
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='Stop'; $summary = Get-Content -LiteralPath 'summary.txt' -Raw; if ($summary -notmatch 'total=5') { throw 'missing total=5' }; if ($summary -notmatch 'failed=2') { throw 'missing failed=2' }; if ($summary -notmatch 'top_service=payments') { throw 'missing top_service=payments' }",
            ],
        }),
        ProfileScenarioKind::PrecisePatch => Some(ProfileScenarioValidationCommand {
            workdir: ".",
            program: "powershell",
            args: &[
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='Stop'; $content = Get-Content -LiteralPath 'src/status_map.ts' -Raw; if ($content -notmatch \"case 'queued':[\\s\\S]*return 'Queued';\") { throw 'queued branch was not patched' }; if ($content -notmatch \"default:[\\s\\S]*return 'Unknown';\") { throw 'default branch changed' }; if (($content | Select-String \"return 'Queued';\" -AllMatches).Matches.Count -ne 1) { throw 'queued label was over-applied' }",
            ],
        }),
        ProfileScenarioKind::MultiFilePatch => Some(ProfileScenarioValidationCommand {
            workdir: ".",
            program: "powershell",
            args: &[
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='Stop'; $routes = Get-Content -LiteralPath 'src/routes.ts' -Raw; $nav = Get-Content -LiteralPath 'src/navigation.ts' -Raw; $docs = Get-Content -LiteralPath 'docs/routes.md' -Raw; if ($routes -notmatch \"id: 'reports'\" -or $routes -notmatch \"path: '/reports'\") { throw 'routes.ts missing reports route' }; if ($nav -notmatch \"label: 'Reports'\" -or $nav -notmatch \"routeId: 'reports'\") { throw 'navigation.ts missing Reports item' }; if ($docs -notmatch '/reports') { throw 'docs missing /reports' }",
            ],
        }),
        ProfileScenarioKind::ManifestContractWrite => Some(ProfileScenarioValidationCommand {
            workdir: ".",
            program: "powershell",
            args: &[
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='Stop'; $manifest = Get-Content -LiteralPath 'generated/release-manifest.json' -Raw | ConvertFrom-Json; $keys = @($manifest.PSObject.Properties.Name | Sort-Object); if (($keys -join ',') -ne 'artifacts,channel,checksum,currentVersion,previousVersion') { throw 'manifest keys do not match contract' }; if ($manifest.channel -ne 'stable' -or $manifest.currentVersion -ne '1.4.0' -or $manifest.previousVersion -ne '1.3.9' -or $manifest.checksum -ne '9c2f8a1d') { throw 'manifest values do not match approved release' }; if ((@($manifest.artifacts) -join ',') -ne 'spark-1.4.0-windows-x64.zip,spark-1.4.0-checksums.txt') { throw 'manifest artifacts do not preserve approved order' }; $notes = Get-Content -LiteralPath 'generated/release-notes.md' -Raw; foreach ($text in @('# Spark 1.4.0','- spark-1.4.0-windows-x64.zip','- spark-1.4.0-checksums.txt','SHA-256: 9c2f8a1d')) { if (-not $notes.Contains($text)) { throw \"notes missing $text\" } }; if ($notes.Contains('1.4.1-rc.1') -or $notes.Contains('deadbeef')) { throw 'notes mention rejected release' }",
            ],
        }),
        ProfileScenarioKind::ScopedPolicyPatch => Some(ProfileScenarioValidationCommand {
            workdir: ".",
            program: "powershell",
            args: &[
                "-NoProfile",
                "-Command",
                r"$ErrorActionPreference='Stop'; $content = Get-Content -LiteralPath 'src/rate_limit.ts' -Raw; if ($content -notmatch 'canRetryPayment[\s\S]*if \(!account\.active\) return false;[\s\S]*if \(account\.retriesToday >= 3\) return false;') { throw 'canRetryPayment does not reject inactive accounts first' }; if ($content -notmatch 'isRetryLimitExceeded[\s\S]*if \(!account\.active\) return true;[\s\S]*return account\.retriesToday >= 3;') { throw 'isRetryLimitExceeded was changed' }; if (($content | Select-String 'if \(!account\.active\)' -AllMatches).Matches.Count -ne 2) { throw 'inactive-account policy was over-edited' }",
            ],
        }),
        ProfileScenarioKind::MultiModuleBugfix => Some(ProfileScenarioValidationCommand {
            workdir: ".",
            program: "bun",
            args: &["test"],
        }),
        ProfileScenarioKind::StatefulReconciliationBugfix => {
            Some(ProfileScenarioValidationCommand {
                workdir: ".",
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; bun test tests/projection.test.ts; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }; bun test ./tests/.harness/projection.validation.ts; exit $LASTEXITCODE",
                ],
            })
        }
        ProfileScenarioKind::FeatureRolloutConsistencyBugfix => {
            Some(ProfileScenarioValidationCommand {
                workdir: ".",
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; bun test tests/rollout.test.ts; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }; bun test ./tests/.harness/rollout.validation.ts; exit $LASTEXITCODE",
                ],
            })
        }
        ProfileScenarioKind::FrontierRuleTransfer => Some(ProfileScenarioValidationCommand {
            workdir: ".",
            program: "powershell",
            args: &[
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='Stop'; bun test tests/public.test.ts; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }; bun test ./tests/.harness/frontier.validation.ts; exit $LASTEXITCODE",
            ],
        }),
        ProfileScenarioKind::TerminalRepair => Some(ProfileScenarioValidationCommand {
            workdir: ".",
            program: "powershell",
            args: &[
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='Stop'; $settings = Get-Content -LiteralPath 'config/settings.json' -Raw | ConvertFrom-Json; if ($settings.dataPath -ne 'data/report.csv') { throw 'settings.json dataPath must point at data/report.csv' }; $out = bun run start; if ($LASTEXITCODE -ne 0) { throw 'bun run start failed' }; $text = $out -join \"`n\"; if ($text -notlike '*REPORT OK*') { throw 'missing REPORT OK' }; if ($text -notlike '*rows=5*') { throw 'missing rows=5' }; if ($text -notlike '*top=api*') { throw 'missing top=api' }",
            ],
        }),
        ProfileScenarioKind::MultiHopAnalysis => Some(ProfileScenarioValidationCommand {
            workdir: ".",
            program: "powershell",
            args: &[
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='Stop'; $answer = Get-Content -LiteralPath 'answer.json' -Raw | ConvertFrom-Json; if ($answer.product -ne 'Atlas') { throw 'product must be Atlas' }; if ($answer.region -ne 'EMEA') { throw 'region must be EMEA' }; if ([math]::Abs([decimal]$answer.netRevenue - 180) -gt 0.001) { throw 'netRevenue must be 180' }; $explanation = Get-Content -LiteralPath 'answer.md' -Raw; foreach ($term in @('A1','A4','180')) { if ($explanation -notlike \"*$term*\") { throw \"answer.md missing $term\" } }",
            ],
        }),
        ProfileScenarioKind::PolicySupportAgent => Some(ProfileScenarioValidationCommand {
            workdir: ".",
            program: "powershell",
            args: &[
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='Stop'; $resolution = Get-Content -LiteralPath 'resolution.json' -Raw | ConvertFrom-Json; if ($resolution.orderId -ne '5591') { throw 'orderId must be 5591' }; if ($resolution.refundApproved -ne $true) { throw 'refundApproved must be true' }; if ([math]::Abs([decimal]$resolution.refundAmount - 48.5) -gt 0.001) { throw 'refundAmount must be 48.5' }; if ($resolution.refundMethod -ne 'store_credit') { throw 'refundMethod must be store_credit' }; if ($resolution.reasonCode -ne 'damaged_on_arrival') { throw 'reasonCode must be damaged_on_arrival' }; $citations = @($resolution.policyCitations); foreach ($section in @('S3','S4')) { if ($citations -notcontains $section) { throw \"policyCitations missing $section\" } }",
            ],
        }),
        _ => None,
    }
}
