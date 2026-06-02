//! Claimed-stage workflow bridge for the Speckit known profile.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::profile::{
    ExecuteStageRequest, ExecuteStageResponse, ImplementationWorkflowStatus, PlanningFinding,
    PlanningFindingSeverity, PlanningReadinessStatus, PlanningRemediationSkipReason,
    PlanningRemediationTaskOutcome, SpeckitProfileError,
};

const FIXTURE_DIRECTORY_NAME: &str = ".speckit";
const EXECUTE_STAGE_FAILED_FIXTURE_FILE_NAME: &str = "execute-stage.failed";
const PLANNING_READINESS_FIXTURE_FILE_NAME: &str = "planning-readiness.json";

const PLAN_STAGE_KEY: &str = "plan";
const RUN_STAGE_KEY: &str = "run";

const STATUS_BLOCKED: &str = "blocked";
const STATUS_FAILED: &str = "failed";
const STATUS_SUCCEEDED: &str = "succeeded";

const FAILURE_CLASS_ADAPTER_RUNTIME: &str = "adapter_runtime";
const FAILURE_NEXT_ACTION: &str =
    "Remove .speckit/execute-stage.failed or repair the stage input before retrying";

const FEATURE_SPEC_FILE_NAME: &str = "spec.md";
const ANALYSIS_FILE_NAME: &str = "analysis.md";
const PLAN_FILE_NAME: &str = "plan.md";
const TASKS_FILE_NAME: &str = "tasks.md";

const SETUP_PLAN_SCRIPT_REF: &str = ".specify/scripts/bash/setup-plan.sh";
const CHECK_PREREQUISITES_SCRIPT_REF: &str = ".specify/scripts/bash/check-prerequisites.sh";
const IMPLEMENT_PROMPT_REF: &str = ".github/prompts/speckit.implement.prompt.md";
const PLANNING_WORKFLOW_FILE_REF: &str = ".specify/workflows/speckit/planning.yml";
const IMPLEMENTATION_WORKFLOW_FILE_REF: &str = ".specify/workflows/speckit/implementation.yml";

const SHELL_PROGRAM: &str = "sh";
const SPECIFY_PROGRAM: &str = "specify";
const SPECIFY_PROGRAM_ENV_VAR: &str = "BOUNDLINE_SPECKIT_SPECIFY_BIN";
const SPECKIT_PLANNING_WORKFLOW_ID: &str = "speckit-planning";
const SPECKIT_IMPLEMENTATION_WORKFLOW_ID: &str = "speckit-implementation";
const SPECKIT_ANALYZE_COMMAND: &str = "speckit.analyze";

const JSON_FLAG: &str = "--json";
const INCLUDE_TASKS_FLAG: &str = "--include-tasks";
const REQUIRE_TASKS_FLAG: &str = "--require-tasks";
const WORKFLOW_INPUT_FLAG: &str = "-i";
const WORKFLOW_SPEC_INPUT_KEY: &str = "spec";

const WORKFLOW_GATE_MARKER: &str = "type: gate";
const WORKFLOW_STATUS_PREFIX: &str = "Status:";
const WORKFLOW_RUN_ID_PREFIX: &str = "Run ID:";
const WORKFLOW_RESUME_PREFIX: &str = "Resume with:";
const WORKFLOW_STATUS_ABORTED: &str = "aborted";
const WORKFLOW_STATUS_COMPLETED: &str = "completed";
const WORKFLOW_STATUS_PAUSED: &str = "paused";

const PLAN_STAGE_SUCCESS_SUMMARY: &str =
    "Speckit prepared feature planning artifacts via Spec Kit workspace scripts";
const PLAN_STAGE_BLOCKED_SUMMARY: &str =
    "Speckit planning bridge could not prepare Spec Kit planning artifacts";
const RUN_STAGE_BLOCKED_SUMMARY: &str =
    "Speckit implementation workflow paused and requires interactive continuation";
const RUN_STAGE_COMPLETED_SUMMARY: &str =
    "Speckit implementation workflow completed through the real Spec Kit entrypoint";
const RUN_STAGE_FAILED_SUMMARY: &str =
    "Speckit implementation workflow failed before it could yield a resumable run";
const PLAN_STAGE_READY_SUMMARY: &str =
    "Speckit planning completed after clearing blocking analyze findings";
const PLAN_STAGE_LIMIT_SUMMARY: &str =
    "Speckit planning stopped because blocking analyze findings remained after bounded remediation";
const PLAN_STAGE_UNRESOLVED_SUMMARY: &str = "Speckit planning stopped because blocking analyze findings could not be remediated automatically";
const PLAN_STAGE_RETRY_ACTION: &str = "Review the remaining blocking planning findings and rerun Boundline plan after updating the packet";
const MISSING_SPECIFY_WORKSPACE_RECOVERY: &str =
    "Initialize Spec Kit in the target workspace and create the feature packet before retrying";
const MAX_PLAN_ANALYZE_PASSES: usize = 3;
const MAX_PLAN_REMEDIATION_CYCLES: usize = 2;

#[derive(Debug, Deserialize)]
struct SetupPlanResult {
    #[serde(rename = "IMPL_PLAN")]
    impl_plan: String,
}

#[derive(Debug, Deserialize)]
struct CheckPrerequisitesResult {
    #[serde(rename = "FEATURE_DIR")]
    feature_dir: String,
    #[serde(rename = "AVAILABLE_DOCS", default)]
    available_docs: Vec<String>,
}

