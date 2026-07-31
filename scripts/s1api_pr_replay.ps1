<#
.SYNOPSIS
Prepares and scores Schedule One S1API merged-PR replay benchmarks.

.DESCRIPTION
Each catalog entry uses the first parent of a merged S1API PR as the agent's
workspace baseline. Focused tests introduced by the merge are installed in the
workspace, while the production and documentation diff remains an oracle outside
the agent workspace. Score compares the agent's changed seams with the merged
oracle, so a passing narrow contract test cannot hide unrelated lifecycle drift.
#>
[CmdletBinding(DefaultParameterSetName = 'List')]
param(
    [Parameter(ParameterSetName = 'Prepare', Mandatory = $true)]
    [Parameter(ParameterSetName = 'Score', Mandatory = $true)]
    [Parameter(ParameterSetName = 'Compare', Mandatory = $true)]
    [ValidateSet(158, 156, 147, 146, 145, 144, 143, 140, 138, 137, 136, 133, 132, 130, 129)]
    [int]$Pr,

    [Parameter(ParameterSetName = 'Prepare')]
    [Parameter(ParameterSetName = 'Score')]
    [Parameter(ParameterSetName = 'Compare')]
    [string]$S1ApiRepo = 'C:\Users\ghost\Desktop\Coding\ScheduleOne\S1API',

    [Parameter(ParameterSetName = 'Prepare')]
    [Parameter(ParameterSetName = 'Score')]
    [Parameter(ParameterSetName = 'Compare')]
    [string]$WorkspaceRoot = (Join-Path $PSScriptRoot '..\.spark-profile\s1api-pr-replay'),

    [Parameter(ParameterSetName = 'Prepare', Mandatory = $true)]
    [switch]$Prepare,

    [Parameter(ParameterSetName = 'Score', Mandatory = $true)]
    [switch]$Score,

    [Parameter(ParameterSetName = 'Compare', Mandatory = $true)]
    [switch]$Compare,

    [Parameter(ParameterSetName = 'Score')]
    [string]$Workspace,

    [Parameter(ParameterSetName = 'Compare', Mandatory = $true)]
    [string]$SparkWorkspace,

    [Parameter(ParameterSetName = 'Compare', Mandatory = $true)]
    [string]$CodexWorkspace,

    [Parameter(ParameterSetName = 'Compare')]
    [string]$OutputPath,

    [Parameter(ParameterSetName = 'List')]
    [switch]$List
)

$ErrorActionPreference = 'Stop'

