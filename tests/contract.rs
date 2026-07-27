use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use boundline_adapter_speckit::{
    ADAPTER_ID, ADAPTER_REPO_FIELD_KEY, BINARY_NAME, BOOTSTRAP_STATUS, CONTRACT_LINE, ConfigValue,
    ConfigValueKind, EmitHookRequest, ExecuteStageRequest, PreflightRequest,
    SpeckitBootstrapMetadata, TEMPLATE_REPO_FIELD_KEY, bootstrap_metadata, bootstrap_status_line,
};
use serde_json::{Value, json};

const TEST_BOUNDLINE_VERSION: &str = "0.90.0";
const TEST_WORKSPACE_REF: &str = "../tmp/example-workspace";
const TEST_TRACE_PAYLOAD_REF: &str = ".boundline/traces/run.json";
const TEST_SPEC_ARTIFACT_REF: &str = "specs/066-agentic-framework-integration/spec.md";
const PLAN_STAGE_KEY: &str = "plan";
const RUN_STAGE_KEY: &str = "run";
const STAGE_COMPLETED_HOOK_KEY: &str = "stage_completed";
const READY_PREFLIGHT_STATUS: &str = "ready";
const SUCCEEDED_STAGE_STATUS: &str = "succeeded";
const BLOCKED_STAGE_STATUS: &str = "blocked";
const DELIVERED_HOOK_STATUS: &str = "delivered";

const EXPECTED_TRANSPORT: &str = "stdio";
const EXPECTED_ENCODING: &str = "json";
const EXPECTED_REQUEST_CHANNEL: &str = "stdin";
const EXPECTED_RESPONSE_CHANNEL: &str = "stdout";
const SPEC_ARTIFACT_REF: &str = "specs/066-agentic-framework-integration/spec.md";
const PLAN_ARTIFACT_REF: &str = "specs/066-agentic-framework-integration/plan.md";
const TASKS_ARTIFACT_REF: &str = "specs/066-agentic-framework-integration/tasks.md";
const PLANNING_WORKFLOW_ARTIFACT_REF: &str = ".specify/workflows/speckit/planning.yml";
const IMPLEMENTATION_WORKFLOW_ARTIFACT_REF: &str = ".specify/workflows/speckit/implementation.yml";
const IMPLEMENT_PROMPT_REF: &str = ".github/prompts/speckit.implement.prompt.md";
const INVALID_JSON_INPUT: &str = "{";
const PARSE_JSON_ERROR_PREFIX: &str = "failed to parse stdin JSON";
const SPECIFY_BIN_ENV_VAR: &str = "BOUNDLINE_SPECKIT_SPECIFY_BIN";
const PLANNING_WORKFLOW_ID: &str = "speckit-planning";
const IMPLEMENTATION_WORKFLOW_ID: &str = "speckit-implementation";
const FEATURE_DIR_REF: &str = "specs/066-agentic-framework-integration";
const FEATURE_SPEC_CONTENT: &str = "# Agentic Framework Integration\n";
const FEATURE_PLAN_CONTENT: &str = "# Plan\n";
const FEATURE_TASKS_CONTENT: &str = "# Tasks\n";
const IMPLEMENT_PROMPT_CONTENT: &str = "# Speckit Implement\n";
const PLANNING_WORKFLOW_ASSET_CONTENT: &str = concat!(
    "id: \"speckit-planning\"\n",
    "steps:\n",
    "  - command: speckit.specify\n",
    "  - command: speckit.plan\n",
    "  - command: speckit.tasks\n",
);
const IMPLEMENTATION_WORKFLOW_ASSET_CONTENT: &str = concat!(
    "id: \"speckit-implementation\"\n",
    "steps:\n",
    "  - command: speckit.implement\n",
);

