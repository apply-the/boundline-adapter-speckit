use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use boundline_adapter_speckit::{
    ADAPTER_ID, EmitHookRequest, ExecuteStageRequest, PreflightRequest, SpeckitProfileError,
    execute_stage_response,
};
use serde::Serialize;
use serde_json::Value;

const TEST_BOUNDLINE_VERSION: &str = "0.66.0";
const TEST_WORKSPACE_REF: &str = "../tmp/example-workspace";
const TEST_HOOK_PAYLOAD_REF: &str = ".boundline/traces/run.json";
const PLAN_STAGE_KEY: &str = "plan";
const RUN_STAGE_KEY: &str = "run";
const STAGE_FAILED_HOOK_KEY: &str = "stage_failed";
const SUCCEEDED_STAGE_STATUS: &str = "succeeded";
const BLOCKED_STAGE_STATUS: &str = "blocked";
const FAILED_STAGE_STATUS: &str = "failed";
const SPECIFY_WORKFLOW_RESUME_COMMAND: &str = "specify workflow resume";
const TEMPLATE_REPO_FIELD_KEY: &str = "template_repo";
const ADAPTER_REPO_FIELD_KEY: &str = "adapter_repo";

const EXECUTE_STAGE_FAILED_FIXTURE_REF: &str = ".speckit/execute-stage.failed";
const EMIT_HOOK_FAILED_FIXTURE_REF: &str = ".speckit/emit-hook.failed";
const FAILURE_CLASS_ADAPTER_RUNTIME: &str = "adapter_runtime";
const TEMPLATE_REPO_PATH: &str = "../boundline-framework-template";
const ADAPTER_REPO_PATH: &str = "../boundline-adapter-speckit";
const SPEC_ARTIFACT_REF: &str = "specs/066-agentic-framework-integration/spec.md";
const PLAN_ARTIFACT_REF: &str = "specs/066-agentic-framework-integration/plan.md";
const TASKS_ARTIFACT_REF: &str = "specs/066-agentic-framework-integration/tasks.md";
const PLANNING_WORKFLOW_ARTIFACT_REF: &str = ".specify/workflows/speckit/planning.yml";
const IMPLEMENTATION_WORKFLOW_ARTIFACT_REF: &str = ".specify/workflows/speckit/implementation.yml";
const IMPLEMENT_PROMPT_REF: &str = ".github/prompts/speckit.implement.prompt.md";
const UNKNOWN_STAGE_KEY: &str = "unknown";
const TEST_FAILURE_LINE: &str = "simulated command failure";
const SPECIFY_BIN_ENV_VAR: &str = "BOUNDLINE_SPECKIT_SPECIFY_BIN";
const TEST_FEATURE_DIR_NAME: &str = "099-coverage-feature";
const TEST_WORKFLOW_RUN_ID: &str = "run-123";
const FEATURE_DIR_RERUN_SUFFIX: &str = "spec=099-coverage-feature";
const PLANNING_WORKFLOW_ID: &str = "speckit-planning";
const IMPLEMENTATION_WORKFLOW_ID: &str = "speckit-implementation";
const PLANNING_READINESS_FIXTURE_REF: &str = ".speckit/planning-readiness.json";
const ANALYZE_COMMAND_REF: &str = "speckit.analyze";
const REAL_FEATURE_DIR_REF: &str = "specs/066-agentic-framework-integration";
const REAL_FEATURE_SPEC_CONTENT: &str = "# Agentic Framework Integration\n";
const REAL_FEATURE_PLAN_CONTENT: &str = "# Plan\n";
const REAL_FEATURE_TASKS_CONTENT: &str = "# Tasks\n";
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

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct PlanningReadinessJson<'a> {
    analyze_passes: Vec<AnalyzePassJson<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct AnalyzePassJson<'a> {
    findings: Vec<PlanningFindingJson<'a>>,
    remediation_tasks: Vec<PlanningRemediationTaskJson<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct PlanningFindingJson<'a> {
    finding_id: &'a str,
    summary: &'a str,
    severity: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct PlanningRemediationTaskJson<'a> {
    task_id: &'a str,
    summary: &'a str,
    finding_ids: Vec<&'a str>,
    in_scope: bool,
    safe: bool,
    deterministic: bool,
    requires_operator_input: bool,
    command: Option<PlanningRemediationCommandJson<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct PlanningRemediationCommandJson<'a> {
    program: &'a str,
    args: Vec<&'a str>,
}

#[derive(Serialize)]
struct SetupPlanJson<'a> {
    #[serde(rename = "IMPL_PLAN")]
    impl_plan: &'a str,
}

#[derive(Serialize)]
struct CheckPrerequisitesJson<'a> {
    #[serde(rename = "FEATURE_DIR")]
    feature_dir: &'a str,
    #[serde(rename = "AVAILABLE_DOCS")]
    available_docs: Vec<&'a str>,
}