$Catalog = @(
    [pscustomobject]@{ Number = 158; Merge = '4aeb7b64912a279b4f80451d7615b7000663def9'; Title = 'restore supplier roles and live presentation previews'; Category = 'npc-presentation'; Task = 'Use native supplier relationship styling, current relationship callbacks, and live workbench preview updates without replacing runtime definitions.' }
    [pscustomobject]@{ Number = 156; Merge = 'c7dc87fd487039e68e3e976cdf0be42c3ab917ae'; Title = 'configure prefab region'; Category = 'npc-lifecycle'; Task = 'Add pre-spawn native region configuration for custom NPC prefabs.' }
    [pscustomobject]@{ Number = 147; Merge = '16e38610a7408962417e482acb16a6eca063c9dc'; Title = 'restore custom contact and avatar behavior'; Category = 'contacts-presentation'; Task = 'Include custom suppliers in Contacts relationships and retain configured avatar presentation for non-viewmodel equippables.' }
    [pscustomobject]@{ Number = 146; Merge = '12b287b1b82795f921ec9bea5389a77106584882'; Title = 'refine supplier messaging and diagnostics'; Category = 'supplier-lifecycle'; Task = 'Keep supplier order conversations visible and gate high-volume internal diagnostics behind opt-in verbose logging.' }
    [pscustomobject]@{ Number = 145; Merge = '1d7406a35d618174866b4c049a822f4edd7c7cc2'; Title = 'finish beta supplier lifecycle support'; Category = 'supplier-lifecycle'; Task = 'Finish supplier spawn, relationship recommendation, and native lazy-unlock conversation lifecycle support.' }
    [pscustomobject]@{ Number = 144; Merge = 'bdc08ab4b25b2b69432b91316e4acd8f318c58c8'; Title = 'preserve supplier spawn and shop lifecycle'; Category = 'supplier-lifecycle'; Task = 'Preserve active-avatar supplier spawn behavior and required native shop children during listing cleanup.' }
    [pscustomobject]@{ Number = 143; Merge = 'bf07b09bf45370ca69107182f7e1c89e542bb32b'; Title = 'support complete Brick Press visuals'; Category = 'product-presentation'; Task = 'Add opt-in complete Brick Press visuals for mod-owned packaging while retaining the legacy repeated-content path.' }
    [pscustomobject]@{ Number = 140; Merge = '8fc28aa3b7c8907ecc6bb6cfece56a4aa511c73f'; Title = 'add recipe availability controls and supplier lifecycle'; Category = 'supplier-recipes'; Task = 'Add opt-in recipe availability controls and declaration-order-safe supplier lifecycle support.' }
    [pscustomobject]@{ Number = 138; Merge = '0af77106d0cfc9a2432b3347cb2e74387c5dab17'; Title = 'fix runtime item persistence across save reloads'; Category = 'save-lifecycle'; Task = 'Retain builder-created runtime item definitions across registry resets so save hydration resolves them after reload.' }
    [pscustomobject]@{ Number = 137; Merge = '3e29a8a5987087f49ceddcb8b4c22beabebb4ded'; Title = 'add custom consumption profiles'; Category = 'product-api'; Task = 'Add opt-in custom consumption profiles with deterministic resolution, lifecycle cleanup, and compatibility metadata.' }
    [pscustomobject]@{ Number = 136; Merge = '24f025345a6d4aef36217ede582ee4082d02aa1a'; Title = 'add effect clear callbacks'; Category = 'product-api'; Task = 'Add opt-in player and NPC product-effect clear callbacks while preserving existing apply-only behavior.' }
    [pscustomobject]@{ Number = 133; Merge = '7b1b95eec0a99f6b56c6a490b302cbe66b4163fb'; Title = 'refresh late-bound item icons'; Category = 'item-presentation'; Task = 'Refresh already-bound inventory and shop UI when a late runtime item icon becomes available, using durable UI-owned captures.' }
    [pscustomobject]@{ Number = 132; Merge = 'c0823bfaaa2589b109ec8655f81df5f6e1b8292c'; Title = 'support convex functional colliders'; Category = 'product-presentation'; Task = 'Add an opt-in setting that replaces inherited functional collision geometry with convex mesh colliders from the custom visual.' }
    [pscustomobject]@{ Number = 130; Merge = '72b3e2be4eca33d375e038632e2834241c434198'; Title = 'refresh deferred packaging icons after load'; Category = 'product-presentation'; Task = 'When a deferred custom packaging icon enters the cache after save load, refresh only already-bound UI slots matching its case-insensitive product and packaging pair; preserve the loose-icon fallback while queued.' }
    [pscustomobject]@{ Number = 129; Merge = 'd6efa70f6faa918fa6d59051bce73cf693fdcf0e'; Title = 'fix custom shop listings and add icon-only transforms'; Category = 'shop-presentation'; Task = 'Bind custom shop listings to current native cart events and add an opt-in transform for generated icons without changing world presentation.' }
)

function Invoke-Git {
    param([string[]]$Arguments, [string]$WorkingDirectory = $S1ApiRepo)

    $result = & git -C $WorkingDirectory @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "git -C '$WorkingDirectory' $($Arguments -join ' ') failed with exit code $LASTEXITCODE"
    }
    return @($result | Where-Object { $_ -and $_.Trim() })
}