#[test]
fn bootstrap_metadata_matches_known_profile() -> Result<(), String> {
    let metadata = bootstrap_metadata();

    if metadata.adapter_id != ADAPTER_ID {
        return Err(format!(
            "expected adapter id {ADAPTER_ID}, got {}",
            metadata.adapter_id
        ));
    }

    if metadata.binary_name != BINARY_NAME {
        return Err(format!(
            "expected binary name {BINARY_NAME}, got {}",
            metadata.binary_name
        ));
    }

    if metadata.contract_line != CONTRACT_LINE {
        return Err(format!(
            "expected contract line {CONTRACT_LINE}, got {}",
            metadata.contract_line
        ));
    }

    if metadata.bootstrap_status != BOOTSTRAP_STATUS {
        return Err(format!(
            "expected bootstrap status {BOOTSTRAP_STATUS}, got {}",
            metadata.bootstrap_status
        ));
    }

    Ok(())
}

#[test]
fn bootstrap_binary_message_is_operator_readable() -> Result<(), String> {
    let line = bootstrap_status_line();

    if !line.contains(BINARY_NAME) {
        return Err(format!("status line missing binary name: {line}"));
    }

    if !line.contains(CONTRACT_LINE) {
        return Err(format!("status line missing contract line: {line}"));
    }

    if !line.contains(BOOTSTRAP_STATUS) {
        return Err(format!("status line missing bootstrap marker: {line}"));
    }

    Ok(())
}

