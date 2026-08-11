use crate::cli::ProfileScenarioKind;

use super::{ProfileScenarioValidationCheck, ProfileScenarioValidationCommand};

pub(crate) fn profile_scenario_validation_checks(
    scenario: ProfileScenarioKind,
) -> &'static [ProfileScenarioValidationCheck] {
    match scenario {
        ProfileScenarioKind::PullRequestReview => &[
            ProfileScenarioValidationCheck {
                name: "structured finding schema",
                weight: 5,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $raw = Get-Content -LiteralPath 'review.json' -Raw; if ($raw.TrimStart()[0] -ne '[') { throw 'review.json must be an array' }; $items = @($raw | ConvertFrom-Json | ForEach-Object { $_ }); foreach ($item in $items) { $keys = @($item.psobject.Properties.Name | Sort-Object); if (($keys -join ',') -ne 'evidence,fix,impact,severity,source,test') { throw 'finding schema mismatch' }; foreach ($key in $keys) { if ([string]::IsNullOrWhiteSpace([string]$item.$key)) { throw \"empty finding field $key\" } } }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "authorization boundary regression",
                weight: 9,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $f = @(Get-Content 'review.json' -Raw | ConvertFrom-Json | ForEach-Object { $_ } | Where-Object source -eq 'src/checkout.ts'); if ($f.Count -ne 1) { throw 'missing checkout finding' }; $t = $f[0] | ConvertTo-Json -Compress; foreach ($term in @('includes','read-only-admin','tests/checkout.test.ts')) { if ($t -notlike \"*$term*\") { throw \"checkout finding missing $term\" } }; if ($t -notmatch '(?i)(exact|explicit|strict|===|equality)') { throw 'missing exact-role fix' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "stable pagination regression",
                weight: 6,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $f = @(Get-Content 'review.json' -Raw | ConvertFrom-Json | ForEach-Object { $_ } | Where-Object source -eq 'src/orders.ts'); if ($f.Count -ne 1) { throw 'missing orders finding' }; $t = $f[0] | ConvertTo-Json -Compress; foreach ($term in @('createdAt','tests/orders.test.ts')) { if ($t -notlike \"*$term*\") { throw \"orders finding missing $term\" } }; if ($t -notmatch '(?i)(tie|same|duplicate|equal).{0,80}(timestamp|createdAt)|(timestamp|createdAt).{0,80}(tie|same|duplicate|equal)' -or $t -notmatch '(?i)(composite|tie.?break|id)' -or $t -notmatch '(?i)(skip|miss|omit|drop|lose|lost)') { throw 'incomplete pagination finding' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "listener cleanup regression",
                weight: 6,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $f = @(Get-Content 'review.json' -Raw | ConvertFrom-Json | ForEach-Object { $_ } | Where-Object source -eq 'src/useSocketMessages.ts'); if ($f.Count -ne 1) { throw 'missing socket finding' }; $t = $f[0] | ConvertTo-Json -Compress; foreach ($term in @('removeEventListener','tests/useSocketMessages.test.ts')) { if ($t -notlike \"*$term*\") { throw \"socket finding missing $term\" } }; if ($t -notmatch '(?i)(identity|same|different|new).{0,80}(callback|handler|function)|(callback|handler|function).{0,80}(identity|same|different|new)' -or $t -notmatch '(?i)(leak|duplicate|remain|detach|remove)') { throw 'incomplete listener finding' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "payment idempotency regression",
                weight: 9,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $f = @(Get-Content 'review.json' -Raw | ConvertFrom-Json | ForEach-Object { $_ } | Where-Object source -eq 'src/payments.ts'); if ($f.Count -ne 1) { throw 'missing payment finding' }; $t = $f[0] | ConvertTo-Json -Compress; foreach ($term in @('Date.now','tests/payments.test.ts')) { if ($t -notlike \"*$term*\") { throw \"payment finding missing $term\" } }; if ($t -notmatch '(?i)attempt.?id' -or $t -notmatch '(?i)(retry|idempoten)' -or $t -notmatch '(?i)(duplicate|double).{0,50}(charge|capture|payment)') { throw 'incomplete payment finding' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "awaited batch persistence regression",
                weight: 8,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $f = @(Get-Content 'review.json' -Raw | ConvertFrom-Json | ForEach-Object { $_ } | Where-Object source -eq 'src/batchOrders.ts'); if ($f.Count -ne 1) { throw 'missing batch finding' }; $t = $f[0] | ConvertTo-Json -Compress; foreach ($term in @('forEach','tests/batchOrders.test.ts')) { if ($t -notlike \"*$term*\") { throw \"batch finding missing $term\" } }; if ($t -notmatch '(?i)(before|early|pending|in.?flight|not await)' -or $t -notmatch '(?i)(Promise\\.all|for.{0,20}of|await all)') { throw 'incomplete batch finding' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "tenant cache isolation regression",
                weight: 9,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $f = @(Get-Content 'review.json' -Raw | ConvertFrom-Json | ForEach-Object { $_ } | Where-Object source -eq 'src/reportCache.ts'); if ($f.Count -ne 1) { throw 'missing cache finding' }; $t = $f[0] | ConvertTo-Json -Compress; foreach ($term in @('tenantId','reportId','tests/reportCache.test.ts')) { if ($t -notlike \"*$term*\") { throw \"cache finding missing $term\" } }; if ($t -notmatch '(?i)(cross.?tenant|wrong report|leak|expos)' -or $t -notmatch '(?i)(key|cache)') { throw 'incomplete tenant cache finding' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "rollout zero-boundary regression",
                weight: 5,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $f = @(Get-Content 'review.json' -Raw | ConvertFrom-Json | ForEach-Object { $_ } | Where-Object source -eq 'src/rollout.ts'); if ($f.Count -ne 1) { throw 'missing rollout finding' }; $t = $f[0] | ConvertTo-Json -Compress; if ($t -notlike '*tests/rollout.test.ts*' -or $t -notmatch '(?i)(inclusive|less than or equal|<=)' -or $t -notmatch '(?i)(0%|zero|bucket 0)' -or $t -notmatch '(?i)(strict|exclusive|[^=]<[^=])') { throw 'incomplete rollout finding' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "credential logging regression",
                weight: 7,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $f = @(Get-Content 'review.json' -Raw | ConvertFrom-Json | ForEach-Object { $_ } | Where-Object source -eq 'src/audit.ts'); if ($f.Count -ne 1) { throw 'missing audit finding' }; $t = $f[0] | ConvertTo-Json -Compress; foreach ($term in @('accessToken','tests/audit.test.ts')) { if ($t -notlike \"*$term*\") { throw \"audit finding missing $term\" } }; if ($t -notmatch '(?i)(log|expos|leak)' -or $t -notmatch '(?i)(omit|remove|redact|mask)') { throw 'incomplete credential finding' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "webhook retry regression",
                weight: 9,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $f = @(Get-Content 'review.json' -Raw | ConvertFrom-Json | ForEach-Object { $_ } | Where-Object source -eq 'src/webhookDelivery.ts'); if ($f.Count -ne 1) { throw 'missing webhook finding' }; $t = $f[0] | ConvertTo-Json -Compress; foreach ($term in @('processedEvents','tests/webhookDelivery.test.ts')) { if ($t -notlike \"*$term*\") { throw \"webhook finding missing $term\" } }; if ($t -notmatch '(?i)(fail|reject|throw|error)' -or $t -notmatch '(?i)(retry|redeliver)' -or $t -notmatch '(?i)(after|success|remove|delete)') { throw 'incomplete webhook retry finding' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "single-use invite race regression",
                weight: 9,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $f = @(Get-Content 'review.json' -Raw | ConvertFrom-Json | ForEach-Object { $_ } | Where-Object source -eq 'src/invites.ts'); if ($f.Count -ne 1) { throw 'missing invite finding' }; $t = $f[0] | ConvertTo-Json -Compress; foreach ($term in @('isUsed','markUsed','tests/invites.test.ts')) { if ($t -notlike \"*$term*\") { throw \"invite finding missing $term\" } }; if ($t -notmatch '(?i)(concurr|race|simult|two)' -or $t -notmatch '(?i)(atomic|transaction|compare|consume|claim)') { throw 'incomplete invite race finding' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "zero-valued settings regression",
                weight: 6,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $f = @(Get-Content 'review.json' -Raw | ConvertFrom-Json | ForEach-Object { $_ } | Where-Object source -eq 'src/runtimeSettings.ts'); if ($f.Count -ne 1) { throw 'missing runtime settings finding' }; $t = $f[0] | ConvertTo-Json -Compress; if ($t -notlike '*tests/runtimeSettings.test.ts*' -or $t -notmatch '(?i)(zero|0|falsy)' -or $t -notmatch '(?i)(nullish|\\?\\?|undefined)') { throw 'incomplete zero-value finding' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "review precision",
                weight: 6,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $items = @(Get-Content 'review.json' -Raw | ConvertFrom-Json | ForEach-Object { $_ }); if ($items.Count -ne 11) { throw \"expected 11 actionable findings, got $($items.Count)\" }; foreach ($safe in @('src/reportQuery.ts','src/sessionExpiry.ts')) { if (@($items | Where-Object source -eq $safe).Count -ne 0) { throw \"false positive for $safe\" } }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "markdown and structured review agree",
                weight: 6,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $items = @(Get-Content 'review.json' -Raw | ConvertFrom-Json | ForEach-Object { $_ }); $markdown = Get-Content 'review.md' -Raw; foreach ($item in $items) { if ($markdown -notlike \"*$($item.source)*\") { throw \"review.md missing $($item.source)\" } }; if ($markdown.Length -lt 800) { throw 'review.md is too shallow' }",
                ],
            },
        ],
        ProfileScenarioKind::DependencyUpgradeTriage => &[
            ProfileScenarioValidationCheck {
                name: "upgrade evidence",
                weight: 10,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $content = Get-Content -LiteralPath 'upgrade-triage.md' -Raw; foreach ($term in @('@acme/time-utils','2.0.0')) { if ($content -notlike \"*$term*\") { throw \"missing $term\" } }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "documented timezone behavior change",
                weight: 30,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $content = Get-Content -LiteralPath 'upgrade-triage.md' -Raw; foreach ($term in @('parseBusinessDate','UTC','local')) { if ($content -notlike \"*$term*\") { throw \"missing $term\" } }; if ($content -notmatch '(?i)date.?only') { throw 'missing affected input shape' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "billing impact",
                weight: 15,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $content = Get-Content -LiteralPath 'upgrade-triage.md' -Raw; if ($content -notlike '*src/billingWindow.ts*') { throw 'missing affected source' }; if ($content -notmatch '(?i)(billing|cutoff|month|day)') { throw 'missing user impact' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "utc mitigation",
                weight: 10,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $content = Get-Content -LiteralPath 'upgrade-triage.md' -Raw; if ($content -notmatch \"zone\\s*:\\s*[''`\"]utc[''`\"]\") { throw 'missing explicit UTC option' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "timezone regression coverage",
                weight: 10,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $content = Get-Content -LiteralPath 'upgrade-triage.md' -Raw; if ($content -notlike '*tests/billingWindow.test.ts*') { throw 'missing affected test path' }; if ($content -notmatch '(?i)(non.?UTC|timezone|time zone|TZ)') { throw 'missing timezone test condition' }; if ($content -notmatch '(?i)(test gap|missing test|add.{0,30}test|regression test)') { throw 'missing test recommendation' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "documented week-start behavior change",
                weight: 15,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $content = Get-Content -LiteralPath 'upgrade-triage.md' -Raw; foreach ($term in @('startOfBusinessWeek','src/billingWeek.ts','Monday','Sunday')) { if ($content -notlike \"*$term*\") { throw \"missing $term\" } }; if ($content -notmatch '(?i)(default|defaults|changed|change)') { throw 'missing changed-default explanation' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "week-start mitigation and coverage",
                weight: 10,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $content = Get-Content -LiteralPath 'upgrade-triage.md' -Raw; if ($content -notmatch 'weekStartsOn\\s*:\\s*1') { throw 'missing Monday option' }; if ($content -notlike '*tests/billingWeek.test.ts*') { throw 'missing week test path' }; if ($content -notmatch '(?i)(test gap|missing test|add.{0,30}test|regression test)') { throw 'missing week regression coverage' }",
                ],
            },
        ],
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
        ProfileScenarioKind::TechnicalEssay => &[
            ProfileScenarioValidationCheck {
                name: "required title",
                weight: 20,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $content = Get-Content -LiteralPath 'essay.md' -Raw; if ($content -notlike '*Operational Visibility Is a Product Feature*') { throw 'missing required title' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "source citations",
                weight: 30,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $content = Get-Content -LiteralPath 'essay.md' -Raw; foreach ($term in @('[S1]','[S2]','[S3]')) { if ($content -notlike \"*$term*\") { throw \"missing $term\" } }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "minimum length",
                weight: 25,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $content = Get-Content -LiteralPath 'essay.md' -Raw; $words = @($content -split '\\s+' | Where-Object { $_ }); if ($words.Count -lt 350) { throw \"essay too short: $($words.Count) words\" }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "section structure",
                weight: 25,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $content = Get-Content -LiteralPath 'essay.md' -Raw; $headings = @($content -split \"`r?`n\" | Where-Object { $_ -like '## *' }); if ($headings.Count -lt 2) { throw 'missing section headings' }",
                ],
            },
        ],
        ProfileScenarioKind::ConfigMigration => &[
            ProfileScenarioValidationCheck {
                name: "schema version",
                weight: 20,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $json = Get-Content -LiteralPath 'config/app.json' -Raw | ConvertFrom-Json; if ($json.schemaVersion -ne 2) { throw 'schemaVersion not 2' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "authentication migration",
                weight: 20,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $json = Get-Content -LiteralPath 'config/app.json' -Raw | ConvertFrom-Json; if ($json.authentication.method -ne 'password') { throw 'authentication.method not preserved' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "retry migration",
                weight: 25,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $json = Get-Content -LiteralPath 'config/app.json' -Raw | ConvertFrom-Json; if ($json.retry.maxAttempts -ne 3) { throw 'retry.maxAttempts not migrated' }; if ($json.retry.backoffMs -ne 250) { throw 'retry.backoffMs not preserved' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "stale contract removed",
                weight: 20,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $all = (Get-Content -LiteralPath 'src/config.ts' -Raw) + (Get-Content -LiteralPath 'docs/config.md' -Raw) + (Get-Content -LiteralPath 'config/app.json' -Raw); foreach ($term in @('authMode','retries: number','retry.retries')) { if ($all -like \"*$term*\") { throw \"stale term $term\" } }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "new contract documented",
                weight: 15,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $all = (Get-Content -LiteralPath 'src/config.ts' -Raw) + (Get-Content -LiteralPath 'docs/config.md' -Raw) + (Get-Content -LiteralPath 'config/app.json' -Raw); foreach ($term in @('authentication','method','maxAttempts','schemaVersion: 2')) { if ($all -notlike \"*$term*\") { throw \"missing $term\" } }",
                ],
            },
        ],
        ProfileScenarioKind::OpsReport => &[
            ProfileScenarioValidationCheck {
                name: "ticket totals",
                weight: 25,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $m = Get-Content -LiteralPath 'metrics.json' -Raw | ConvertFrom-Json; if ($m.totalTickets -ne 8 -or $m.openTickets -ne 5) { throw 'ticket totals are incorrect' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "open priority count",
                weight: 15,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $m = Get-Content -LiteralPath 'metrics.json' -Raw | ConvertFrom-Json; if ($m.p1Open -ne 2) { throw 'p1Open must be 2' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "average open duration",
                weight: 20,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $m = Get-Content -LiteralPath 'metrics.json' -Raw | ConvertFrom-Json; if ([math]::Abs([double]$m.averageOpenMinutes - 51.4) -gt 0.01) { throw 'averageOpenMinutes must be 51.4' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "highest risk team",
                weight: 25,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $r = Get-Content -LiteralPath 'report.md' -Raw; $p = (($r -replace '[*`#_]', '') -replace '\\s+', ' ').Trim(); if ($p -notmatch '(?i)(highest-risk team\\s*(?:-|:)?\\s*team\\s*:\\s*billing|highest-risk team\\s*(:|-|is)?\\s*billing|billing\\s+(is\\s+)?(the\\s+)?highest-risk team|billing\\s+team\\s+is\\s+highest\\s+risk)') { throw 'billing must be highest-risk team' }; if ($p -match '(?i)(highest-risk team\\s*(?:-|:)?\\s*team\\s*:\\s*api|highest-risk team\\s*(:|-|is)?\\s*api|api\\s+(is\\s+)?(the\\s+)?highest-risk team|api\\s+team\\s+is\\s+highest\\s+risk)') { throw 'api incorrectly identified as highest-risk team' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "risk evidence",
                weight: 15,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $r = Get-Content -LiteralPath 'report.md' -Raw; if ($r -notmatch '95') { throw 'missing 95 minute risk evidence' }",
                ],
            },
        ],
        ProfileScenarioKind::MultiModuleBugfix => &[
            ProfileScenarioValidationCheck {
                name: "final invoice rounding",
                weight: 33,
                program: "bun",
                args: &[
                    "-e",
                    "import { buildInvoiceLines } from './src/invoice.ts'; import { invoiceTotalCents } from './src/total.ts'; const lines = buildInvoiceLines([{ sku: 'a', quantity: 1, unitPriceCents: 20.5 }, { sku: 'b', quantity: 1, unitPriceCents: 20.5 }]); if (invoiceTotalCents(lines, 0, 0) !== 41) throw new Error('final invoice rounding failed');",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "discount before tax",
                weight: 33,
                program: "bun",
                args: &[
                    "-e",
                    "import { buildInvoiceLines } from './src/invoice.ts'; import { invoiceTotalCents } from './src/total.ts'; const lines = buildInvoiceLines([{ sku: 'a', quantity: 1, unitPriceCents: 1000 }]); if (invoiceTotalCents(lines, 100, 1000) !== 990) throw new Error('discount before tax failed');",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "fractional precision",
                weight: 34,
                program: "bun",
                args: &[
                    "-e",
                    "import { buildInvoiceLines } from './src/invoice.ts'; import { invoiceTotalCents } from './src/total.ts'; const lines = buildInvoiceLines([{ sku: 'a', quantity: 3, unitPriceCents: 333.34 }, { sku: 'b', quantity: 1, unitPriceCents: 10.01 }]); if (invoiceTotalCents(lines, 100, 1000) !== 1001) throw new Error('fractional precision failed');",
                ],
            },
        ],
        ProfileScenarioKind::TerminalRepair => &[
            ProfileScenarioValidationCheck {
                name: "configured data path",
                weight: 25,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $s = Get-Content -LiteralPath 'config/settings.json' -Raw | ConvertFrom-Json; if ($s.dataPath -ne 'data/report.csv') { throw 'settings.json dataPath is incorrect' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "command completes",
                weight: 25,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $out = bun run start; if ($LASTEXITCODE -ne 0) { throw 'bun run start failed' }; if (($out -join \"`n\") -notlike '*REPORT OK*') { throw 'missing REPORT OK' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "row count",
                weight: 25,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $out = bun run start; if ($LASTEXITCODE -ne 0) { throw 'bun run start failed' }; if (($out -join \"`n\") -notlike '*rows=5*') { throw 'missing rows=5' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "top team",
                weight: 25,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $out = bun run start; if ($LASTEXITCODE -ne 0) { throw 'bun run start failed' }; if (($out -join \"`n\") -notlike '*top=api*') { throw 'missing top=api' }",
                ],
            },
        ],
        ProfileScenarioKind::MultiHopAnalysis => &[
            ProfileScenarioValidationCheck {
                name: "product identification",
                weight: 20,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $a = Get-Content -LiteralPath 'answer.json' -Raw | ConvertFrom-Json; if ($a.product -ne 'Atlas') { throw 'product must be Atlas' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "region identification",
                weight: 20,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $a = Get-Content -LiteralPath 'answer.json' -Raw | ConvertFrom-Json; if ($a.region -ne 'EMEA') { throw 'region must be EMEA' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "net revenue calculation",
                weight: 35,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $a = Get-Content -LiteralPath 'answer.json' -Raw | ConvertFrom-Json; if ([math]::Abs([decimal]$a.netRevenue - 180) -gt 0.001) { throw 'netRevenue must be 180' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "grounded explanation",
                weight: 25,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $e = Get-Content -LiteralPath 'answer.md' -Raw; foreach ($term in @('A1','A4','180')) { if ($e -notlike \"*$term*\") { throw \"answer.md missing $term\" } }",
                ],
            },
        ],
        ProfileScenarioKind::PolicySupportAgent => &[
            ProfileScenarioValidationCheck {
                name: "order identity",
                weight: 15,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $r = Get-Content -LiteralPath 'resolution.json' -Raw | ConvertFrom-Json; if ($r.orderId -ne '5591') { throw 'orderId must be 5591' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "refund approval",
                weight: 15,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $r = Get-Content -LiteralPath 'resolution.json' -Raw | ConvertFrom-Json; if ($r.refundApproved -ne $true) { throw 'refundApproved must be true' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "refund amount",
                weight: 25,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $r = Get-Content -LiteralPath 'resolution.json' -Raw | ConvertFrom-Json; if ([math]::Abs([decimal]$r.refundAmount - 48.5) -gt 0.001) { throw 'refundAmount must be 48.5' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "refund resolution",
                weight: 25,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $r = Get-Content -LiteralPath 'resolution.json' -Raw | ConvertFrom-Json; if ($r.refundMethod -ne 'store_credit') { throw 'refundMethod must be store_credit' }; if ($r.reasonCode -ne 'damaged_on_arrival') { throw 'reasonCode must be damaged_on_arrival' }",
                ],
            },
            ProfileScenarioValidationCheck {
                name: "policy citations",
                weight: 20,
                program: "powershell",
                args: &[
                    "-NoProfile",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $c = @((Get-Content -LiteralPath 'resolution.json' -Raw | ConvertFrom-Json).policyCitations); foreach ($section in @('S3','S4')) { if ($c -notcontains $section) { throw \"policyCitations missing $section\" } }",
                ],
            },
        ],
        _ => &[],
    }
}

pub(crate) fn profile_scenario_validation_command(
    scenario: ProfileScenarioKind,
) -> Option<ProfileScenarioValidationCommand> {
    let command = match scenario {
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
                "$ErrorActionPreference='Stop'; foreach ($path in @('review.json','review.md')) { if (-not (Test-Path -LiteralPath $path)) { throw \"missing $path\" } }; $items = @(Get-Content -LiteralPath 'review.json' -Raw | ConvertFrom-Json | ForEach-Object { $_ }); if ($items.Count -eq 0) { throw 'review.json has no findings' }; $content = Get-Content -LiteralPath 'review.md' -Raw; if ($content.Length -lt 120) { throw 'review.md is too short to contain grounded findings' }",
            ],
        }),
        ProfileScenarioKind::DependencyUpgradeTriage => Some(ProfileScenarioValidationCommand {
            workdir: ".",
            program: "powershell",
            args: &[
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='Stop'; if (-not (Test-Path -LiteralPath 'upgrade-triage.md')) { throw 'missing upgrade-triage.md' }; $content = Get-Content -LiteralPath 'upgrade-triage.md' -Raw; if ($content.Length -lt 120) { throw 'upgrade-triage.md is too short to contain grounded analysis' }",
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
    };
    command.map(|mut command| {
        if command.program == "powershell" && !cfg!(windows) {
            command.program = "pwsh";
        }
        command
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn granular_validation_weights_cover_the_full_quality_score() {
        let scenarios = [
            ProfileScenarioKind::TechnicalEssay,
            ProfileScenarioKind::ConfigMigration,
            ProfileScenarioKind::OpsReport,
            ProfileScenarioKind::MultiModuleBugfix,
            ProfileScenarioKind::TerminalRepair,
            ProfileScenarioKind::MultiHopAnalysis,
            ProfileScenarioKind::PolicySupportAgent,
            ProfileScenarioKind::StatefulReconciliationBugfix,
            ProfileScenarioKind::FeatureRolloutConsistencyBugfix,
            ProfileScenarioKind::FrontierRuleTransfer,
            ProfileScenarioKind::InventoryRebalancePlan,
            ProfileScenarioKind::ExperimentRolloutAudit,
            ProfileScenarioKind::PullRequestReview,
            ProfileScenarioKind::DependencyUpgradeTriage,
        ];

        for scenario in scenarios {
            let checks = profile_scenario_validation_checks(scenario);
            assert!(!checks.is_empty(), "{scenario:?} has no quality checks");
            assert_eq!(
                checks.iter().map(|check| check.weight).sum::<u32>(),
                100,
                "{scenario:?} quality weights must total 100"
            );
        }
    }

    #[test]
    fn powershell_validators_use_the_platform_executable_name() {
        let command = profile_scenario_validation_command(ProfileScenarioKind::TerminalRepair)
            .expect("terminal repair validation");
        assert_eq!(
            command.program,
            if cfg!(windows) { "powershell" } else { "pwsh" }
        );
    }
}