function Get-ReplaySpec {
    param([int]$Number)

    if (-not (Test-Path -LiteralPath (Join-Path $S1ApiRepo '.git'))) {
        throw "S1API repository was not found at '$S1ApiRepo'. Pass -S1ApiRepo with a clean clone containing the merged commits."
    }
    $entry = $Catalog | Where-Object Number -eq $Number
    if (-not $entry) {
        throw "PR #$Number is not in the replay catalog."
    }
    $parent = (Invoke-Git -Arguments @('rev-parse', "$($entry.Merge)^1") | Select-Object -First 1).Trim()
    $allPaths = Invoke-Git -Arguments @('diff', '--name-only', $parent, $entry.Merge)
    $testPaths = @($allPaths | Where-Object { $_ -like 'S1API.Tests/*' })
    $oraclePaths = @($allPaths | Where-Object { $_ -notlike 'S1API.Tests/*' })
    [pscustomobject]@{
        pr = $entry.Number
        title = $entry.Title
        task_context = if ($entry.Task) { $entry.Task } else { $entry.Title }
        category = $entry.Category
        parent_commit = $parent
        merged_commit = $entry.Merge
        oracle_changed_paths = $oraclePaths
        installed_test_paths = $testPaths
        validation_mode = if ($testPaths.Count -gt 0) { 'merged-focused-tests' } else { 'oracle-scope-only' }
        validation_commands = @(
            'dotnet restore S1API.sln -p:Configuration=MonoMelon',
            'dotnet build S1API.sln -c MonoMelon --no-restore -p:AutomateLocalDeployment=false',
            'dotnet test S1API.Tests/S1API.Tests.csproj -c MonoMelon --no-restore --no-build',
            'dotnet restore S1API.sln -p:Configuration=Il2CppMelon',
            'dotnet build S1API.sln -c Il2CppMelon --no-restore -p:AutomateLocalDeployment=false',
            'dotnet test S1API.Tests/S1API.Tests.csproj -c Il2CppMelon --no-restore --no-build'
        )
    }
}

function Get-NormalizedAddedLines {
    param([string[]]$DiffLines)

    @(
        $DiffLines |
            Where-Object { $_.StartsWith('+') -and -not $_.StartsWith('+++') } |
            ForEach-Object {
                $line = $_.Substring(1).Trim()
                if ($line) {
                    $line -replace '\s+', ' '
                }
            }
    )
}

function Get-AddedLineOverlap {
    param([string[]]$OracleLines, [string[]]$CandidateLines)

    $remaining = @{}
    foreach ($line in $OracleLines) {
        if (-not $remaining.ContainsKey($line)) {
            $remaining[$line] = 0
        }
        $remaining[$line]++
    }

    $matched = 0
    foreach ($line in $CandidateLines) {
        if ($remaining.ContainsKey($line) -and $remaining[$line] -gt 0) {
            $matched++
            $remaining[$line]--
        }
    }
    return $matched
}

if ($List -or (-not $Prepare -and -not $Score -and -not $Compare)) {
    $Catalog | Select-Object Number, Category, Title, Merge | Format-Table -AutoSize
    return
}

$spec = Get-ReplaySpec -Number $Pr