#[test]
fn describe_command_emits_known_speckit_profile_manifest() -> Result<(), String> {
    let output = Command::new(env!("CARGO_BIN_EXE_boundline-adapter-speckit"))
        .arg("describe")
        .output()
        .map_err(|error| format!("failed to run describe command: {error}"))?;

    if !output.status.success() {
        return Err(format!(
            "describe command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let document: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("describe stdout was not valid JSON: {error}"))?;

    expect_bool(&document, "/success", true)?;
    expect_string(&document, "/data/protocol_line", CONTRACT_LINE)?;
    expect_string(&document, "/data/adapter_id", ADAPTER_ID)?;
    expect_string(
        &document,
        "/data/supported_transports/0/transport",
        EXPECTED_TRANSPORT,
    )?;
    expect_string(
        &document,
        "/data/supported_transports/0/encoding",
        EXPECTED_ENCODING,
    )?;
    expect_string(
        &document,
        "/data/supported_transports/0/request_channel",
        EXPECTED_REQUEST_CHANNEL,
    )?;
    expect_string(
        &document,
        "/data/supported_transports/0/response_channel",
        EXPECTED_RESPONSE_CHANNEL,
    )?;
    expect_string(
        &document,
        "/data/declared_stage_overrides/0",
        PLAN_STAGE_KEY,
    )?;
    expect_string(&document, "/data/declared_stage_overrides/1", RUN_STAGE_KEY)?;
    expect_string(
        &document,
        "/data/required_config_fields/0/field_key",
        TEMPLATE_REPO_FIELD_KEY,
    )?;
    expect_string(
        &document,
        "/data/required_config_fields/1/field_key",
        ADAPTER_REPO_FIELD_KEY,
    )?;

    Ok(())
}

#[test]
fn preflight_command_accepts_known_profile_defaults_and_returns_ready() -> Result<(), String> {
    let request = serde_json::to_string(&PreflightRequest {
        boundline_version: TEST_BOUNDLINE_VERSION.to_string(),
        workspace_ref: TEST_WORKSPACE_REF.to_string(),
        non_interactive: true,
        config_values: vec![
            path_value(TEMPLATE_REPO_FIELD_KEY, "../boundline-framework-template"),
            path_value(ADAPTER_REPO_FIELD_KEY, "../boundline-adapter-speckit"),
        ],
    })
    .map_err(|error| format!("failed to encode preflight request: {error}"))?;

    let output = run_with_stdin(&["preflight"], &request)?;
    let document: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("preflight stdout was not valid JSON: {error}"))?;

    expect_bool(&document, "/success", true)?;
    expect_string(&document, "/data/status", READY_PREFLIGHT_STATUS)?;
    expect_array_length(&document, "/data/normalized_config_values", 2)?;

    Ok(())
}

#[test]
fn execute_stage_command_returns_real_plan_artifacts() -> Result<(), String> {
    let workspace = temp_specify_workspace("speckit-contract-plan-stage")?;
    let fake_specify =
        write_fake_specify_script(&workspace, "#!/bin/sh\nprintf 'Status: completed\\n'\n")?;
    let request = serde_json::to_string(&ExecuteStageRequest {
        run_id: "run-001".to_string(),
        stage_key: PLAN_STAGE_KEY.to_string(),
        stage_attempt: 1,
        workspace_ref: workspace.to_string_lossy().into_owned(),
        adapter_id: ADAPTER_ID.to_string(),
        config_values: vec![
            path_value(TEMPLATE_REPO_FIELD_KEY, "../boundline-framework-template"),
            path_value(ADAPTER_REPO_FIELD_KEY, "../boundline-adapter-speckit"),
        ],
        context_artifacts: vec![TEST_SPEC_ARTIFACT_REF.to_string()],
    })
    .map_err(|error| format!("failed to encode execute-stage request: {error}"))?;

    let output = run_with_stdin_and_env(
        &["execute-stage"],
        &request,
        &[(SPECIFY_BIN_ENV_VAR, fake_specify.to_string_lossy().as_ref())],
    )?;
    let document: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("execute-stage stdout was not valid JSON: {error}"))?;

    expect_bool(&document, "/success", true)?;
    expect_string(&document, "/data/status", SUCCEEDED_STAGE_STATUS)?;
    expect_string(&document, "/data/workflow_id", PLANNING_WORKFLOW_ID)?;
    expect_string(&document, "/data/produced_artifacts/0", SPEC_ARTIFACT_REF)?;
    expect_string(&document, "/data/produced_artifacts/1", PLAN_ARTIFACT_REF)?;
    expect_string(&document, "/data/produced_artifacts/2", TASKS_ARTIFACT_REF)?;
    expect_string(
        &document,
        "/data/produced_artifacts/3",
        PLANNING_WORKFLOW_ARTIFACT_REF,
    )?;
    let marker_path = workspace.join("speckit-plan-claimed.txt");
    if marker_path.exists() {
        return Err(format!(
            "unexpected placeholder marker created at {}",
            marker_path.display()
        ));
    }

    Ok(())
}

#[test]
fn execute_stage_command_returns_blocked_run_recovery_for_real_workflow() -> Result<(), String> {
    let workspace = temp_specify_workspace("speckit-contract-run-stage")?;
    let fake_specify = write_fake_specify_script(
        &workspace,
        "#!/bin/sh\nprintf 'Status: paused\\n'\nprintf 'Resume with: specify workflow resume run-123\\n'\n",
    )?;
    let request = serde_json::to_string(&ExecuteStageRequest {
        run_id: "run-002".to_string(),
        stage_key: RUN_STAGE_KEY.to_string(),
        stage_attempt: 1,
        workspace_ref: workspace.to_string_lossy().into_owned(),
        adapter_id: ADAPTER_ID.to_string(),
        config_values: vec![
            path_value(TEMPLATE_REPO_FIELD_KEY, "../boundline-framework-template"),
            path_value(ADAPTER_REPO_FIELD_KEY, "../boundline-adapter-speckit"),
        ],
        context_artifacts: vec![TEST_SPEC_ARTIFACT_REF.to_string()],
    })
    .map_err(|error| format!("failed to encode execute-stage request: {error}"))?;

    let output = run_with_stdin_and_env(
        &["execute-stage"],
        &request,
        &[(SPECIFY_BIN_ENV_VAR, fake_specify.to_string_lossy().as_ref())],
    )?;
    let document: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("execute-stage stdout was not valid JSON: {error}"))?;

    expect_bool(&document, "/success", true)?;
    expect_string(&document, "/data/status", BLOCKED_STAGE_STATUS)?;
    expect_string(&document, "/data/workflow_id", IMPLEMENTATION_WORKFLOW_ID)?;
    expect_string(&document, "/data/produced_artifacts/0", PLAN_ARTIFACT_REF)?;
    expect_string(&document, "/data/produced_artifacts/1", TASKS_ARTIFACT_REF)?;
    expect_string(
        &document,
        "/data/produced_artifacts/2",
        IMPLEMENTATION_WORKFLOW_ARTIFACT_REF,
    )?;
    expect_string(
        &document,
        "/data/produced_artifacts/3",
        IMPLEMENT_PROMPT_REF,
    )?;
    let next_action = document
        .pointer("/data/next_action")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing next_action for blocked workflow run".to_string())?;
    if !next_action.contains("specify workflow resume") {
        return Err(format!(
            "expected next_action to resume the paused workflow, got {next_action}"
        ));
    }

    Ok(())
}

