# Trusted-host automation

spark automation --stdio runs one non-interactive Spark harness request for a
trusted host process. It reads one versioned JSON object from standard input,
writes one JSON object to standard output, and sends progress or diagnostics
only to standard error.

The host supplies the workspace, prompt, model, reasoning effort, JSON output
schema, read-only roots, and any per-run HTTP MCP brokers. Spark does not load
ambient MCP configuration for this command.

## Request

~~~json
{
  "schema_version": "spark.automation.v1",
  "request_id": "job-42",
  "cwd": "C:\\workspaces\\job-42",
  "prompt": "Inspect the request and return the required artifact.",
  "model": "gpt-5.3-codex-spark",
  "reasoning_effort": "medium",
  "output_schema_name": "diffuin_artifact",
  "output_schema": {
    "type": "object",
    "additionalProperties": false,
    "properties": {
      "summary": { "type": "string" }
    },
    "required": ["summary"]
  },
  "read_roots": [
    "C:\\references\\schedule-one"
  ],
  "tool_policy": {
    "workspace_writes": false,
    "allow_unsandboxed_commands": false
  },
  "mcp_servers": [
    {
      "name": "diffuin_github",
      "url": "http://127.0.0.1:43120/mcp",
      "bearer_token_env_var": "DIFFUIN_GITHUB_READ_TOKEN"
    }
  ]
}
~~~

minimal, low, medium, high, xhigh, and max are accepted protocol values for
reasoning_effort. The selected model still determines which values the provider
supports.

Bearer tokens are read from the named environment variables by the MCP
transport. Token values are not placed in the request, prompt, tool
descriptions, or structured response.
The MCP URL and environment-variable binding are trusted host configuration;
do not derive them from model output or untrusted job content.

## Response

On success, Spark returns:

~~~json
{
  "schema_version": "spark.automation.v1",
  "request_id": "job-42",
  "status": "completed",
  "final_response": "{\"summary\":\"...\"}",
  "tool_policy": {
    "workspace_writes": false,
    "allow_unsandboxed_commands": false
  },
  "warnings": []
}
~~~

final_response is the provider-constrained JSON artifact serialized as a string
so a host can preserve its existing validation boundary.

On failure, Spark exits nonzero and still writes a machine-readable envelope:

~~~json
{
  "schema_version": "spark.automation.v1",
  "request_id": "job-42",
  "status": "failed",
  "error": "MCP server `diffuin_github` is unavailable"
}
~~~

request_id is null when the request could not be parsed. Hosts should bound and
capture standard error for diagnostics, but use the standard-output envelope as
the protocol result.

## Security boundary

The default policy exposes bounded workspace and read-root filesystem reads.
workspace_writes adds Spark's native workspace-confined file mutation tools; it
never grants writes to read_roots.

Spark's cmd.exec tool is not an OS sandbox. It can leave the workspace and
access anything available to the Spark process. It remains disabled unless the
host explicitly sets allow_unsandboxed_commands to true, and Spark reports a
warning in the response when enabled. A production host should enable it only
inside a separately enforced per-job container or equivalent OS sandbox.

Public web search, browser automation, GitHub CLI access, subagents, ambient
MCP servers, traces, memory, and persistent sessions are disabled for
automation runs. Only MCP servers named in the request are discovered, and a
failure to initialize any supplied server fails the run.
