# Changelog

All notable changes to boundline-adapter-speckit are documented in this file.

Boundline follows Semantic Versioning. Before `1.0.0`, breaking changes may
occur in minor releases.

## [Unreleased]

- Replaced the placeholder claimed-stage marker path with the corrected Spec
  Kit workflow bridge: `plan` now executes the split planning workflow asset,
  finishes with the mandatory `speckit.analyze` readiness gate, and returns
  planning artifacts plus findings and remediation counters.
- Corrected `run` so it now executes the split implementation workflow asset,
  returns workflow ID `speckit-implementation`, reports implementation
  validation refs, and no longer reruns planning commands.
- Pinned `boundline-adapters` to the released Boundline `0.66.0` git tag for
  sibling-repo compatibility tracking.
- Re-exported the shared framework-adapter catalog and protocol modules so the
  workflow bridge and its tests can reference the released host-owned contract
  surface directly.
- Kept structured stderr optional and trace-only, and kept the V1 bridge
  bounded to one-shot stdio execution with no graceful shutdown lifecycle.

## [0.1.0] - 2026-05-31

- Bootstrapped the known `speckit` adapter scaffold with the released
  `framework-adapter-v1` line, explicit required path fields, and stable
  `plan` plus `run` stage claims.
- Added blocked-preflight and normalized-config handling for the host-guided
  setup flow, including deterministic non-interactive failure behavior.
- Documented and validated the declared V1 stdio JSON transport, standard
  stdout envelope, optional stderr trace-only policy, and explicit one-shot
  no-graceful-shutdown scope.