# boundline-adapter-speckit

[![Version](https://img.shields.io/github/v/release/apply-the/boundline-adapter-speckit?color=blue&label=version)](https://github.com/apply-the/boundline-adapter-speckit/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![CI](https://github.com/apply-the/boundline-adapter-speckit/actions/workflows/ci.yml/badge.svg)](https://github.com/apply-the/boundline-adapter-speckit/actions/workflows/ci.yml)
[![Lint](https://github.com/apply-the/boundline-adapter-speckit/actions/workflows/lint.yml/badge.svg)](https://github.com/apply-the/boundline-adapter-speckit/actions/workflows/lint.yml)
[![Vulnerabilities](https://github.com/apply-the/boundline-adapter-speckit/actions/workflows/vulnerabilities.yml/badge.svg)](https://github.com/apply-the/boundline-adapter-speckit/actions/workflows/vulnerabilities.yml)
[![Coverage](https://codecov.io/gh/apply-the/boundline-adapter-speckit/branch/main/graph/badge.svg)](https://codecov.io/gh/apply-the/boundline-adapter-speckit)
[![Quality Gate](https://sonarcloud.io/api/project_badges/measure?project=apply-the_boundline-adapter-speckit&metric=alert_status)](https://sonarcloud.io/summary/new_code?id=apply-the_boundline-adapter-speckit)

Known Speckit workflow bridge for Boundline.

This repository exposes the stdio contract surface for the shipped `speckit`
known profile. The bridge declares claimed `plan` and `run` stages, requires
the template and adapter repository paths during preflight, and now implements
the corrected Spec 066 stage contract.

Current bridge contract:

- the repository builds as a standalone Rust crate
- the binary exports the stable adapter ID `speckit`
- every stdio command returns the standard success envelope on stdout
- `describe` emits the known Speckit manifest with required config fields and the explicit V1 transport declaration
- `preflight` validates the required `template_repo` and `adapter_repo` values
- `preflight` trims surrounding whitespace from required path values before returning `normalized_config_values`
- `execute-stage` for `plan` must identify workflow ID `speckit-planning`, execute the full planning lifecycle (`speckit.specify`, `speckit.clarify` when required, `speckit.plan`, `speckit.tasks`, mandatory `speckit.analyze`, and bounded remediation or analyze re-checks), and return planning artifacts plus planning findings
- `execute-stage` for `run` must identify workflow ID `speckit-implementation`, invoke `speckit.implement` plus implementation validation or status capture only, and must not rerun planning commands
- a claimed `plan` attempt inherits host retry controls and may use at most one initial analyze pass plus two remediation or analyze re-check cycles before it must return `status = blocked`
- `goal`, `status`, and `inspect` remain Boundline-owned surfaces; the adapter contributes artifacts and findings, but it does not own those surfaces
- `emit-hook` returns an enveloped delivered response for subscribed hook smoke tests
- the real bridge executes `.specify/workflows/speckit/planning.yml` and `.specify/workflows/speckit/implementation.yml` by YAML path while the response payload continues to report the semantic workflow IDs

The crate now pins the released shared protocol package directly:

```toml
[dependencies]
boundline-adapters = { git = "https://github.com/apply-the/boundline", tag = "0.66.0" }
```

It also re-exports `boundline_adapters::framework_protocol` and
`boundline_adapters::framework_catalog` so downstream validation and custom
bridge code can reference the same released host-owned contract surface.

## Quality Automation

GitHub Actions mirrors the local validation scripts shipped with this adapter:

- `ci.yml` runs the baseline format, test, and clippy jobs together
- `lint.yml` exposes the dedicated format and clippy checks as a separate quality signal
- `vulnerabilities.yml` runs `cargo audit` against the workspace lockfile
- `quality.yml` runs `./scripts/coverage.sh`, uploads `lcov.info` to Codecov, and publishes the same report to SonarCloud

Rerun the same checks locally with:

```bash
cargo fmt --all --check
./scripts/test.sh
./scripts/clippy.sh
./scripts/coverage.sh
```

## Homebrew Tap

This repository now exposes the same two-step Homebrew flow that `boundline`
uses:

- `scripts/sync-distribution-metadata.sh` regenerates `distribution/homebrew/Formula/boundline-adapter-speckit.rb` from `Cargo.toml` and `distribution/channel-metadata.toml`
- `.github/workflows/sync-homebrew-tap.yml` copies that formula into the tap repository declared in `distribution/channel-metadata.toml`

Local dry run:

```bash
bash scripts/sync-distribution-metadata.sh
bash scripts/release/sync-homebrew-tap.sh \
	--formula distribution/homebrew/Formula/boundline-adapter-speckit.rb \
	--tap-root homebrew-boundline-adapter-speckit
```

The generated formula expects a real semver release tag in
`apply-the/boundline-adapter-speckit`. Until the first public tag exists, the
tap can be updated and reviewed locally, but `brew install` from the published
tap will still depend on that release tag being pushed.

## Setup Guidance

Boundline's known `speckit` profile already prefills the two required path
fields during `boundline adapter add speckit`:

- `template_repo`: defaults to `../boundline-framework-template`
- `adapter_repo`: defaults to `../boundline-adapter-speckit`

If an operator overrides either path interactively, Speckit trims surrounding
whitespace during `preflight` and returns the normalized path values in the
ready response. If either required field is still missing or blank, Speckit
stays blocked and returns the host-facing recovery command instead of trying to
prompt on its own.

Blocked preflight example:

```json
{
	"success": true,
	"data": {
		"status": "blocked",
		"reason": "missing_required_config",
		"missing_fields": ["template_repo", "adapter_repo"],
		"recovery": "boundline adapter add speckit --workspace <workspace>"
	}
}
```

## Smoke Commands

Describe the known Speckit profile:

```bash
cargo run -- describe
```

Example response:

```json
{
	"success": true,
	"data": {
		"protocol_line": "framework-adapter-v1",
		"adapter_id": "speckit",
		"adapter_version": "0.1.0",
		"supported_boundline_range": ">=0.66.0,<0.67.0",
		"supported_transports": [
			{
				"transport": "stdio",
				"encoding": "json",
				"request_channel": "stdin",
				"response_channel": "stdout"
			}
		],
		"declared_stage_overrides": ["plan", "run"],
		"declared_hook_subscriptions": ["stage_completed", "stage_failed"],
		"required_config_fields": [
			{
				"field_key": "template_repo",
				"display_label": "Template repository",
				"value_kind": "path",
				"required": true,
				"secret": false,
				"prompt_text": "Path to the reusable template repo",
				"help_text": "Point this at ../boundline-framework-template or another checked-out template repo",
				"non_interactive_policy": "fail"
			},
			{
				"field_key": "adapter_repo",
				"display_label": "Adapter repository",
				"value_kind": "path",
				"required": true,
				"secret": false,
				"prompt_text": "Path to the Speckit adapter repo",
				"help_text": "Point this at ../boundline-adapter-speckit or another checked-out Speckit repo",
				"non_interactive_policy": "fail"
			}
		]
	}
}
```

Validate the default known-profile config values:

```bash
printf '%s' '{"boundline_version":"0.66.0","workspace_ref":"../tmp/example-workspace","non_interactive":true,"config_values":[{"field_key":"template_repo","value_kind":"path","path_value":"../boundline-framework-template"},{"field_key":"adapter_repo","value_kind":"path","path_value":"../boundline-adapter-speckit"}]}' | cargo run -- preflight
```

Every `preflight`, `execute-stage`, and `emit-hook` response uses the same
stdout envelope shape, with the command-specific payload nested under
`data`.

The ready `preflight` response echoes normalized config values in a stable
order: `template_repo` first, then `adapter_repo`.

Exercise the corrected target `plan` bridge:

```bash
printf '%s' '{"run_id":"run-001","stage_key":"plan","stage_attempt":1,"workspace_ref":"../tmp/example-workspace","adapter_id":"speckit","config_values":[{"field_key":"template_repo","value_kind":"path","path_value":"../boundline-framework-template"},{"field_key":"adapter_repo","value_kind":"path","path_value":"../boundline-adapter-speckit"}],"context_artifacts":["specs/066-agentic-framework-integration/spec.md"]}' | cargo run -- execute-stage
```

The corrected `plan` bridge routes through semantic workflow ID
`speckit-planning`, launches `.specify/workflows/speckit/planning.yml` by
local YAML path, returns explicit `executed_commands`, `planning_findings`,
`analyze_pass_count`, and `remediation_cycles_used` fields, and produces at
least specification, plan, tasks, and planning workflow artifact refs.
Structured stderr remains optional and is reserved for trace enrichment rather
than control flow.

Exercise the corrected target `run` bridge:

```bash
printf '%s' '{"run_id":"run-002","stage_key":"run","stage_attempt":1,"workspace_ref":"../tmp/example-workspace","adapter_id":"speckit","config_values":[{"field_key":"template_repo","value_kind":"path","path_value":"../boundline-framework-template"},{"field_key":"adapter_repo","value_kind":"path","path_value":"../boundline-adapter-speckit"}],"context_artifacts":["specs/066-agentic-framework-integration/spec.md"]}' | cargo run -- execute-stage
```

The corrected `run` bridge routes through semantic workflow ID
`speckit-implementation`, launches
`.specify/workflows/speckit/implementation.yml` by local YAML path, calls
`speckit.implement` plus implementation validation or status capture only, and
returns explicit `executed_commands`, `implementation_status`, and
`validation_refs` fields.

## Override Fixtures

The bridge also exposes bounded fixture-mode overrides for local and host-side
tests. Create these files inside the target workspace under `.speckit/` before
invoking the binary:

- `execute-stage.failed`: `execute-stage` still returns a success envelope, but
	the nested payload reports `status = failed`, `failure_class =
	adapter_runtime`, and a retry-oriented `next_action`
- `emit-hook.failed`: `emit-hook` still returns a success envelope, but the
	nested payload reports `status = failed`

Example failure fixture flow:

```bash
mkdir -p ../tmp/example-workspace/.speckit
touch ../tmp/example-workspace/.speckit/execute-stage.failed

printf '%s' '{"run_id":"run-002","stage_key":"run","stage_attempt":1,"workspace_ref":"../tmp/example-workspace","adapter_id":"speckit","config_values":[{"field_key":"template_repo","value_kind":"path","path_value":"../boundline-framework-template"},{"field_key":"adapter_repo","value_kind":"path","path_value":"../boundline-adapter-speckit"}],"context_artifacts":[]}' | cargo run -- execute-stage
```

This keeps the default bridge path honest while still giving the sibling repo
and the Boundline host a deterministic claimed-stage failure surface to test
against.

## Compatibility Validation

The corrected 2026-06-01 workflow-bridge validation reran:

```bash
cargo test --test contract
cargo test --test override_flow
cargo test --test config_flow
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
```

Those suites stayed green and reconfirmed the corrected Speckit profile:

- the crate now pins `boundline-adapters` to the released Boundline `0.66.0`
  git tag for sibling-repo compatibility tracking
- the published dependency pin remains on `0.66.0` because no newer Boundline
  release tag exists yet; corrected Spec 066 semantics are therefore recorded
  in host and sibling `Unreleased` notes rather than through an invented git tag
- `describe` still declares adapter ID `speckit`, the released compatibility
	range, and the V1 stdio JSON transport (`stdin -> stdout`)
- `preflight`, `execute-stage`, and `emit-hook` still use the same standard
	stdout envelope the host expects from the protocol line
- missing required repo paths still block preflight explicitly, while ready
	responses still return normalized path values
- `execute-stage(plan)` now reports workflow ID `speckit-planning`, launches
	`.specify/workflows/speckit/planning.yml` by YAML path, returns real packet
	artifacts, and keeps the mandatory `speckit.analyze` readiness gate plus
	bounded remediation accounting explicit in the payload
- `execute-stage(run)` now reports workflow ID `speckit-implementation`,
	launches `.specify/workflows/speckit/implementation.yml` by YAML path,
	invokes implementation-only behavior, and does not rerun planning commands
- the legacy combined workflow asset, registry entry, and hidden run-stage
	fallback were retired before the final rerun, so the split planning and
	implementation entrypoints are now the only live bundled Speckit workflow
	surfaces
- the release line still treats structured stderr as optional trace-only
	enrichment rather than a separate control channel
- the validated command surface is one-shot only; there is still no graceful-
	shutdown or resident adapter lifecycle in the released slice
- the Boundline provider-catalog refresh remained a no-change result for this
	release, so no Speckit-specific catalog delta was required beyond the host
	catalog update already landed on 2026-05-30
- the Boundline packet's read-only `/speckit.analyze` rerun passed for the
	corrected 066 stage-map slice, so the remaining work is release-packaging
	follow-through rather than semantic repair