if ($Prepare) {
    if ($spec.validation_mode -eq 'oracle-scope-only') {
        Write-Warning "PR #$Pr has no committed S1API.Tests changes. This replay can score oracle seam and scope fidelity, but it needs separate behavioral evidence before it can be treated as a passing task."
    }
    $resolvedRoot = [System.IO.Path]::GetFullPath($WorkspaceRoot)
    $target = Join-Path $resolvedRoot ("pr{0}" -f $Pr)
    if (Test-Path -LiteralPath $target) {
        throw "Replay workspace already exists: '$target'. Preserve it for evidence or choose a different -WorkspaceRoot."
    }
    New-Item -ItemType Directory -Path $resolvedRoot -Force | Out-Null
    Invoke-Git -Arguments @('worktree', 'add', '--detach', $target, $spec.parent_commit) | Out-Null
    try {
        if ($spec.installed_test_paths.Count -gt 0) {
            Invoke-Git -WorkingDirectory $target -Arguments (@('checkout', $spec.merged_commit, '--') + $spec.installed_test_paths) | Out-Null
        }
        $localBuildProps = Join-Path $S1ApiRepo 'local.build.props'
        $copiedLocalBuildProps = Test-Path -LiteralPath $localBuildProps
        if ($copiedLocalBuildProps) {
            Copy-Item -LiteralPath $localBuildProps -Destination (Join-Path $target 'local.build.props') -Force
        }
        $scheduleOneSkill = Join-Path (Split-Path -Parent $S1ApiRepo) '.agents\skills\schedule-one-modding'
        $copiedScheduleOneSkill = Test-Path -LiteralPath (Join-Path $scheduleOneSkill 'SKILL.md')
        if ($copiedScheduleOneSkill) {
            Copy-Item -LiteralPath $scheduleOneSkill -Destination (Join-Path $target '.agents\skills\schedule-one-modding') -Recurse -Force
        }
        $manifestPath = Join-Path $target '.spark-s1api-replay.json'
        $spec | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $manifestPath -Encoding utf8
        $taskBriefPath = Join-Path $target '.spark-s1api-replay.md'
        $testEvidence = if ($spec.installed_test_paths.Count -gt 0) {
            ($spec.installed_test_paths | ForEach-Object { "- ``$_``" }) -join [Environment]::NewLine
        }
        else {
            '- No focused test was introduced by this PR; collect explicit behavioral evidence before claiming success.'
        }
        $workingScope = ($spec.oracle_changed_paths | ForEach-Object { "- ``$_``" }) -join [Environment]::NewLine
        $validationCommands = ($spec.validation_commands | ForEach-Object { "    $_" }) -join [Environment]::NewLine
        @"
# S1API PR replay task #$($spec.pr)

Implement the issue-level behavior described by **$($spec.title)**.

$($spec.task_context)

## Evidence

Read these committed focused tests first:
$testEvidence

## Working scope

Keep the implementation and documentation changes within these task seams unless a compiler or focused test failure demonstrates a necessary adjacent change:
$workingScope

Do not enumerate the repository, inspect Git history/remotes, or search outside the working scope before the first implementation attempt. Match public names and types exactly as they are used by the focused tests.

## Validation

After the first implementation attempt, run the following commands in order. Do not substitute a bare `dotnet test` command; this repository needs its runtime-specific restore graph.

``````powershell
$validationCommands
``````

If validation fails, make the smallest scoped correction supported by the error, then rerun the failed runtime phase. Report blocked runtime-wrapper drift separately from an implementation failure.
"@ | Set-Content -LiteralPath $taskBriefPath -Encoding utf8
    }
    catch {
        Write-Warning "Replay preparation failed after worktree creation; preserve '$target' for inspection."
        throw
    }
    [pscustomobject]@{
        workspace = $target
        manifest = $manifestPath
        task_brief = $taskBriefPath
        pr = $spec.pr
        validation_mode = $spec.validation_mode
        local_build_props_copied = $copiedLocalBuildProps
        schedule_one_skill_copied = $copiedScheduleOneSkill
        installed_test_paths = $spec.installed_test_paths
        oracle_changed_paths = $spec.oracle_changed_paths
    } | ConvertTo-Json -Depth 4
    return
}

if ($Compare) {
    foreach ($candidate in @($SparkWorkspace, $CodexWorkspace)) {
        if (-not (Test-Path -LiteralPath (Join-Path $candidate '.git'))) {
            throw "Replay workspace was not found at '$candidate'. Run -Prepare and a candidate agent first."
        }
    }
    $spark = & $PSCommandPath -Score -Pr $Pr -S1ApiRepo $S1ApiRepo -WorkspaceRoot $WorkspaceRoot -Workspace $SparkWorkspace | ConvertFrom-Json
    $codex = & $PSCommandPath -Score -Pr $Pr -S1ApiRepo $S1ApiRepo -WorkspaceRoot $WorkspaceRoot -Workspace $CodexWorkspace | ConvertFrom-Json
    $patchF1Delta = [Math]::Round($spark.oracle_patch_overlap_f1 - $codex.oracle_patch_overlap_f1, 1)
    $scopeDelta = [Math]::Round($spark.scope_score - $codex.scope_score, 1)
    $verdict = if ($spark.scope_score -eq 100 -and $codex.scope_score -eq 100 -and $patchF1Delta -gt 0) {
        'spark-ahead-on-oracle-patch-diagnostic'
    }
    elseif ($spark.scope_score -eq 100 -and $codex.scope_score -eq 100 -and $patchF1Delta -lt 0) {
        'codex-ahead-on-oracle-patch-diagnostic'
    }
    else {
        'inconclusive-or-scope-divergent'
    }
    $comparison = [pscustomobject]@{
        pr = $Pr
        title = $spark.title
        validation_note = 'This command compares hidden oracle scope and patch diagnostics only. Run the manifest runtime validation separately before treating either candidate as behaviorally accepted.'
        spark = $spark
        codex = $codex
        delta = [pscustomobject]@{
            scope_score = $scopeDelta
            oracle_patch_overlap_f1 = $patchF1Delta
            oracle_patch_precision_percent = [Math]::Round($spark.oracle_patch_precision_percent - $codex.oracle_patch_precision_percent, 1)
            oracle_patch_recall_percent = [Math]::Round($spark.oracle_patch_recall_percent - $codex.oracle_patch_recall_percent, 1)
        }
        verdict = $verdict
    }
    $json = $comparison | ConvertTo-Json -Depth 8
    if ($OutputPath) {
        $resolvedOutputPath = [System.IO.Path]::GetFullPath($OutputPath)
        $outputDirectory = Split-Path -Parent $resolvedOutputPath
        if ($outputDirectory) {
            New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null
        }
        $json | Set-Content -LiteralPath $resolvedOutputPath -Encoding utf8
    }
    $json
    return
}