#[test]
fn split_workflow_assets_publish_correct_ids_and_command_surfaces() -> Result<(), String> {
    let workspace = temp_specify_workspace("speckit-contract-workflow-assets")?;
    let planning_asset = fs::read_to_string(workspace.join(PLANNING_WORKFLOW_ARTIFACT_REF))
        .map_err(|error| format!("failed to read planning workflow asset: {error}"))?;
    let implementation_asset =
        fs::read_to_string(workspace.join(IMPLEMENTATION_WORKFLOW_ARTIFACT_REF))
            .map_err(|error| format!("failed to read implementation workflow asset: {error}"))?;

    assert_contains(&planning_asset, "id: \"speckit-planning\"")?;
    assert_contains(&planning_asset, "command: speckit.specify")?;
    assert_contains(&planning_asset, "command: speckit.plan")?;
    assert_contains(&planning_asset, "command: speckit.tasks")?;
    assert_ordered(
        &planning_asset,
        &[
            "command: speckit.specify",
            "command: speckit.plan",
            "command: speckit.tasks",
        ],
    )?;

    assert_contains(&implementation_asset, "id: \"speckit-implementation\"")?;
    assert_contains(&implementation_asset, "command: speckit.implement")?;
    assert_not_contains(&implementation_asset, "command: speckit.plan")?;
    assert_not_contains(&implementation_asset, "command: speckit.tasks")?;
    assert_not_contains(&implementation_asset, "command: speckit.analyze")?;

    Ok(())
}

#[test]
fn emit_hook_command_returns_enveloped_delivery_payload() -> Result<(), String> {
    let request = serde_json::to_string(&EmitHookRequest {
        run_id: "run-001".to_string(),
        hook_key: STAGE_COMPLETED_HOOK_KEY.to_string(),
        stage_key: PLAN_STAGE_KEY.to_string(),
        stage_claimed: true,
        workspace_ref: TEST_WORKSPACE_REF.to_string(),
        payload_ref: TEST_TRACE_PAYLOAD_REF.to_string(),
    })
    .map_err(|error| format!("failed to encode emit-hook request: {error}"))?;

    let output = run_with_stdin(&["emit-hook"], &request)?;
    let document: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("emit-hook stdout was not valid JSON: {error}"))?;

    expect_bool(&document, "/success", true)?;
    expect_string(&document, "/data/status", DELIVERED_HOOK_STATUS)?;

    Ok(())
}

#[test]
fn preflight_command_accepts_detected_sibling_repo_paths() -> Result<(), String> {
    let template_repo = sibling_repo_path("boundline-framework-template")?;
    let adapter_repo = sibling_repo_path("boundline-adapter-speckit")?;
    let request = serde_json::to_string(&PreflightRequest {
        boundline_version: TEST_BOUNDLINE_VERSION.to_string(),
        workspace_ref: TEST_WORKSPACE_REF.to_string(),
        non_interactive: true,
        config_values: vec![
            path_value(TEMPLATE_REPO_FIELD_KEY, &template_repo.to_string_lossy()),
            path_value(ADAPTER_REPO_FIELD_KEY, &adapter_repo.to_string_lossy()),
        ],
    })
    .map_err(|error| format!("failed to encode preflight request: {error}"))?;

    let output = run_with_stdin(&["preflight"], &request)?;
    let document: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("preflight stdout was not valid JSON: {error}"))?;

    expect_bool(&document, "/success", true)?;
    expect_string(&document, "/data/status", READY_PREFLIGHT_STATUS)?;
    expect_array_length(&document, "/data/normalized_config_values", 2)?;

    Ok(())
}