#[test]
fn plan_stage_claim_returns_real_specify_artifacts_without_placeholder_markers()
-> Result<(), String> {
    let workspace = temp_specify_workspace("speckit-override-plan-success")?;
    let fake_specify =
        write_fake_specify_script(&workspace, "#!/bin/sh\nprintf 'Status: completed\\n'\n")?;
    let request = serde_json::to_string(&ExecuteStageRequest {
        run_id: "plan-success-001".to_string(),
        stage_key: PLAN_STAGE_KEY.to_string(),
        stage_attempt: 1,
        workspace_ref: workspace.to_string_lossy().into_owned(),
        adapter_id: ADAPTER_ID.to_string(),
        config_values: config_values()?,
        context_artifacts: Vec::new(),
    })
    .map_err(|error| format!("failed to encode execute-stage request: {error}"))?;

    let output = run_raw_with_stdin_and_env(
        &["execute-stage"],
        &request,
        &[(SPECIFY_BIN_ENV_VAR, fake_specify.to_string_lossy().as_ref())],
    )?;
    if !output.status.success() {
        return Err(format!(
            "execute-stage command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let document: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("execute-stage stdout was not valid JSON: {error}"))?;

    expect_bool(&document, "/success", true)?;
    expect_string(&document, "/data/status", SUCCEEDED_STAGE_STATUS)?;
    expect_string(&document, "/data/workflow_id", PLANNING_WORKFLOW_ID)?;
    expect_string(&document, "/data/final_planning_readiness_status", "ready")?;
    expect_string(&document, "/data/produced_artifacts/0", SPEC_ARTIFACT_REF)?;
    expect_string(&document, "/data/produced_artifacts/1", PLAN_ARTIFACT_REF)?;
    expect_string(&document, "/data/produced_artifacts/2", TASKS_ARTIFACT_REF)?;
    expect_string(
        &document,
        "/data/produced_artifacts/3",
        PLANNING_WORKFLOW_ARTIFACT_REF,
    )?;
    let executed_commands = document
        .pointer("/data/executed_commands")
        .and_then(Value::as_array)
        .ok_or_else(|| "missing executed_commands array".to_string())?;
    if !executed_commands.iter().any(|value| {
        value
            .as_str()
            .is_some_and(|command| command.contains(PLANNING_WORKFLOW_ARTIFACT_REF))
    }) {
        return Err("expected executed commands to include the planning workflow path".to_string());
    }
    if !executed_commands
        .iter()
        .any(|value| value.as_str() == Some(ANALYZE_COMMAND_REF))
    {
        return Err("expected executed commands to include speckit.analyze".to_string());
    }
    let placeholder_marker = workspace.join("speckit-plan-claimed.txt");
    if placeholder_marker.exists() {
        return Err(format!(
            "unexpected placeholder marker created at {}",
            placeholder_marker.display()
        ));
    }

    Ok(())
}

#[test]
fn plan_stage_attempts_all_actionable_blocking_remediations_without_top_n_cap() -> Result<(), String>
{
    let workspace = temp_workspace("speckit-plan-remediation-all")?;
    let (feature_dir, plan_path) = create_feature_packet(&workspace)?;
    write_plan_stage_scripts(
        &workspace,
        &json_script(&setup_plan_json(&plan_path)?),
        &json_script(&check_prerequisites_json(&feature_dir)?),
    )?;
    let remediation_root = workspace.join("remediation");
    let first_ref = write_remediation_script(&workspace, "first.sh", "first.txt")?;
    let second_ref = write_remediation_script(&workspace, "second.sh", "second.txt")?;
    let third_ref = write_remediation_script(&workspace, "third.sh", "third.txt")?;
    write_planning_readiness_fixture(
        &workspace,
        &PlanningReadinessJson {
            analyze_passes: vec![
                AnalyzePassJson {
                    findings: vec![
                        blocking_finding("f-1", "First blocker"),
                        blocking_finding("f-2", "Second blocker"),
                        blocking_finding("f-3", "Third blocker"),
                    ],
                    remediation_tasks: vec![
                        actionable_task("r-1", "Fix first blocker", "f-1", &first_ref),
                        actionable_task("r-2", "Fix second blocker", "f-2", &second_ref),
                        actionable_task("r-3", "Fix third blocker", "f-3", &third_ref),
                    ],
                },
                AnalyzePassJson {
                    findings: Vec::new(),
                    remediation_tasks: Vec::new(),
                },
            ],
        },
    )?;

    let response = execute_stage_response(&stage_request(PLAN_STAGE_KEY, &workspace))
        .map_err(|error| error.to_string())?;
    assert_eq!(response.status, SUCCEEDED_STAGE_STATUS);
    assert_eq!(response.workflow_id.as_deref(), Some(PLANNING_WORKFLOW_ID));
    assert_eq!(response.analyze_pass_count, Some(2));
    assert_eq!(response.remediation_cycles_used, Some(1));
    assert_eq!(
        response
            .final_planning_readiness_status
            .as_ref()
            .map(|_| "ready"),
        Some("ready")
    );
    assert_eq!(response.remediation_tasks_attempted.len(), 3);
    assert_eq!(response.remediation_tasks_completed.len(), 3);
    assert!(response.remediation_tasks_skipped.is_empty());
    assert!(response.remaining_blocking_findings.is_empty());
    assert!(
        response
            .executed_commands
            .iter()
            .filter(|command| command.as_str() == ANALYZE_COMMAND_REF)
            .count()
            >= 2
    );
    assert!(remediation_root.join("first.txt").is_file());
    assert!(remediation_root.join("second.txt").is_file());
    assert!(remediation_root.join("third.txt").is_file());

    Ok(())
}

#[test]
fn plan_stage_stays_blocked_when_in_scope_blocking_findings_remain_unresolved() -> Result<(), String>
{
    let workspace = temp_workspace("speckit-plan-remediation-blocked")?;
    let (feature_dir, plan_path) = create_feature_packet(&workspace)?;
    write_plan_stage_scripts(
        &workspace,
        &json_script(&setup_plan_json(&plan_path)?),
        &json_script(&check_prerequisites_json(&feature_dir)?),
    )?;
    let actionable_ref = write_remediation_script(&workspace, "covered.sh", "covered.txt")?;
    write_planning_readiness_fixture(
        &workspace,
        &PlanningReadinessJson {
            analyze_passes: vec![AnalyzePassJson {
                findings: vec![
                    blocking_finding("covered", "Covered blocker"),
                    blocking_finding("uncovered", "Uncovered blocker"),
                    non_blocking_finding("note", "Operator note"),
                ],
                remediation_tasks: vec![actionable_task(
                    "r-covered",
                    "Fix covered blocker",
                    "covered",
                    &actionable_ref,
                )],
            }],
        },
    )?;

    let document = execute_stage_via_cli_with_env(&stage_request(PLAN_STAGE_KEY, &workspace), &[])?;
    expect_bool(&document, "/success", true)?;
    expect_string(&document, "/data/status", BLOCKED_STAGE_STATUS)?;
    expect_string(&document, "/data/workflow_id", PLANNING_WORKFLOW_ID)?;
    expect_string(
        &document,
        "/data/final_planning_readiness_status",
        "blocked",
    )?;
    expect_string(
        &document,
        "/data/remaining_blocking_findings/0/finding_id",
        "covered",
    )?;
    expect_string(
        &document,
        "/data/remaining_blocking_findings/1/finding_id",
        "uncovered",
    )?;
    expect_string(
        &document,
        "/data/remediation_tasks_completed/0/task_id",
        "r-covered",
    )?;
    expect_string(&document, "/data/planning_findings/2/finding_id", "note")?;

    Ok(())
}

#[test]
fn run_stage_claim_returns_blocked_workflow_recovery_without_placeholder_markers()
-> Result<(), String> {
    let workspace = temp_specify_workspace("speckit-override-run-blocked")?;
    let fake_specify = write_fake_specify_script(
        &workspace,
        &format!(
            "#!/bin/sh\nprintf 'Status: paused\\n'\nprintf 'Resume with: specify workflow resume {TEST_WORKFLOW_RUN_ID}\\n'\n"
        ),
    )?;
    let request = serde_json::to_string(&ExecuteStageRequest {
        run_id: "run-blocked-001".to_string(),
        stage_key: RUN_STAGE_KEY.to_string(),
        stage_attempt: 1,
        workspace_ref: workspace.to_string_lossy().into_owned(),
        adapter_id: ADAPTER_ID.to_string(),
        config_values: config_values()?,
        context_artifacts: Vec::new(),
    })
    .map_err(|error| format!("failed to encode execute-stage request: {error}"))?;

    let output = run_raw_with_stdin_and_env(
        &["execute-stage"],
        &request,
        &[(SPECIFY_BIN_ENV_VAR, fake_specify.to_string_lossy().as_ref())],
    )?;
    if !output.status.success() {
        return Err(format!(
            "execute-stage command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
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
        .ok_or_else(|| "missing next_action for blocked Speckit run bridge".to_string())?;
    if !next_action.contains(SPECIFY_WORKFLOW_RESUME_COMMAND) {
        return Err(format!(
            "expected blocked run next_action to resume the workflow, got {next_action}"
        ));
    }
    let placeholder_marker = workspace.join("speckit-run-claimed.txt");
    if placeholder_marker.exists() {
        return Err(format!(
            "unexpected placeholder marker created at {}",
            placeholder_marker.display()
        ));
    }

    Ok(())
}

#[test]
fn run_stage_claim_returns_failed_outcome_when_failure_fixture_exists() -> Result<(), String> {
    let workspace = temp_workspace("speckit-override-run-failure")?;
    write_fixture_file(&workspace, EXECUTE_STAGE_FAILED_FIXTURE_REF)?;
    let request = serde_json::to_string(&ExecuteStageRequest {
        run_id: "run-failure-001".to_string(),
        stage_key: RUN_STAGE_KEY.to_string(),
        stage_attempt: 1,
        workspace_ref: workspace.to_string_lossy().into_owned(),
        adapter_id: ADAPTER_ID.to_string(),
        config_values: config_values()?,
        context_artifacts: Vec::new(),
    })
    .map_err(|error| format!("failed to encode execute-stage request: {error}"))?;

    let output = run_with_stdin(&["execute-stage"], &request)?;
    let document: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("execute-stage stdout was not valid JSON: {error}"))?;

    expect_bool(&document, "/success", true)?;
    expect_string(&document, "/data/status", FAILED_STAGE_STATUS)?;
    expect_string(
        &document,
        "/data/failure_class",
        FAILURE_CLASS_ADAPTER_RUNTIME,
    )?;
    let marker_path = workspace.join("speckit-run-claimed.txt");
    if marker_path.exists() {
        return Err(format!(
            "unexpected success marker created at {}",
            marker_path.display()
        ));
    }

    Ok(())
}

#[test]
fn stage_failed_hook_returns_failed_outcome_when_failure_fixture_exists() -> Result<(), String> {
    let workspace = temp_workspace("speckit-override-hook-failure")?;
    write_fixture_file(&workspace, EMIT_HOOK_FAILED_FIXTURE_REF)?;
    let request = serde_json::to_string(&EmitHookRequest {
        run_id: "hook-failure-001".to_string(),
        hook_key: STAGE_FAILED_HOOK_KEY.to_string(),
        stage_key: RUN_STAGE_KEY.to_string(),
        stage_claimed: true,
        workspace_ref: workspace.to_string_lossy().into_owned(),
        payload_ref: TEST_HOOK_PAYLOAD_REF.to_string(),
    })
    .map_err(|error| format!("failed to encode emit-hook request: {error}"))?;

    let output = run_with_stdin(&["emit-hook"], &request)?;
    let document: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("emit-hook stdout was not valid JSON: {error}"))?;

    expect_bool(&document, "/success", true)?;
    expect_string(&document, "/data/status", FAILED_STAGE_STATUS)?;

    Ok(())
}

#[test]
fn coverage_helpers_cover_plan_stage_error_paths() -> Result<(), String> {
    let missing_workspace = temp_workspace("speckit-plan-missing")?;
    let unknown_response =
        execute_stage_response(&stage_request(UNKNOWN_STAGE_KEY, &missing_workspace))
            .map_err(|error| error.to_string())?;
    assert_eq!(unknown_response.status, FAILED_STAGE_STATUS);
    assert_eq!(
        unknown_response.failure_class.as_deref(),
        Some(FAILURE_CLASS_ADAPTER_RUNTIME)
    );
    assert!(
        unknown_response
            .next_action
            .as_deref()
            .is_some_and(|text| text.contains("supported lifecycle stages"))
    );

    let missing_response =
        execute_stage_response(&stage_request(PLAN_STAGE_KEY, &missing_workspace))
            .map_err(|error| error.to_string())?;
    assert_eq!(missing_response.status, BLOCKED_STAGE_STATUS);
    assert_eq!(missing_response.produced_artifacts, Vec::<String>::new());

    let setup_failure_workspace = temp_workspace("speckit-plan-setup-failure")?;
    write_plan_stage_scripts(
        &setup_failure_workspace,
        &failing_script(TEST_FAILURE_LINE),
        &json_script(&check_prerequisites_json(
            &setup_failure_workspace
                .join("specs")
                .join(TEST_FEATURE_DIR_NAME),
        )?),
    )?;
    write_file(
        &setup_failure_workspace.join(PLANNING_WORKFLOW_ARTIFACT_REF),
        "workflow: speckit-planning\n",
    )?;
    let setup_failure_response =
        execute_stage_response(&stage_request(PLAN_STAGE_KEY, &setup_failure_workspace))
            .map_err(|error| error.to_string())?;
    assert_eq!(setup_failure_response.status, BLOCKED_STAGE_STATUS);
    assert!(setup_failure_response.summary.contains(TEST_FAILURE_LINE));

    let prerequisites_failure_workspace = temp_workspace("speckit-plan-prereq-failure")?;
    let (_, plan_path) = create_feature_packet(&prerequisites_failure_workspace)?;
    let outside_plan_path = std::env::temp_dir()
        .join("boundline-speckit-coverage")
        .join("outside-plan.md");
    write_plan_stage_scripts(
        &prerequisites_failure_workspace,
        &json_script(&setup_plan_json(&outside_plan_path)?),
        &failing_script(TEST_FAILURE_LINE),
    )?;
    write_file(
        &prerequisites_failure_workspace.join(PLANNING_WORKFLOW_ARTIFACT_REF),
        "workflow: speckit-planning\n",
    )?;
    let prerequisites_failure_response = execute_stage_response(&stage_request(
        PLAN_STAGE_KEY,
        &prerequisites_failure_workspace,
    ))
    .map_err(|error| error.to_string())?;
    assert_eq!(prerequisites_failure_response.status, BLOCKED_STAGE_STATUS);
    assert!(
        prerequisites_failure_response
            .summary
            .contains(TEST_FAILURE_LINE)
    );
    assert_eq!(
        prerequisites_failure_response.produced_artifacts,
        vec![outside_plan_path.to_string_lossy().into_owned()]
    );

    let setup_parse_error_workspace = temp_workspace("speckit-plan-setup-parse")?;
    write_plan_stage_scripts(
        &setup_parse_error_workspace,
        &invalid_json_script(),
        &json_script(&check_prerequisites_json(
            &setup_parse_error_workspace
                .join("specs")
                .join(TEST_FEATURE_DIR_NAME),
        )?),
    )?;
    write_file(
        &setup_parse_error_workspace.join(PLANNING_WORKFLOW_ARTIFACT_REF),
        "workflow: speckit-planning\n",
    )?;
    let setup_parse_error =
        execute_stage_response(&stage_request(PLAN_STAGE_KEY, &setup_parse_error_workspace))
            .expect_err("expected setup-plan invalid JSON to fail");
    match setup_parse_error {
        SpeckitProfileError::ParseBridgeJson { command, .. } => {
            assert!(command.contains("setup-plan.sh"));
        }
        other => {
            return Err(format!(
                "expected setup-plan ParseBridgeJson error, got {other:?}"
            ));
        }
    }

    let prerequisites_parse_error_workspace = temp_workspace("speckit-plan-prereq-parse")?;
    let (_, parse_plan_path) = create_feature_packet(&prerequisites_parse_error_workspace)?;
    write_plan_stage_scripts(
        &prerequisites_parse_error_workspace,
        &json_script(&setup_plan_json(&parse_plan_path)?),
        &invalid_json_script(),
    )?;
    write_file(
        &prerequisites_parse_error_workspace.join(PLANNING_WORKFLOW_ARTIFACT_REF),
        "workflow: speckit-planning\n",
    )?;
    let prerequisites_parse_error = execute_stage_response(&stage_request(
        PLAN_STAGE_KEY,
        &prerequisites_parse_error_workspace,
    ))
    .expect_err("expected prerequisites invalid JSON to fail");
    match prerequisites_parse_error {
        SpeckitProfileError::ParseBridgeJson { command, .. } => {
            assert!(command.contains("check-prerequisites.sh"));
        }
        other => {
            return Err(format!(
                "expected prerequisites ParseBridgeJson error, got {other:?}"
            ));
        }
    }

    let _ = plan_path;

    Ok(())
}

#[test]
fn coverage_helpers_cover_run_stage_prerequisite_and_fake_specify_paths() -> Result<(), String> {
    let missing_workspace = temp_workspace("speckit-run-missing")?;
    let missing_response =
        execute_stage_response(&stage_request(RUN_STAGE_KEY, &missing_workspace))
            .map_err(|error| error.to_string())?;
    assert_eq!(missing_response.status, BLOCKED_STAGE_STATUS);
    assert_eq!(missing_response.produced_artifacts, Vec::<String>::new());

    let prerequisites_failure_workspace = temp_workspace("speckit-run-prereq-failure")?;
    create_run_stage_workspace(
        &prerequisites_failure_workspace,
        &failing_script(TEST_FAILURE_LINE),
    )?;
    let prerequisites_failure_response = execute_stage_response(&stage_request(
        RUN_STAGE_KEY,
        &prerequisites_failure_workspace,
    ))
    .map_err(|error| error.to_string())?;
    assert_eq!(prerequisites_failure_response.status, BLOCKED_STAGE_STATUS);
    assert!(
        prerequisites_failure_response
            .summary
            .contains(TEST_FAILURE_LINE)
    );

    let prerequisites_parse_error_workspace = temp_workspace("speckit-run-prereq-parse")?;
    create_run_stage_workspace(&prerequisites_parse_error_workspace, &invalid_json_script())?;
    let prerequisites_parse_error = execute_stage_response(&stage_request(
        RUN_STAGE_KEY,
        &prerequisites_parse_error_workspace,
    ))
    .expect_err("expected run-stage prerequisites invalid JSON to fail");
    match prerequisites_parse_error {
        SpeckitProfileError::ParseBridgeJson { command, .. } => {
            assert!(command.contains("check-prerequisites.sh"));
        }
        other => {
            return Err(format!(
                "expected run-stage ParseBridgeJson error, got {other:?}"
            ));
        }
    }

    let completed_workspace = temp_workspace("speckit-run-completed")?;
    let completed_specify = write_fake_specify_script(
        &completed_workspace,
        "#!/bin/sh\nprintf 'Status: completed\\n'\n",
    )?;
    create_run_stage_workspace(
        &completed_workspace,
        &json_script(&check_prerequisites_json(
            &create_feature_packet(&completed_workspace)?.0,
        )?),
    )?;
    let completed_document = execute_stage_via_cli_with_env(
        &stage_request(RUN_STAGE_KEY, &completed_workspace),
        &[(
            SPECIFY_BIN_ENV_VAR,
            completed_specify.to_string_lossy().as_ref(),
        )],
    )?;
    expect_bool(&completed_document, "/success", true)?;
    expect_string(&completed_document, "/data/status", SUCCEEDED_STAGE_STATUS)?;
    expect_string(
        &completed_document,
        "/data/workflow_id",
        IMPLEMENTATION_WORKFLOW_ID,
    )?;
    expect_string(
        &completed_document,
        "/data/implementation_status",
        "completed",
    )?;

    let no_status_workspace = temp_workspace("speckit-run-no-status")?;
    let no_status_specify = write_fake_specify_script(
        &no_status_workspace,
        &format!("#!/bin/sh\necho '{TEST_FAILURE_LINE}' >&2\nexit 0\n"),
    )?;
    let feature_dir = create_feature_packet_without_spec(&no_status_workspace)?;
    create_run_stage_workspace(
        &no_status_workspace,
        &json_script(&check_prerequisites_json(&feature_dir)?),
    )?;
    let no_status_document = execute_stage_via_cli_with_env(
        &stage_request(RUN_STAGE_KEY, &no_status_workspace),
        &[(
            SPECIFY_BIN_ENV_VAR,
            no_status_specify.to_string_lossy().as_ref(),
        )],
    )?;
    expect_bool(&no_status_document, "/success", true)?;
    expect_string(&no_status_document, "/data/status", BLOCKED_STAGE_STATUS)?;
    expect_string(
        &no_status_document,
        "/data/workflow_id",
        IMPLEMENTATION_WORKFLOW_ID,
    )?;
    expect_string(
        &no_status_document,
        "/data/implementation_status",
        "blocked",
    )?;
    let no_status_summary = no_status_document
        .pointer("/data/summary")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing run-stage blocked summary".to_string())?;
    if !no_status_summary.contains(TEST_FAILURE_LINE) {
        return Err(format!(
            "expected blocked run summary to include {TEST_FAILURE_LINE}, got {no_status_summary}"
        ));
    }
    let no_status_next_action = no_status_document
        .pointer("/data/next_action")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing run-stage next_action".to_string())?;
    if !no_status_next_action.contains(FEATURE_DIR_RERUN_SUFFIX) {
        return Err(format!(
            "expected rerun next_action to mention {FEATURE_DIR_RERUN_SUFFIX}, got {no_status_next_action}"
        ));
    }

    let failed_workspace = temp_workspace("speckit-run-failed")?;
    let failed_specify = write_fake_specify_script(
        &failed_workspace,
        &format!(
            "#!/bin/sh\necho 'Run ID: {TEST_WORKFLOW_RUN_ID}' >&2\necho '{TEST_FAILURE_LINE}' >&2\nexit 1\n"
        ),
    )?;
    create_run_stage_workspace(
        &failed_workspace,
        &json_script(&check_prerequisites_json(
            &create_feature_packet(&failed_workspace)?.0,
        )?),
    )?;
    let failed_document = execute_stage_via_cli_with_env(
        &stage_request(RUN_STAGE_KEY, &failed_workspace),
        &[(
            SPECIFY_BIN_ENV_VAR,
            failed_specify.to_string_lossy().as_ref(),
        )],
    )?;
    expect_bool(&failed_document, "/success", true)?;
    expect_string(&failed_document, "/data/status", FAILED_STAGE_STATUS)?;
    expect_string(
        &failed_document,
        "/data/workflow_id",
        IMPLEMENTATION_WORKFLOW_ID,
    )?;
    expect_string(&failed_document, "/data/implementation_status", "failed")?;
    expect_string(
        &failed_document,
        "/data/failure_class",
        FAILURE_CLASS_ADAPTER_RUNTIME,
    )?;
    expect_string(
        &failed_document,
        "/data/next_action",
        "specify workflow resume run-123",
    )?;

    let missing_program_workspace = temp_workspace("speckit-run-missing-program")?;
    create_run_stage_workspace(
        &missing_program_workspace,
        &json_script(&check_prerequisites_json(
            &create_feature_packet(&missing_program_workspace)?.0,
        )?),
    )?;
    let missing_program_output = run_raw_with_stdin_and_env(
        &["execute-stage"],
        &serde_json::to_string(&stage_request(RUN_STAGE_KEY, &missing_program_workspace))
            .map_err(|error| format!("failed to encode execute-stage request: {error}"))?,
        &[(SPECIFY_BIN_ENV_VAR, "/definitely/missing/speckit-binary")],
    )?;
    if missing_program_output.status.success() {
        return Err("expected missing specify override to fail the command".to_string());
    }
    let missing_program_stderr = String::from_utf8(missing_program_output.stderr)
        .map_err(|error| format!("stderr was not valid UTF-8: {error}"))?;
    if !missing_program_stderr.contains("failed to run bridge command") {
        return Err(format!(
            "expected missing program stderr to mention bridge command failure, got {missing_program_stderr}"
        ));
    }

    Ok(())
}

fn config_values() -> Result<Vec<boundline_adapter_speckit::ConfigValue>, String> {
    let request = PreflightRequest {
        boundline_version: TEST_BOUNDLINE_VERSION.to_string(),
        workspace_ref: TEST_WORKSPACE_REF.to_string(),
        non_interactive: true,
        config_values: vec![
            path_value(TEMPLATE_REPO_FIELD_KEY, TEMPLATE_REPO_PATH),
            path_value(ADAPTER_REPO_FIELD_KEY, ADAPTER_REPO_PATH),
        ],
    };
    Ok(request.config_values)
}

fn path_value(field_key: &str, path_value: &str) -> boundline_adapter_speckit::ConfigValue {
    boundline_adapter_speckit::ConfigValue {
        field_key: field_key.to_string(),
        value_kind: boundline_adapter_speckit::ConfigValueKind::Path,
        string_value: None,
        path_value: Some(path_value.to_string()),
        bool_value: None,
        int_value: None,
    }
}

fn stage_request(stage_key: &str, workspace_path: &Path) -> ExecuteStageRequest {
    ExecuteStageRequest {
        run_id: "coverage-run-001".to_string(),
        stage_key: stage_key.to_string(),
        stage_attempt: 2,
        workspace_ref: workspace_path.to_string_lossy().into_owned(),
        adapter_id: ADAPTER_ID.to_string(),
        config_values: config_values().unwrap_or_default(),
        context_artifacts: Vec::new(),
    }
}

fn run_with_stdin(args: &[&str], input: &str) -> Result<std::process::Output, String> {
    let output = run_raw_with_stdin_and_env(args, input, &[])?;
    if !output.status.success() {
        return Err(format!(
            "command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(output)
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

fn temp_workspace(prefix: &str) -> Result<PathBuf, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("failed to read system time: {error}"))?
        .as_nanos();
    let workspace = std::env::temp_dir().join(format!("{prefix}-{timestamp}"));
    fs::create_dir_all(&workspace)
        .map_err(|error| format!("failed to create temp workspace: {error}"))?;
    Ok(workspace)
}

fn temp_specify_workspace(prefix: &str) -> Result<PathBuf, String> {
    let workspace = temp_workspace(prefix)?;
    seed_specify_workspace(&workspace)?;
    Ok(workspace)
}

fn seed_specify_workspace(workspace_path: &Path) -> Result<(), String> {
    let feature_dir = workspace_path.join(REAL_FEATURE_DIR_REF);
    fs::create_dir_all(&feature_dir)
        .map_err(|error| format!("failed to create real feature dir: {error}"))?;
    let plan_path = workspace_path.join(PLAN_ARTIFACT_REF);
    write_file(
        &workspace_path.join(SPEC_ARTIFACT_REF),
        REAL_FEATURE_SPEC_CONTENT,
    )?;
    write_file(&plan_path, REAL_FEATURE_PLAN_CONTENT)?;
    write_file(
        &workspace_path.join(TASKS_ARTIFACT_REF),
        REAL_FEATURE_TASKS_CONTENT,
    )?;
    write_plan_stage_scripts(
        workspace_path,
        &json_script(&setup_plan_json(&plan_path)?),
        &json_script(&check_prerequisites_json(&feature_dir)?),
    )?;
    write_file(
        &workspace_path.join(PLANNING_WORKFLOW_ARTIFACT_REF),
        PLANNING_WORKFLOW_ASSET_CONTENT,
    )?;
    write_file(
        &workspace_path.join(IMPLEMENTATION_WORKFLOW_ARTIFACT_REF),
        IMPLEMENTATION_WORKFLOW_ASSET_CONTENT,
    )?;
    write_file(
        &workspace_path.join(IMPLEMENT_PROMPT_REF),
        IMPLEMENT_PROMPT_CONTENT,
    )?;
    Ok(())
}

fn create_feature_packet(workspace: &Path) -> Result<(PathBuf, PathBuf), String> {
    let feature_dir = workspace.join("specs").join(TEST_FEATURE_DIR_NAME);
    fs::create_dir_all(&feature_dir)
        .map_err(|error| format!("failed to create feature dir: {error}"))?;
    let plan_path = feature_dir.join("plan.md");
    fs::write(&plan_path, "# Plan\n")
        .map_err(|error| format!("failed to write plan artifact: {error}"))?;
    let spec_path = feature_dir.join("spec.md");
    fs::write(&spec_path, "# Coverage Feature\n")
        .map_err(|error| format!("failed to write feature spec: {error}"))?;
    Ok((feature_dir, plan_path))
}

fn create_feature_packet_without_spec(workspace: &Path) -> Result<PathBuf, String> {
    let feature_dir = workspace.join("specs").join(TEST_FEATURE_DIR_NAME);
    fs::create_dir_all(&feature_dir)
        .map_err(|error| format!("failed to create feature dir: {error}"))?;
    fs::write(feature_dir.join("plan.md"), "# Plan\n")
        .map_err(|error| format!("failed to write plan artifact: {error}"))?;
    fs::write(feature_dir.join("tasks.md"), "# Tasks\n")
        .map_err(|error| format!("failed to write tasks artifact: {error}"))?;
    Ok(feature_dir)
}

fn write_plan_stage_scripts(
    workspace_path: &Path,
    setup_plan_script: &str,
    prerequisites_script: &str,
) -> Result<(), String> {
    write_executable(
        &workspace_path.join(".specify/scripts/bash/setup-plan.sh"),
        setup_plan_script,
    )?;
    write_executable(
        &workspace_path.join(".specify/scripts/bash/check-prerequisites.sh"),
        prerequisites_script,
    )?;
    Ok(())
}

fn create_run_stage_workspace(
    workspace_path: &Path,
    prerequisites_script: &str,
) -> Result<(), String> {
    write_executable(
        &workspace_path.join(".specify/scripts/bash/check-prerequisites.sh"),
        prerequisites_script,
    )?;
    write_file(
        &workspace_path.join(IMPLEMENTATION_WORKFLOW_ARTIFACT_REF),
        IMPLEMENTATION_WORKFLOW_ASSET_CONTENT,
    )
}

fn write_fake_specify_script(workspace_path: &Path, contents: &str) -> Result<PathBuf, String> {
    let path = workspace_path.join("fake-specify.sh");
    write_executable(&path, contents)?;
    Ok(path)
}

fn write_planning_readiness_fixture(
    workspace_path: &Path,
    fixture: &PlanningReadinessJson<'_>,
) -> Result<(), String> {
    let fixture_json = serde_json::to_string(fixture)
        .map_err(|error| format!("failed to encode planning readiness fixture: {error}"))?;
    write_file(
        &workspace_path.join(PLANNING_READINESS_FIXTURE_REF),
        &fixture_json,
    )
}

fn write_remediation_script(
    workspace_path: &Path,
    script_name: &str,
    output_name: &str,
) -> Result<String, String> {
    let remediation_dir = workspace_path.join("remediation");
    fs::create_dir_all(&remediation_dir)
        .map_err(|error| format!("failed to create remediation dir: {error}"))?;
    let script_path = remediation_dir.join(script_name);
    let script_contents =
        format!("#!/bin/sh\nmkdir -p remediation\n: > remediation/{output_name}\n");
    write_executable(&script_path, &script_contents)?;
    Ok(format!("./{}", relative_ref(workspace_path, &script_path)))
}

fn write_executable(path: &Path, contents: &str) -> Result<(), String> {
    write_file(path, contents)?;
    let mut permissions = fs::metadata(path)
        .map_err(|error| format!("failed to read metadata {}: {error}", path.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .map_err(|error| format!("failed to set permissions {}: {error}", path.display()))
}

fn write_file(path: &Path, contents: &str) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Err(format!("path {} had no parent directory", path.display()));
    };
    fs::create_dir_all(parent).map_err(|error| format!("failed to create parent dir: {error}"))?;
    fs::write(path, contents)
        .map_err(|error| format!("failed to write file {}: {error}", path.display()))
}

fn relative_ref(workspace_path: &Path, path: &Path) -> String {
    path.strip_prefix(workspace_path)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

fn setup_plan_json(plan_path: &Path) -> Result<String, String> {
    serde_json::to_string(&SetupPlanJson {
        impl_plan: &plan_path.to_string_lossy(),
    })
    .map_err(|error| format!("failed to encode setup-plan JSON: {error}"))
}

fn check_prerequisites_json(feature_dir: &Path) -> Result<String, String> {
    serde_json::to_string(&CheckPrerequisitesJson {
        feature_dir: &feature_dir.to_string_lossy(),
        available_docs: vec!["tasks.md"],
    })
    .map_err(|error| format!("failed to encode prerequisites JSON: {error}"))
}

fn json_script(json: &str) -> String {
    format!("#!/bin/sh\nprintf '%s' '{json}'\n")
}

fn invalid_json_script() -> String {
    "#!/bin/sh\nprintf '%s' '{invalid'\n".to_string()
}

fn failing_script(error_line: &str) -> String {
    format!("#!/bin/sh\necho '{error_line}' >&2\nexit 1\n")
}

fn execute_stage_via_cli_with_env(
    request: &ExecuteStageRequest,
    envs: &[(&str, &str)],
) -> Result<Value, String> {
    let request_json = serde_json::to_string(request)
        .map_err(|error| format!("failed to encode execute-stage request: {error}"))?;
    let output = run_with_stdin_and_env(&["execute-stage"], &request_json, envs)?;
    serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("execute-stage stdout was not valid JSON: {error}"))
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

fn write_fixture_file(workspace: &Path, fixture_ref: &str) -> Result<(), String> {
    let fixture_path = workspace.join(fixture_ref);
    let Some(parent) = fixture_path.parent() else {
        return Err(format!(
            "fixture path {fixture_ref} had no parent directory"
        ));
    };
    fs::create_dir_all(parent).map_err(|error| format!("failed to create fixture dir: {error}"))?;
    fs::write(&fixture_path, "enabled\n")
        .map_err(|error| format!("failed to write fixture file: {error}"))
}

fn expect_bool(document: &Value, pointer: &str, expected: bool) -> Result<(), String> {
    let actual = document
        .pointer(pointer)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("missing bool at json pointer {pointer}"))?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!("expected {expected} at {pointer}, got {actual}"))
    }
}

fn expect_string(document: &Value, pointer: &str, expected: &str) -> Result<(), String> {
    let actual = document
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing string at json pointer {pointer}"))?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!("expected {expected} at {pointer}, got {actual}"))
    }
}

fn blocking_finding<'a>(finding_id: &'a str, summary: &'a str) -> PlanningFindingJson<'a> {
    PlanningFindingJson {
        finding_id,
        summary,
        severity: "blocking",
    }
}

fn non_blocking_finding<'a>(finding_id: &'a str, summary: &'a str) -> PlanningFindingJson<'a> {
    PlanningFindingJson {
        finding_id,
        summary,
        severity: "non_blocking",
    }
}

fn actionable_task<'a>(
    task_id: &'a str,
    summary: &'a str,
    finding_id: &'a str,
    command_ref: &'a str,
) -> PlanningRemediationTaskJson<'a> {
    PlanningRemediationTaskJson {
        task_id,
        summary,
        finding_ids: vec![finding_id],
        in_scope: true,
        safe: true,
        deterministic: true,
        requires_operator_input: false,
        command: Some(PlanningRemediationCommandJson {
            program: command_ref,
            args: Vec::new(),
        }),
    }
}
