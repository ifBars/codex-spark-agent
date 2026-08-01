# Spark Bench web data

The published site intentionally starts with an unavailable usage-history artifact at `src/data/usage-history.json`. It contains no session data, prompts, paths, tool output, account credentials, quota, or price.

To generate a local aggregate, first export it from Spark:

```powershell
spark usage --history --json --output usage-history.json
```

Then explicitly sanitize and import it for a local site build:

```powershell
bun ..\scripts\import_usage_history.mjs --input ..\usage-history.json --output src\data\usage-history.json
```

The importer accepts only the versioned `spark.usage_history.v1` aggregate envelope. It rejects session identifiers, prompts/messages, paths/cwd, tool output, auth fields, and raw payloads; it discards additive keys that are not part of the public aggregate schema. Do not commit a real local history artifact without intentionally reviewing it.

The Usage Evidence panel keeps three distinct claims separate:

- Source-reported local token activity
- Pricing availability (Spark remains unavailable unless a source explicitly supplies a valid estimate)
- Account quota, which is deliberately excluded from history imports

Reasoning tokens are shown as a subset of output and are never added to output or total again.