#[test]
fn coverage_helpers_cover_bootstrap_cli_and_default_metadata() -> Result<(), String> {
    assert_eq!(SpeckitBootstrapMetadata::default(), bootstrap_metadata());

    let bootstrap_output = Command::new(env!("CARGO_BIN_EXE_boundline-adapter-speckit"))
        .output()
        .map_err(|error| format!("failed to run bootstrap command: {error}"))?;
    if !bootstrap_output.status.success() {
        return Err(format!(
            "bootstrap command failed: {}",
            String::from_utf8_lossy(&bootstrap_output.stderr)
        ));
    }
    let bootstrap_stdout = String::from_utf8(bootstrap_output.stdout)
        .map_err(|error| format!("bootstrap stdout was not valid UTF-8: {error}"))?;
    if bootstrap_stdout.trim_end() != bootstrap_status_line() {
        return Err(format!(
            "expected bootstrap stdout `{}` to equal `{}`",
            bootstrap_stdout.trim_end(),
            bootstrap_status_line()
        ));
    }

    let parse_error_output = run_raw_with_stdin(&["preflight"], INVALID_JSON_INPUT)?;
    if parse_error_output.status.success() {
        return Err("expected invalid preflight JSON to exit with a non-zero status".to_string());
    }
    let parse_error_stderr = String::from_utf8(parse_error_output.stderr)
        .map_err(|error| format!("stderr was not valid UTF-8: {error}"))?;
    if !parse_error_stderr.contains(PARSE_JSON_ERROR_PREFIX) {
        return Err(format!(
            "expected stderr to mention `{PARSE_JSON_ERROR_PREFIX}`, got {parse_error_stderr}"
        ));
    }

    Ok(())
}

fn run_raw_with_stdin(args: &[&str], input: &str) -> Result<std::process::Output, String> {
    run_raw_with_stdin_and_env(args, input, &[])
}

fn run_raw_with_stdin_and_env(
    args: &[&str],
    input: &str,
    envs: &[(&str, &str)],
) -> Result<std::process::Output, String> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_boundline-adapter-speckit"))
        .args(args)
        .envs(envs.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("failed to spawn command: {error}"))?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "command did not expose stdin".to_string())?;
    stdin
        .write_all(input.as_bytes())
        .map_err(|error| format!("failed to write command stdin: {error}"))?;
    drop(stdin);

    child
        .wait_with_output()
        .map_err(|error| format!("failed to read command output: {error}"))
}

fn run_with_stdin(args: &[&str], input: &str) -> Result<std::process::Output, String> {
    run_with_stdin_and_env(args, input, &[])
}