#[derive(Debug, Default)]
struct WorkflowRunStatus {
    status: Option<String>,
    run_id: Option<String>,
    resume_command: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
struct PlanningReadinessFixture {
    #[serde(default)]
    analyze_passes: Vec<PlanningAnalyzePassFixture>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
struct PlanningAnalyzePassFixture {
    #[serde(default)]
    findings: Vec<PlanningFindingFixture>,
    #[serde(default)]
    remediation_tasks: Vec<PlanningRemediationTaskFixture>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct PlanningFindingFixture {
    finding_id: String,
    summary: String,
    severity: PlanningFindingSeverity,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct PlanningRemediationTaskFixture {
    task_id: String,
    summary: String,
    #[serde(default)]
    finding_ids: Vec<String>,
    #[serde(default = "default_true")]
    in_scope: bool,
    #[serde(default = "default_true")]
    safe: bool,
    #[serde(default = "default_true")]
    deterministic: bool,
    #[serde(default)]
    requires_operator_input: bool,
    #[serde(default)]
    command: Option<PlanningRemediationCommandFixture>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
struct PlanningRemediationCommandFixture {
    program: String,
    #[serde(default)]
    args: Vec<String>,
}

#[derive(Debug, Default)]
struct PlanningStageOutcome {
    planning_findings: Vec<PlanningFinding>,
    remediation_tasks_attempted: Vec<PlanningRemediationTaskOutcome>,
    remediation_tasks_completed: Vec<PlanningRemediationTaskOutcome>,
    remediation_tasks_skipped: Vec<PlanningRemediationTaskOutcome>,
    remaining_blocking_findings: Vec<PlanningFinding>,
    final_planning_readiness_status: PlanningReadinessStatus,
    analyze_pass_count: usize,
    remediation_cycles_used: usize,
    summary: String,
    next_action: Option<String>,
    status: String,
}

/// Executes the claimed-stage workflow bridge, with a bounded fixture mode for
/// deterministic failed outcomes in local tests.
pub(crate) fn execute_stage_response(
    request: &ExecuteStageRequest,
) -> Result<ExecuteStageResponse, SpeckitProfileError> {
    if execute_stage_failure_fixture_exists(&request.workspace_ref) {
        return Ok(failed_response(
            &request.stage_key,
            request.stage_attempt,
            FAILURE_NEXT_ACTION,
        ));
    }

    match request.stage_key.as_str() {
        PLAN_STAGE_KEY => execute_plan_stage(request),
        RUN_STAGE_KEY => execute_run_stage(request),
        _ => Ok(failed_response(
            &request.stage_key,
            request.stage_attempt,
            "Update the adapter stage mapping so Speckit only claims supported lifecycle stages",
        )),
    }
}

fn execute_plan_stage(
    request: &ExecuteStageRequest,
) -> Result<ExecuteStageResponse, SpeckitProfileError> {
    let workspace_path = Path::new(&request.workspace_ref);
    let setup_plan_path = workspace_path.join(SETUP_PLAN_SCRIPT_REF);
    let prerequisites_path = workspace_path.join(CHECK_PREREQUISITES_SCRIPT_REF);
    let planning_workflow_path = workspace_path.join(PLANNING_WORKFLOW_FILE_REF);
    let uses_planning_readiness_fixture = planning_readiness_fixture_exists(&request.workspace_ref);
    let mut executed_commands = Vec::new();

    if !setup_plan_path.is_file()
        || !prerequisites_path.is_file()
        || (!uses_planning_readiness_fixture && !planning_workflow_path.is_file())
    {
        return Ok(blocked_response(
            PLAN_STAGE_BLOCKED_SUMMARY,
            Vec::new(),
            Some(SPECKIT_PLANNING_WORKFLOW_ID.to_string()),
            Vec::new(),
            PlanningStageOutcome {
                final_planning_readiness_status: PlanningReadinessStatus::Blocked,
                status: STATUS_BLOCKED.to_string(),
                summary: PLAN_STAGE_BLOCKED_SUMMARY.to_string(),
                next_action: Some(MISSING_SPECIFY_WORKSPACE_RECOVERY.to_string()),
                ..PlanningStageOutcome::default()
            },
            Some(MISSING_SPECIFY_WORKSPACE_RECOVERY.to_string()),
        ));
    }

    let setup_command_display = format!("{SHELL_PROGRAM} {SETUP_PLAN_SCRIPT_REF} {JSON_FLAG}");
    executed_commands.push(setup_command_display.clone());
    let setup_output = run_bridge_command(
        &request.workspace_ref,
        workspace_path,
        &setup_command_display,
        SHELL_PROGRAM,
        &[SETUP_PLAN_SCRIPT_REF.to_string(), JSON_FLAG.to_string()],
    )?;
    if !setup_output.status.success() {
        return Ok(blocked_response(
            &format!(
                "{PLAN_STAGE_BLOCKED_SUMMARY}: {}",
                command_output_summary(&setup_output, "setup-plan did not succeed")
            ),
            Vec::new(),
            Some(SPECKIT_PLANNING_WORKFLOW_ID.to_string()),
            executed_commands,
            PlanningStageOutcome {
                final_planning_readiness_status: PlanningReadinessStatus::Blocked,
                status: STATUS_BLOCKED.to_string(),
                summary: PLAN_STAGE_BLOCKED_SUMMARY.to_string(),
                next_action: Some(MISSING_SPECIFY_WORKSPACE_RECOVERY.to_string()),
                ..PlanningStageOutcome::default()
            },
            Some(MISSING_SPECIFY_WORKSPACE_RECOVERY.to_string()),
        ));
    }
    let setup_plan = parse_bridge_json::<SetupPlanResult>(
        &request.workspace_ref,
        &setup_command_display,
        &setup_output,
    )?;

    let prerequisites_command_display = format!(
        "{SHELL_PROGRAM} {CHECK_PREREQUISITES_SCRIPT_REF} {JSON_FLAG} {INCLUDE_TASKS_FLAG}"
    );
    executed_commands.push(prerequisites_command_display.clone());
    let prerequisites_output = run_bridge_command(
        &request.workspace_ref,
        workspace_path,
        &prerequisites_command_display,
        SHELL_PROGRAM,
        &[
            CHECK_PREREQUISITES_SCRIPT_REF.to_string(),
            JSON_FLAG.to_string(),
            INCLUDE_TASKS_FLAG.to_string(),
        ],
    )?;
    if !prerequisites_output.status.success() {
        return Ok(blocked_response(
            &format!(
                "{PLAN_STAGE_BLOCKED_SUMMARY}: {}",
                command_output_summary(
                    &prerequisites_output,
                    "check-prerequisites did not succeed"
                )
            ),
            vec![relative_artifact_ref(
                workspace_path,
                Path::new(&setup_plan.impl_plan),
            )],
            Some(SPECKIT_PLANNING_WORKFLOW_ID.to_string()),
            executed_commands,
            PlanningStageOutcome {
                final_planning_readiness_status: PlanningReadinessStatus::Blocked,
                status: STATUS_BLOCKED.to_string(),
                summary: PLAN_STAGE_BLOCKED_SUMMARY.to_string(),
                next_action: Some(MISSING_SPECIFY_WORKSPACE_RECOVERY.to_string()),
                ..PlanningStageOutcome::default()
            },
            Some(MISSING_SPECIFY_WORKSPACE_RECOVERY.to_string()),
        ));
    }
    let prerequisites = parse_bridge_json::<CheckPrerequisitesResult>(
        &request.workspace_ref,
        &prerequisites_command_display,
        &prerequisites_output,
    )?;

    let workflow_spec_input = workflow_spec_input(workspace_path, &prerequisites);
    if !uses_planning_readiness_fixture {
        let specify_program = resolved_specify_program();
        let workflow_command_display = format!(
            "{specify_program} workflow run {PLANNING_WORKFLOW_FILE_REF} {WORKFLOW_INPUT_FLAG} {WORKFLOW_SPEC_INPUT_KEY}={workflow_spec_input}"
        );
        executed_commands.push(workflow_command_display.clone());
        let workflow_output = run_bridge_command(
            &request.workspace_ref,
            workspace_path,
            &workflow_command_display,
            &specify_program,
            &[
                "workflow".to_string(),
                "run".to_string(),
                PLANNING_WORKFLOW_FILE_REF.to_string(),
                WORKFLOW_INPUT_FLAG.to_string(),
                format!("{WORKFLOW_SPEC_INPUT_KEY}={workflow_spec_input}"),
            ],
        )?;
        let workflow_status = parse_workflow_run_status(&workflow_output);
        if !workflow_output.status.success() {
            return Ok(blocked_response(
                &format!(
                    "{PLAN_STAGE_BLOCKED_SUMMARY}: {}",
                    workflow_pause_reason(
                        workspace_path,
                        PLANNING_WORKFLOW_FILE_REF,
                        &workflow_output,
                    )
                ),
                Vec::new(),
                Some(SPECKIT_PLANNING_WORKFLOW_ID.to_string()),
                executed_commands,
                PlanningStageOutcome {
                    final_planning_readiness_status: PlanningReadinessStatus::Blocked,
                    status: STATUS_BLOCKED.to_string(),
                    summary: PLAN_STAGE_BLOCKED_SUMMARY.to_string(),
                    next_action: workflow_resume_or_recovery_command(
                        &workflow_status,
                        PLANNING_WORKFLOW_FILE_REF,
                        &workflow_spec_input,
                    ),
                    ..PlanningStageOutcome::default()
                },
                workflow_resume_or_recovery_command(
                    &workflow_status,
                    PLANNING_WORKFLOW_FILE_REF,
                    &workflow_spec_input,
                ),
            ));
        }
        if matches!(
            workflow_status.status.as_deref(),
            Some(WORKFLOW_STATUS_PAUSED | WORKFLOW_STATUS_ABORTED)
        ) {
            return Ok(blocked_response(
                &workflow_pause_reason(
                    workspace_path,
                    PLANNING_WORKFLOW_FILE_REF,
                    &workflow_output,
                ),
                Vec::new(),
                Some(SPECKIT_PLANNING_WORKFLOW_ID.to_string()),
                executed_commands,
                PlanningStageOutcome {
                    final_planning_readiness_status: PlanningReadinessStatus::Blocked,
                    status: STATUS_BLOCKED.to_string(),
                    summary: PLAN_STAGE_BLOCKED_SUMMARY.to_string(),
                    next_action: workflow_resume_or_recovery_command(
                        &workflow_status,
                        PLANNING_WORKFLOW_FILE_REF,
                        &workflow_spec_input,
                    ),
                    ..PlanningStageOutcome::default()
                },
                workflow_resume_or_recovery_command(
                    &workflow_status,
                    PLANNING_WORKFLOW_FILE_REF,
                    &workflow_spec_input,
                ),
            ));
        }
    }

    let produced_artifacts = plan_stage_artifacts(workspace_path, &setup_plan, &prerequisites);
    let planning_outcome =
        execute_planning_readiness_bridge(request, workspace_path, &mut executed_commands)?;

    Ok(ExecuteStageResponse {
        status: planning_outcome.status.clone(),
        summary: planning_outcome.summary.clone(),
        produced_artifacts,
        workflow_id: Some(SPECKIT_PLANNING_WORKFLOW_ID.to_string()),
        executed_commands,
        planning_findings: planning_outcome.planning_findings,
        remediation_tasks_attempted: planning_outcome.remediation_tasks_attempted,
        remediation_tasks_completed: planning_outcome.remediation_tasks_completed,
        remediation_tasks_skipped: planning_outcome.remediation_tasks_skipped,
        remaining_blocking_findings: planning_outcome.remaining_blocking_findings,
        final_planning_readiness_status: Some(planning_outcome.final_planning_readiness_status),
        analyze_pass_count: Some(planning_outcome.analyze_pass_count),
        remediation_cycles_used: Some(planning_outcome.remediation_cycles_used),
        implementation_status: None,
        validation_refs: Vec::new(),
        failure_class: None,
        next_action: planning_outcome.next_action,
    })
}

fn execute_run_stage(
    request: &ExecuteStageRequest,
) -> Result<ExecuteStageResponse, SpeckitProfileError> {
    let workspace_path = Path::new(&request.workspace_ref);
    let prerequisites_path = workspace_path.join(CHECK_PREREQUISITES_SCRIPT_REF);
    let workflow_file_ref = resolved_implementation_workflow_file_ref(workspace_path);
    let workflow_path = workspace_path.join(workflow_file_ref);
    let mut executed_commands = Vec::new();

    if !prerequisites_path.is_file() || !workflow_path.is_file() {
        return Ok(blocked_response(
            RUN_STAGE_BLOCKED_SUMMARY,
            Vec::new(),
            Some(SPECKIT_IMPLEMENTATION_WORKFLOW_ID.to_string()),
            Vec::new(),
            PlanningStageOutcome::default(),
            Some(MISSING_SPECIFY_WORKSPACE_RECOVERY.to_string()),
        ));
    }

    let prerequisites_command_display = format!(
        "{SHELL_PROGRAM} {CHECK_PREREQUISITES_SCRIPT_REF} {JSON_FLAG} {REQUIRE_TASKS_FLAG} {INCLUDE_TASKS_FLAG}"
    );
    executed_commands.push(prerequisites_command_display.clone());
    let prerequisites_output = run_bridge_command(
        &request.workspace_ref,
        workspace_path,
        &prerequisites_command_display,
        SHELL_PROGRAM,
        &[
            CHECK_PREREQUISITES_SCRIPT_REF.to_string(),
            JSON_FLAG.to_string(),
            REQUIRE_TASKS_FLAG.to_string(),
            INCLUDE_TASKS_FLAG.to_string(),
        ],
    )?;
    if !prerequisites_output.status.success() {
        return Ok(blocked_response(
            &format!(
                "{RUN_STAGE_BLOCKED_SUMMARY}: {}",
                command_output_summary(
                    &prerequisites_output,
                    "check-prerequisites did not succeed"
                )
            ),
            Vec::new(),
            Some(SPECKIT_IMPLEMENTATION_WORKFLOW_ID.to_string()),
            executed_commands,
            PlanningStageOutcome::default(),
            Some(MISSING_SPECIFY_WORKSPACE_RECOVERY.to_string()),
        ));
    }
    let prerequisites = parse_bridge_json::<CheckPrerequisitesResult>(
        &request.workspace_ref,
        &prerequisites_command_display,
        &prerequisites_output,
    )?;
    let produced_artifacts = run_stage_artifacts(workspace_path, &prerequisites);

    let workflow_spec_input = workflow_spec_input(workspace_path, &prerequisites);
    let specify_program = resolved_specify_program();
    let workflow_command_display = format!(
        "{specify_program} workflow run {workflow_file_ref} {WORKFLOW_INPUT_FLAG} {WORKFLOW_SPEC_INPUT_KEY}={workflow_spec_input}"
    );
    executed_commands.push(workflow_command_display.clone());
    let workflow_output = run_bridge_command(
        &request.workspace_ref,
        workspace_path,
        &workflow_command_display,
        &specify_program,
        &[
            "workflow".to_string(),
            "run".to_string(),
            workflow_file_ref.to_string(),
            WORKFLOW_INPUT_FLAG.to_string(),
            format!("{WORKFLOW_SPEC_INPUT_KEY}={workflow_spec_input}"),
        ],
    )?;
    let workflow_status = parse_workflow_run_status(&workflow_output);

    match workflow_status.status.as_deref() {
        Some(WORKFLOW_STATUS_COMPLETED) => Ok(ExecuteStageResponse {
            status: STATUS_SUCCEEDED.to_string(),
            summary: RUN_STAGE_COMPLETED_SUMMARY.to_string(),
            produced_artifacts: produced_artifacts.clone(),
            workflow_id: Some(SPECKIT_IMPLEMENTATION_WORKFLOW_ID.to_string()),
            executed_commands,
            planning_findings: Vec::new(),
            remediation_tasks_attempted: Vec::new(),
            remediation_tasks_completed: Vec::new(),
            remediation_tasks_skipped: Vec::new(),
            remaining_blocking_findings: Vec::new(),
            final_planning_readiness_status: None,
            analyze_pass_count: None,
            remediation_cycles_used: None,
            implementation_status: Some(ImplementationWorkflowStatus::Completed),
            validation_refs: produced_artifacts,
            failure_class: None,
            next_action: None,
        }),
        Some(WORKFLOW_STATUS_PAUSED) | Some(WORKFLOW_STATUS_ABORTED) => Ok(blocked_response(
            &format!(
                "{RUN_STAGE_BLOCKED_SUMMARY}: {}",
                workflow_pause_reason(workspace_path, workflow_file_ref, &workflow_output)
            ),
            produced_artifacts,
            Some(SPECKIT_IMPLEMENTATION_WORKFLOW_ID.to_string()),
            executed_commands,
            PlanningStageOutcome::default(),
            workflow_resume_or_recovery_command(
                &workflow_status,
                workflow_file_ref,
                &workflow_spec_input,
            ),
        )),
        _ if workflow_output.status.success() => Ok(blocked_response(
            &format!(
                "{RUN_STAGE_BLOCKED_SUMMARY}: {}",
                workflow_pause_reason(workspace_path, workflow_file_ref, &workflow_output)
            ),
            produced_artifacts,
            Some(SPECKIT_IMPLEMENTATION_WORKFLOW_ID.to_string()),
            executed_commands,
            PlanningStageOutcome::default(),
            workflow_resume_or_recovery_command(
                &workflow_status,
                workflow_file_ref,
                &workflow_spec_input,
            ),
        )),
        _ => Ok(ExecuteStageResponse {
            status: STATUS_FAILED.to_string(),
            summary: format!(
                "{RUN_STAGE_FAILED_SUMMARY}: {}",
                command_output_summary(&workflow_output, "workflow run did not succeed")
            ),
            produced_artifacts: produced_artifacts.clone(),
            workflow_id: Some(SPECKIT_IMPLEMENTATION_WORKFLOW_ID.to_string()),
            executed_commands,
            planning_findings: Vec::new(),
            remediation_tasks_attempted: Vec::new(),
            remediation_tasks_completed: Vec::new(),
            remediation_tasks_skipped: Vec::new(),
            remaining_blocking_findings: Vec::new(),
            final_planning_readiness_status: None,
            analyze_pass_count: None,
            remediation_cycles_used: None,
            implementation_status: Some(ImplementationWorkflowStatus::Failed),
            validation_refs: produced_artifacts,
            failure_class: Some(FAILURE_CLASS_ADAPTER_RUNTIME.to_string()),
            next_action: workflow_resume_or_recovery_command(
                &workflow_status,
                workflow_file_ref,
                &workflow_spec_input,
            ),
        }),
    }
}

fn execute_stage_failure_fixture_exists(workspace_ref: &str) -> bool {
    fixture_path(workspace_ref, EXECUTE_STAGE_FAILED_FIXTURE_FILE_NAME).is_file()
}

fn failed_response(
    stage_key: &str,
    stage_attempt: usize,
    next_action: &str,
) -> ExecuteStageResponse {
    ExecuteStageResponse {
        status: STATUS_FAILED.to_string(),
        summary: format!("Speckit failed stage {stage_key} attempt {stage_attempt}"),
        produced_artifacts: Vec::new(),
        workflow_id: None,
        executed_commands: Vec::new(),
        planning_findings: Vec::new(),
        remediation_tasks_attempted: Vec::new(),
        remediation_tasks_completed: Vec::new(),
        remediation_tasks_skipped: Vec::new(),
        remaining_blocking_findings: Vec::new(),
        final_planning_readiness_status: None,
        analyze_pass_count: None,
        remediation_cycles_used: None,
        implementation_status: None,
        validation_refs: Vec::new(),
        failure_class: Some(FAILURE_CLASS_ADAPTER_RUNTIME.to_string()),
        next_action: Some(next_action.to_string()),
    }
}

fn blocked_response(
    summary: &str,
    produced_artifacts: Vec<String>,
    workflow_id: Option<String>,
    executed_commands: Vec<String>,
    planning_outcome: PlanningStageOutcome,
    next_action: Option<String>,
) -> ExecuteStageResponse {
    let implementation_status =
        if workflow_id.as_deref() == Some(SPECKIT_IMPLEMENTATION_WORKFLOW_ID) {
            Some(ImplementationWorkflowStatus::Blocked)
        } else {
            None
        };
    let validation_refs = produced_artifacts.clone();

    ExecuteStageResponse {
        status: STATUS_BLOCKED.to_string(),
        summary: summary.to_string(),
        produced_artifacts,
        workflow_id,
        executed_commands,
        planning_findings: planning_outcome.planning_findings,
        remediation_tasks_attempted: planning_outcome.remediation_tasks_attempted,
        remediation_tasks_completed: planning_outcome.remediation_tasks_completed,
        remediation_tasks_skipped: planning_outcome.remediation_tasks_skipped,
        remaining_blocking_findings: planning_outcome.remaining_blocking_findings,
        final_planning_readiness_status: match planning_outcome.final_planning_readiness_status {
            PlanningReadinessStatus::Ready if planning_outcome.analyze_pass_count == 0 => None,
            state => Some(state),
        },
        analyze_pass_count: if planning_outcome.analyze_pass_count == 0 {
            None
        } else {
            Some(planning_outcome.analyze_pass_count)
        },
        remediation_cycles_used: if planning_outcome.analyze_pass_count == 0 {
            None
        } else {
            Some(planning_outcome.remediation_cycles_used)
        },
        implementation_status,
        validation_refs,
        failure_class: None,
        next_action,
    }
}

fn execute_planning_readiness_bridge(
    request: &ExecuteStageRequest,
    workspace_path: &Path,
    executed_commands: &mut Vec<String>,
) -> Result<PlanningStageOutcome, SpeckitProfileError> {
    let fixture = load_planning_readiness_fixture(&request.workspace_ref, workspace_path)?;
    if fixture.analyze_passes.is_empty() {
        executed_commands.push(SPECKIT_ANALYZE_COMMAND.to_string());
        return Ok(PlanningStageOutcome {
            final_planning_readiness_status: PlanningReadinessStatus::Ready,
            analyze_pass_count: 1,
            remediation_cycles_used: 0,
            summary: PLAN_STAGE_SUCCESS_SUMMARY.to_string(),
            status: STATUS_SUCCEEDED.to_string(),
            ..PlanningStageOutcome::default()
        });
    }

    let mut outcome = PlanningStageOutcome {
        final_planning_readiness_status: PlanningReadinessStatus::Blocked,
        summary: PLAN_STAGE_BLOCKED_SUMMARY.to_string(),
        status: STATUS_BLOCKED.to_string(),
        ..PlanningStageOutcome::default()
    };

    for (pass_index, analyze_pass) in fixture
        .analyze_passes
        .iter()
        .take(MAX_PLAN_ANALYZE_PASSES)
        .enumerate()
    {
        executed_commands.push(SPECKIT_ANALYZE_COMMAND.to_string());
        outcome.analyze_pass_count += 1;

        let pass_findings = planning_findings_from_fixture(analyze_pass);
        for finding in &pass_findings {
            push_unique_finding(&mut outcome.planning_findings, finding.clone());
        }

        let blocking_findings = blocking_planning_findings(&pass_findings);
        if blocking_findings.is_empty() {
            outcome.remaining_blocking_findings.clear();
            outcome.final_planning_readiness_status = PlanningReadinessStatus::Ready;
            outcome.summary = PLAN_STAGE_READY_SUMMARY.to_string();
            outcome.status = STATUS_SUCCEEDED.to_string();
            outcome.next_action = None;
            return Ok(outcome);
        }

        if pass_index >= MAX_PLAN_REMEDIATION_CYCLES {
            outcome.remaining_blocking_findings = blocking_findings;
            outcome.summary = PLAN_STAGE_LIMIT_SUMMARY.to_string();
            outcome.next_action = Some(PLAN_STAGE_RETRY_ACTION.to_string());
            return Ok(outcome);
        }

        let remediation_result = execute_planning_remediation_tasks(
            request,
            workspace_path,
            analyze_pass,
            executed_commands,
            &blocking_findings,
        )?;
        outcome
            .remediation_tasks_attempted
            .extend(remediation_result.attempted);
        outcome
            .remediation_tasks_completed
            .extend(remediation_result.completed);
        outcome
            .remediation_tasks_skipped
            .extend(remediation_result.skipped);

        if remediation_result.stop_blocked {
            outcome.remaining_blocking_findings = blocking_findings;
            outcome.remediation_cycles_used += remediation_result.completed_cycles;
            outcome.summary = PLAN_STAGE_UNRESOLVED_SUMMARY.to_string();
            outcome.next_action = Some(PLAN_STAGE_RETRY_ACTION.to_string());
            return Ok(outcome);
        }

        outcome.remediation_cycles_used += remediation_result.completed_cycles;
        if pass_index + 1 >= fixture.analyze_passes.len() {
            outcome.remaining_blocking_findings = blocking_findings;
            outcome.summary = PLAN_STAGE_LIMIT_SUMMARY.to_string();
            outcome.next_action = Some(PLAN_STAGE_RETRY_ACTION.to_string());
            return Ok(outcome);
        }
    }

    if outcome.analyze_pass_count == 0 {
        executed_commands.push(SPECKIT_ANALYZE_COMMAND.to_string());
        outcome.analyze_pass_count = 1;
    }
    outcome.summary = PLAN_STAGE_LIMIT_SUMMARY.to_string();
    outcome.next_action = Some(PLAN_STAGE_RETRY_ACTION.to_string());
    Ok(outcome)
}

#[derive(Debug, Default)]
struct PlanningRemediationExecutionResult {
    attempted: Vec<PlanningRemediationTaskOutcome>,
    completed: Vec<PlanningRemediationTaskOutcome>,
    skipped: Vec<PlanningRemediationTaskOutcome>,
    completed_cycles: usize,
    stop_blocked: bool,
}

fn execute_planning_remediation_tasks(
    request: &ExecuteStageRequest,
    workspace_path: &Path,
    analyze_pass: &PlanningAnalyzePassFixture,
    executed_commands: &mut Vec<String>,
    blocking_findings: &[PlanningFinding],
) -> Result<PlanningRemediationExecutionResult, SpeckitProfileError> {
    let mut result = PlanningRemediationExecutionResult::default();
    let mut actionable_task_found = false;

    for remediation_task in &analyze_pass.remediation_tasks {
        let task_outcome = remediation_outcome(remediation_task);
        if let Some(skip_reason) = remediation_skip_reason(remediation_task) {
            result.skipped.push(PlanningRemediationTaskOutcome {
                skip_reason: Some(skip_reason),
                ..task_outcome
            });
            continue;
        }

        let Some(command) = remediation_task.command.as_ref() else {
            result.skipped.push(PlanningRemediationTaskOutcome {
                skip_reason: Some(PlanningRemediationSkipReason::MissingCommand),
                ..task_outcome
            });
            continue;
        };

        actionable_task_found = true;
        let command_display = remediation_command_display(command);
        executed_commands.push(command_display.clone());
        result.attempted.push(task_outcome.clone());
        let command_output = run_bridge_command(
            &request.workspace_ref,
            workspace_path,
            &command_display,
            &command.program,
            &command.args,
        )?;

        if command_output.status.success() {
            result.completed.push(task_outcome);
            continue;
        }

        result.stop_blocked = true;
        return Ok(result);
    }

    if actionable_task_found {
        result.completed_cycles = 1;
    }
    if !actionable_task_found || !all_blocking_findings_covered(blocking_findings, analyze_pass) {
        result.stop_blocked = true;
    }
    Ok(result)
}

fn load_planning_readiness_fixture(
    workspace_ref: &str,
    workspace_path: &Path,
) -> Result<PlanningReadinessFixture, SpeckitProfileError> {
    let fixture_path = fixture_path(workspace_ref, PLANNING_READINESS_FIXTURE_FILE_NAME);
    if !fixture_path.is_file() {
        return Ok(PlanningReadinessFixture::default());
    }

    let fixture_bytes =
        fs::read(&fixture_path).map_err(|source| SpeckitProfileError::RunBridgeCommand {
            workspace_ref: workspace_ref.to_string(),
            command: fixture_path.to_string_lossy().into_owned(),
            source,
        })?;
    serde_json::from_slice::<PlanningReadinessFixture>(&fixture_bytes).map_err(|source| {
        SpeckitProfileError::ParseBridgeJson {
            workspace_ref: workspace_path.to_string_lossy().into_owned(),
            command: fixture_path.to_string_lossy().into_owned(),
            source,
        }
    })
}

fn planning_readiness_fixture_exists(workspace_ref: &str) -> bool {
    fixture_path(workspace_ref, PLANNING_READINESS_FIXTURE_FILE_NAME).is_file()
}

fn planning_findings_from_fixture(
    analyze_pass: &PlanningAnalyzePassFixture,
) -> Vec<PlanningFinding> {
    analyze_pass
        .findings
        .iter()
        .map(|finding| PlanningFinding {
            finding_id: finding.finding_id.clone(),
            summary: finding.summary.clone(),
            severity: finding.severity,
        })
        .collect()
}

fn blocking_planning_findings(findings: &[PlanningFinding]) -> Vec<PlanningFinding> {
    findings
        .iter()
        .filter(|finding| finding.severity == PlanningFindingSeverity::Blocking)
        .cloned()
        .collect()
}

fn push_unique_finding(findings: &mut Vec<PlanningFinding>, candidate: PlanningFinding) {
    if findings
        .iter()
        .any(|existing| existing.finding_id == candidate.finding_id)
    {
        return;
    }
    findings.push(candidate);
}

fn remediation_skip_reason(
    remediation_task: &PlanningRemediationTaskFixture,
) -> Option<PlanningRemediationSkipReason> {
    if !remediation_task.in_scope {
        return Some(PlanningRemediationSkipReason::OutOfScope);
    }
    if !remediation_task.safe {
        return Some(PlanningRemediationSkipReason::Unsafe);
    }
    if remediation_task.requires_operator_input {
        return Some(PlanningRemediationSkipReason::RequiresOperatorInput);
    }
    if !remediation_task.deterministic {
        return Some(PlanningRemediationSkipReason::NonDeterministic);
    }
    None
}

fn remediation_outcome(
    remediation_task: &PlanningRemediationTaskFixture,
) -> PlanningRemediationTaskOutcome {
    PlanningRemediationTaskOutcome {
        task_id: remediation_task.task_id.clone(),
        summary: remediation_task.summary.clone(),
        finding_ids: remediation_task.finding_ids.clone(),
        skip_reason: None,
    }
}

fn remediation_command_display(command: &PlanningRemediationCommandFixture) -> String {
    if command.args.is_empty() {
        return command.program.clone();
    }
    format!("{} {}", command.program, command.args.join(" "))
}

fn all_blocking_findings_covered(
    blocking_findings: &[PlanningFinding],
    analyze_pass: &PlanningAnalyzePassFixture,
) -> bool {
    blocking_findings.iter().all(|finding| {
        analyze_pass.remediation_tasks.iter().any(|task| {
            task.finding_ids
                .iter()
                .any(|finding_id| finding_id == &finding.finding_id)
                && remediation_skip_reason(task).is_none()
                && task.command.is_some()
        })
    })
}

fn default_true() -> bool {
    true
}

fn run_bridge_command(
    workspace_ref: &str,
    workspace_path: &Path,
    command_display: &str,
    program: &str,
    args: &[String],
) -> Result<Output, SpeckitProfileError> {
    Command::new(program)
        .current_dir(workspace_path)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|source| SpeckitProfileError::RunBridgeCommand {
            workspace_ref: workspace_ref.to_string(),
            command: command_display.to_string(),
            source,
        })
}

fn resolved_specify_program() -> String {
    std::env::var(SPECIFY_PROGRAM_ENV_VAR).unwrap_or_else(|_| SPECIFY_PROGRAM.to_string())
}

fn parse_bridge_json<T>(
    workspace_ref: &str,
    command_display: &str,
    output: &Output,
) -> Result<T, SpeckitProfileError>
where
    T: DeserializeOwned,
{
    serde_json::from_slice(&output.stdout).map_err(|source| SpeckitProfileError::ParseBridgeJson {
        workspace_ref: workspace_ref.to_string(),
        command: command_display.to_string(),
        source,
    })
}

fn plan_stage_artifacts(
    workspace_path: &Path,
    setup_plan: &SetupPlanResult,
    prerequisites: &CheckPrerequisitesResult,
) -> Vec<String> {
    let mut artifacts = Vec::new();
    let feature_dir = Path::new(&prerequisites.feature_dir);
    let feature_spec_path = feature_dir.join(FEATURE_SPEC_FILE_NAME);
    if feature_spec_path.is_file() {
        push_artifact_ref(&mut artifacts, workspace_path, &feature_spec_path);
    }
    push_artifact_ref(
        &mut artifacts,
        workspace_path,
        Path::new(&setup_plan.impl_plan),
    );
    if prerequisites
        .available_docs
        .iter()
        .any(|doc| doc == TASKS_FILE_NAME)
    {
        push_artifact_ref(
            &mut artifacts,
            workspace_path,
            &feature_dir.join(TASKS_FILE_NAME),
        );
    }
    if prerequisites
        .available_docs
        .iter()
        .any(|doc| doc == ANALYSIS_FILE_NAME)
    {
        push_artifact_ref(
            &mut artifacts,
            workspace_path,
            &feature_dir.join(ANALYSIS_FILE_NAME),
        );
    }
    let planning_workflow_path = workspace_path.join(PLANNING_WORKFLOW_FILE_REF);
    if planning_workflow_path.is_file() {
        push_artifact_ref(&mut artifacts, workspace_path, &planning_workflow_path);
    }

    artifacts
}

fn run_stage_artifacts(
    workspace_path: &Path,
    prerequisites: &CheckPrerequisitesResult,
) -> Vec<String> {
    let mut artifacts = Vec::new();
    let feature_dir = Path::new(&prerequisites.feature_dir);
    push_artifact_ref(
        &mut artifacts,
        workspace_path,
        &feature_dir.join(PLAN_FILE_NAME),
    );
    push_artifact_ref(
        &mut artifacts,
        workspace_path,
        &feature_dir.join(TASKS_FILE_NAME),
    );
    push_artifact_ref(
        &mut artifacts,
        workspace_path,
        &workspace_path.join(resolved_implementation_workflow_file_ref(workspace_path)),
    );

    let implement_prompt_path = workspace_path.join(IMPLEMENT_PROMPT_REF);
    if implement_prompt_path.is_file() {
        push_artifact_ref(&mut artifacts, workspace_path, &implement_prompt_path);
    }

    artifacts
}

fn push_artifact_ref(artifacts: &mut Vec<String>, workspace_path: &Path, path: &Path) {
    let artifact_ref = relative_artifact_ref(workspace_path, path);
    if !artifacts.contains(&artifact_ref) {
        artifacts.push(artifact_ref);
    }
}

fn relative_artifact_ref(workspace_path: &Path, path: &Path) -> String {
    if let (Ok(canonical_workspace), Ok(canonical_path)) =
        (fs::canonicalize(workspace_path), fs::canonicalize(path))
        && let Ok(stripped) = canonical_path.strip_prefix(&canonical_workspace)
    {
        return stripped.to_string_lossy().into_owned();
    }

    path.strip_prefix(workspace_path)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

fn workflow_spec_input(workspace_path: &Path, prerequisites: &CheckPrerequisitesResult) -> String {
    let feature_spec_path = Path::new(&prerequisites.feature_dir).join(FEATURE_SPEC_FILE_NAME);
    fs::read_to_string(&feature_spec_path)
        .ok()
        .and_then(|contents| {
            contents
                .lines()
                .map(str::trim)
                .find_map(|line| line.strip_prefix("# ").map(str::trim))
                .filter(|heading| !heading.is_empty())
                .map(str::to_string)
        })
        .or_else(|| {
            feature_spec_path
                .strip_prefix(workspace_path)
                .ok()
                .and_then(|path| path.parent())
                .and_then(|path| path.file_name())
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "current Speckit feature packet".to_string())
}

fn parse_workflow_run_status(output: &Output) -> WorkflowRunStatus {
    let mut status = WorkflowRunStatus::default();
    let combined_output = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    for line in combined_output.lines().map(str::trim) {
        if let Some(value) = line.strip_prefix(WORKFLOW_STATUS_PREFIX) {
            status.status = Some(value.trim().to_lowercase());
        } else if let Some(value) = line.strip_prefix(WORKFLOW_RUN_ID_PREFIX) {
            status.run_id = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix(WORKFLOW_RESUME_PREFIX) {
            status.resume_command = Some(value.trim().to_string());
        }
    }

    status
}

fn workflow_pause_reason(
    workspace_path: &Path,
    workflow_file_ref: &str,
    output: &Output,
) -> String {
    let workflow_path = workspace_path.join(workflow_file_ref);
    let workflow_requires_gate = fs::read_to_string(&workflow_path)
        .ok()
        .is_some_and(|contents| contents.contains(WORKFLOW_GATE_MARKER));
    let command_summary = command_output_summary(output, "workflow paused before completion");

    if workflow_requires_gate {
        format!(
            "the installed Speckit workflow contains interactive review gates; {command_summary}"
        )
    } else {
        command_summary
    }
}

fn workflow_resume_or_recovery_command(
    workflow_status: &WorkflowRunStatus,
    workflow_source: &str,
    workflow_spec_input: &str,
) -> Option<String> {
    workflow_status.resume_command.clone().or_else(|| {
        workflow_status.run_id.as_ref().map(|run_id| {
            format!("{SPECIFY_PROGRAM} workflow resume {run_id}")
        })
    }).or_else(|| {
        Some(format!(
            "{SPECIFY_PROGRAM} workflow run {workflow_source} {WORKFLOW_INPUT_FLAG} {WORKFLOW_SPEC_INPUT_KEY}={workflow_spec_input}"
        ))
    })
}

fn resolved_implementation_workflow_file_ref(workspace_path: &Path) -> &'static str {
    let _ = workspace_path;
    IMPLEMENTATION_WORKFLOW_FILE_REF
}

fn command_output_summary(output: &Output, default_summary: &str) -> String {
    first_non_empty_line(&output.stderr)
        .or_else(|| first_non_empty_line(&output.stdout))
        .unwrap_or_else(|| default_summary.to_string())
}

fn first_non_empty_line(bytes: &[u8]) -> Option<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
}

fn fixture_path(workspace_ref: &str, file_name: &str) -> PathBuf {
    Path::new(workspace_ref)
        .join(FIXTURE_DIRECTORY_NAME)
        .join(file_name)
}