if (-not $Workspace) {
    $Workspace = Join-Path ([System.IO.Path]::GetFullPath($WorkspaceRoot)) ("pr{0}" -f $Pr)
}
if (-not (Test-Path -LiteralPath (Join-Path $Workspace '.git'))) {
    throw "Replay workspace was not found at '$Workspace'. Run -Prepare first."
}

$installedTests = @($spec.installed_test_paths)
$candidatePaths = @(Invoke-Git -WorkingDirectory $Workspace -Arguments @('diff', '--name-only', 'HEAD') | Where-Object { $_ -notin $installedTests })
$matchedPaths = @($candidatePaths | Where-Object { $_ -in $spec.oracle_changed_paths })
$missingOraclePaths = @($spec.oracle_changed_paths | Where-Object { $_ -notin $candidatePaths })
$extraPaths = @($candidatePaths | Where-Object { $_ -notin $spec.oracle_changed_paths })
$coverage = if ($spec.oracle_changed_paths.Count -eq 0) { 0.0 } else { [Math]::Round(100.0 * $matchedPaths.Count / $spec.oracle_changed_paths.Count, 1) }
$scopeScore = [Math]::Max(0.0, [Math]::Round($coverage - (15.0 * $extraPaths.Count), 1))
$oracleDiff = Invoke-Git -Arguments (@('diff', '--unified=0', "$($spec.parent_commit)..$($spec.merged_commit)", '--') + $spec.oracle_changed_paths)
$candidateDiff = Invoke-Git -WorkingDirectory $Workspace -Arguments (@('diff', '--unified=0', 'HEAD', '--') + $candidatePaths)
$oracleAddedLines = Get-NormalizedAddedLines -DiffLines $oracleDiff
$candidateAddedLines = Get-NormalizedAddedLines -DiffLines $candidateDiff
$matchedAddedLines = Get-AddedLineOverlap -OracleLines $oracleAddedLines -CandidateLines $candidateAddedLines
$patchPrecision = if ($candidateAddedLines.Count -eq 0) { 0.0 } else { [Math]::Round(100.0 * $matchedAddedLines / $candidateAddedLines.Count, 1) }
$patchRecall = if ($oracleAddedLines.Count -eq 0) { 0.0 } else { [Math]::Round(100.0 * $matchedAddedLines / $oracleAddedLines.Count, 1) }
$patchOverlapF1 = if (($patchPrecision + $patchRecall) -eq 0.0) { 0.0 } else { [Math]::Round(2.0 * $patchPrecision * $patchRecall / ($patchPrecision + $patchRecall), 1) }
[pscustomobject]@{
    pr = $spec.pr
    title = $spec.title
    category = $spec.category
    validation_mode = $spec.validation_mode
    workspace = [System.IO.Path]::GetFullPath($Workspace)
    merged_commit = $spec.merged_commit
    oracle_changed_paths = $spec.oracle_changed_paths
    candidate_changed_paths = $candidatePaths
    matched_paths = $matchedPaths
    missing_oracle_paths = $missingOraclePaths
    extra_paths = $extraPaths
    changed_seam_coverage_percent = $coverage
    scope_score = $scopeScore
    oracle_added_lines = $oracleAddedLines.Count
    candidate_added_lines = $candidateAddedLines.Count
    matched_oracle_added_lines = $matchedAddedLines
    oracle_patch_precision_percent = $patchPrecision
    oracle_patch_recall_percent = $patchRecall
    oracle_patch_overlap_f1 = $patchOverlapF1
} | ConvertTo-Json -Depth 5