fn run_with_stdin_and_env(
    args: &[&str],
    input: &str,
    envs: &[(&str, &str)],
) -> Result<std::process::Output, String> {
    let output = run_raw_with_stdin_and_env(args, input, envs)?;
    if !output.status.success() {
        return Err(format!(
            "command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(output)
}

fn write_fake_specify_script(
    workspace_path: &std::path::Path,
    contents: &str,
) -> Result<PathBuf, String> {
    let path = workspace_path.join("fake-specify.sh");
    write_executable(&path, contents)?;
    Ok(path)
}

fn write_executable(path: &std::path::Path, contents: &str) -> Result<(), String> {
    write_file(path, contents)?;
    let mut permissions = fs::metadata(path)
        .map_err(|error| format!("failed to read metadata {}: {error}", path.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .map_err(|error| format!("failed to set permissions {}: {error}", path.display()))
}

fn write_file(path: &std::path::Path, contents: &str) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Err(format!("path {} had no parent directory", path.display()));
    };
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create parent {}: {error}", parent.display()))?;
    fs::write(path, contents)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn assert_contains(contents: &str, needle: &str) -> Result<(), String> {
    if contents.contains(needle) {
        return Ok(());
    }
    Err(format!("expected workflow asset to contain `{needle}`"))
}

fn assert_not_contains(contents: &str, needle: &str) -> Result<(), String> {
    if !contents.contains(needle) {
        return Ok(());
    }
    Err(format!("expected workflow asset to omit `{needle}`"))
}

fn assert_ordered(contents: &str, needles: &[&str]) -> Result<(), String> {
    let mut last_index = 0;
    for needle in needles {
        let index = contents[last_index..]
            .find(needle)
            .ok_or_else(|| format!("expected workflow asset to contain `{needle}` in order"))?;
        last_index += index + needle.len();
    }
    Ok(())
}

fn path_value(field_key: &str, path_value: &str) -> ConfigValue {
    ConfigValue {
        field_key: field_key.to_string(),
        value_kind: ConfigValueKind::Path,
        string_value: None,
        path_value: Some(path_value.to_string()),
        bool_value: None,
        int_value: None,
    }
}

fn temp_workspace(prefix: &str) -> Result<PathBuf, String> {
    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let workspace = std::env::temp_dir().join(format!("{prefix}-{unique_suffix}"));
    fs::create_dir_all(&workspace).map_err(|error| {
        format!(
            "failed to create temp workspace {}: {error}",
            workspace.display()
        )
    })?;
    Ok(workspace)
}

fn sibling_repo_path(repo_name: &str) -> Result<PathBuf, String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let sibling_parent = manifest_dir.parent().ok_or_else(|| {
        format!(
            "repository root {} has no parent directory",
            manifest_dir.display()
        )
    })?;
    Ok(sibling_parent.join(repo_name))
}

fn temp_specify_workspace(prefix: &str) -> Result<PathBuf, String> {
    let workspace = temp_workspace(prefix)?;
    seed_specify_workspace(&workspace)?;
    Ok(workspace)
}

fn seed_specify_workspace(workspace: &std::path::Path) -> Result<(), String> {
    write_file(&workspace.join(SPEC_ARTIFACT_REF), FEATURE_SPEC_CONTENT)?;
    write_file(&workspace.join(PLAN_ARTIFACT_REF), FEATURE_PLAN_CONTENT)?;
    write_file(&workspace.join(TASKS_ARTIFACT_REF), FEATURE_TASKS_CONTENT)?;
    write_file(
        &workspace.join(PLANNING_WORKFLOW_ARTIFACT_REF),
        PLANNING_WORKFLOW_ASSET_CONTENT,
    )?;
    write_file(
        &workspace.join(IMPLEMENTATION_WORKFLOW_ARTIFACT_REF),
        IMPLEMENTATION_WORKFLOW_ASSET_CONTENT,
    )?;
    write_file(
        &workspace.join(IMPLEMENT_PROMPT_REF),
        IMPLEMENT_PROMPT_CONTENT,
    )?;
    write_executable(
        &workspace.join(".specify/scripts/bash/setup-plan.sh"),
        &setup_plan_script(workspace)?,
    )?;
    write_executable(
        &workspace.join(".specify/scripts/bash/check-prerequisites.sh"),
        &check_prerequisites_script(workspace)?,
    )?;
    Ok(())
}

fn setup_plan_script(workspace: &std::path::Path) -> Result<String, String> {
    let payload = serde_json::to_string(&json!({
        "IMPL_PLAN": workspace.join(PLAN_ARTIFACT_REF).to_string_lossy().into_owned(),
    }))
    .map_err(|error| format!("failed to encode setup-plan fixture JSON: {error}"))?;
    Ok(format!("#!/bin/sh\nprintf '%s' '{payload}'\n"))
}

fn check_prerequisites_script(workspace: &std::path::Path) -> Result<String, String> {
    let payload = serde_json::to_string(&json!({
        "FEATURE_DIR": workspace.join(FEATURE_DIR_REF).to_string_lossy().into_owned(),
        "AVAILABLE_DOCS": ["tasks.md"],
    }))
    .map_err(|error| format!("failed to encode prerequisites fixture JSON: {error}"))?;
    Ok(format!("#!/bin/sh\nprintf '%s' '{payload}'\n"))
}

fn expect_string(document: &Value, pointer: &str, expected: &str) -> Result<(), String> {
    let actual = document
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing string at {pointer}"))?;

    if actual != expected {
        return Err(format!("expected {pointer} to be {expected}, got {actual}"));
    }

    Ok(())
}

fn expect_bool(document: &Value, pointer: &str, expected: bool) -> Result<(), String> {
    let actual = document
        .pointer(pointer)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("missing bool at {pointer}"))?;

    if actual != expected {
        return Err(format!("expected {pointer} to be {expected}, got {actual}"));
    }

    Ok(())
}

fn expect_array_length(document: &Value, pointer: &str, expected: usize) -> Result<(), String> {
    let array = document
        .pointer(pointer)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("missing array at {pointer}"))?;

    if array.len() != expected {
        return Err(format!(
            "expected array length {expected} at {pointer}, got {}",
            array.len()
        ));
    }

    Ok(())
}
