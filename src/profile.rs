//! Typed Speckit profile contract models and scaffold behavior.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config::{preflight_response_for_request, required_config_fields};

/// Stable adapter identifier used by the host known-profile registry.
pub const ADAPTER_ID: &str = "speckit";

/// Default executable name resolved by the host profile.
pub const BINARY_NAME: &str = "boundline-adapter-speckit";

/// Stable protocol line implemented by the Speckit scaffold.
pub const CONTRACT_LINE: &str = "framework-adapter-v1";

/// Current maturity marker for the Speckit workflow bridge.
pub const BOOTSTRAP_STATUS: &str = "workflow-bridge";

/// Supported Boundline compatibility range for the Speckit scaffold.
pub const SUPPORTED_BOUNDLINE_RANGE: &str = ">=0.90.0,<1.0.0";

/// Required template repository field key.
pub const TEMPLATE_REPO_FIELD_KEY: &str = "template_repo";

/// Required adapter repository field key.
pub const ADAPTER_REPO_FIELD_KEY: &str = "adapter_repo";

const PLAN_STAGE_KEY: &str = "plan";
const RUN_STAGE_KEY: &str = "run";
const STAGE_COMPLETED_HOOK_KEY: &str = "stage_completed";
const STAGE_FAILED_HOOK_KEY: &str = "stage_failed";
const STDIO_TRANSPORT: &str = "stdio";
const JSON_ENCODING: &str = "json";
const STDIN_CHANNEL: &str = "stdin";
const STDOUT_CHANNEL: &str = "stdout";

/// Error returned when the workflow bridge cannot complete its local stage work.
#[derive(Debug, Error)]
pub enum SpeckitProfileError {
    #[error("failed to run bridge command `{command}` in `{workspace_ref}`: {source}")]
    RunBridgeCommand {
        workspace_ref: String,
        command: String,
        #[source]
        source: std::io::Error,
    },
    #[error("bridge command `{command}` in `{workspace_ref}` returned invalid JSON: {source}")]
    ParseBridgeJson {
        workspace_ref: String,
        command: String,
        #[source]
        source: serde_json::Error,
    },
}

/// Minimal metadata exported by the Speckit workflow bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpeckitBootstrapMetadata {
    /// Stable host-visible adapter identifier.
    pub adapter_id: &'static str,
    /// Default binary name that the host resolves for the known profile.
    pub binary_name: &'static str,
    /// Host-owned protocol line the adapter implements.
    pub contract_line: &'static str,
    /// Current workflow-bridge maturity marker.
    pub bootstrap_status: &'static str,
}

impl SpeckitBootstrapMetadata {
    /// Creates the metadata value for the Speckit workflow bridge.
    pub const fn new() -> Self {
        Self {
            adapter_id: ADAPTER_ID,
            binary_name: BINARY_NAME,
            contract_line: CONTRACT_LINE,
            bootstrap_status: BOOTSTRAP_STATUS,
        }
    }
}

impl Default for SpeckitBootstrapMetadata {
    fn default() -> Self {
        Self::new()
    }
}

/// Supported config value kinds for Speckit protocol payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigValueKind {
    /// Arbitrary text.
    String,
    /// Filesystem path.
    Path,
    /// Boolean value.
    Boolean,
    /// Integer value.
    Integer,
    /// Closed enum choice.
    Enum,
}

/// Config value carried through Speckit preflight and stage execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigValue {
    /// Stable field identifier.
    pub field_key: String,
    /// Declared value kind.
    pub value_kind: ConfigValueKind,
    /// Optional string payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub string_value: Option<String>,
    /// Optional path payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path_value: Option<String>,
    /// Optional boolean payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bool_value: Option<bool>,
    /// Optional integer payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub int_value: Option<i64>,
}

/// Config field definition emitted by Speckit `describe`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigFieldDefinition {
    /// Stable field identifier.
    pub field_key: String,
    /// Operator-facing display label.
    pub display_label: String,
    /// Declared field kind.
    pub value_kind: ConfigValueKind,
    /// Whether the field is required.
    pub required: bool,
    /// Whether the field is secret.
    pub secret: bool,
    /// Prompt text used by guided setup.
    pub prompt_text: String,
    /// Help text used by guided setup.
    pub help_text: String,
    /// Non-interactive policy for the field.
    pub non_interactive_policy: String,
}

/// One declared transport supported by the Speckit scaffold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportDescriptor {
    /// Transport family name.
    pub transport: String,
    /// Encoding carried over the transport.
    pub encoding: String,
    /// Request channel identifier.
    pub request_channel: String,
    /// Response channel identifier.
    pub response_channel: String,
}

/// `describe` response emitted by the Speckit scaffold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DescribeResponse {
    /// Stable protocol line implemented by the adapter.
    pub protocol_line: String,
    /// Stable adapter identifier.
    pub adapter_id: String,
    /// Adapter version string.
    pub adapter_version: String,
    /// Supported Boundline version range.
    pub supported_boundline_range: String,
    /// Explicit transports supported by this adapter build.
    pub supported_transports: Vec<TransportDescriptor>,
    /// Declared stage overrides.
    pub declared_stage_overrides: Vec<String>,
    /// Declared hook subscriptions.
    pub declared_hook_subscriptions: Vec<String>,
    /// Required config schema.
    pub required_config_fields: Vec<ConfigFieldDefinition>,
}

/// `preflight` request accepted by the Speckit scaffold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreflightRequest {
    /// Host Boundline version.
    pub boundline_version: String,
    /// Target workspace path.
    pub workspace_ref: String,
    /// Whether the host forbids interactive recovery.
    pub non_interactive: bool,
    /// Proposed config values.
    #[serde(default)]
    pub config_values: Vec<ConfigValue>,
}

/// `preflight` response emitted by the Speckit scaffold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreflightResponse {
    /// Preflight status.
    pub status: String,
    /// Normalized config values.
    #[serde(default)]
    pub normalized_config_values: Vec<ConfigValue>,
    /// Non-blocking warnings.
    #[serde(default)]
    pub warnings: Vec<String>,
    /// Blocking reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Missing field keys.
    #[serde(default)]
    pub missing_fields: Vec<String>,
    /// Recovery instruction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery: Option<String>,
}

/// `execute-stage` request accepted by the Speckit scaffold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecuteStageRequest {
    /// Stable run identifier.
    pub run_id: String,
    /// Host-owned stage key.
    pub stage_key: String,
    /// Attempt number for the current stage.
    pub stage_attempt: usize,
    /// Target workspace path.
    pub workspace_ref: String,
    /// Adapter identifier selected by the host.
    pub adapter_id: String,
    /// Resolved config values.
    #[serde(default)]
    pub config_values: Vec<ConfigValue>,
    /// Context artifact refs passed by the host.
    #[serde(default)]
    pub context_artifacts: Vec<String>,
}

/// `execute-stage` response emitted by the Speckit scaffold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecuteStageResponse {
    /// Stage terminal status.
    pub status: String,
    /// Operator-facing summary.
    pub summary: String,
    /// Produced artifacts or refs.
    #[serde(default)]
    pub produced_artifacts: Vec<String>,
    /// Workflow identifier executed for the stage claim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<String>,
    /// Commands or bridge steps executed during the stage.
    #[serde(default)]
    pub executed_commands: Vec<String>,
    /// Findings surfaced during planning readiness validation.
    #[serde(default)]
    pub planning_findings: Vec<PlanningFinding>,
    /// Remediation tasks attempted for blocking planning findings.
    #[serde(default)]
    pub remediation_tasks_attempted: Vec<PlanningRemediationTaskOutcome>,
    /// Remediation tasks completed successfully.
    #[serde(default)]
    pub remediation_tasks_completed: Vec<PlanningRemediationTaskOutcome>,
    /// Remediation tasks skipped with an explicit reason.
    #[serde(default)]
    pub remediation_tasks_skipped: Vec<PlanningRemediationTaskOutcome>,
    /// Blocking planning findings still unresolved when the stage returned.
    #[serde(default)]
    pub remaining_blocking_findings: Vec<PlanningFinding>,
    /// Final readiness posture for the planning stage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_planning_readiness_status: Option<PlanningReadinessStatus>,
    /// Count of analyze passes observed during the stage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analyze_pass_count: Option<usize>,
    /// Count of remediation cycles used before the stage returned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remediation_cycles_used: Option<usize>,
    /// Implementation-stage outcome detail when `run` owns execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub implementation_status: Option<ImplementationWorkflowStatus>,
    /// Validation or evidence refs produced during stage execution.
    #[serde(default)]
    pub validation_refs: Vec<String>,
    /// Failure classification when the claimed stage does not succeed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_class: Option<String>,
    /// Next action when the host should continue or inspect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_action: Option<String>,
}

/// Readiness posture of the planning bridge after Speckit analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PlanningReadinessStatus {
    /// Planning can continue because blocking findings were cleared.
    Ready,
    /// Planning must stop because blocking findings remain.
    #[default]
    Blocked,
}

/// Severity attached to one planning finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanningFindingSeverity {
    /// The finding blocks planning completion.
    Blocking,
    /// The finding is informational or advisory only.
    NonBlocking,
}

/// Stable record for one planning finding surfaced by Speckit analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanningFinding {
    /// Stable finding identifier.
    pub finding_id: String,
    /// Short operator-facing summary.
    pub summary: String,
    /// Severity used by the planning gate.
    pub severity: PlanningFindingSeverity,
}

/// Skip reasons for one remediation task the bridge intentionally did not run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanningRemediationSkipReason {
    /// The remediation is outside the current feature packet scope.
    OutOfScope,
    /// The remediation would be unsafe to apply automatically.
    Unsafe,
    /// The remediation requires human input or approval.
    RequiresOperatorInput,
    /// The remediation is not deterministic enough for automatic execution.
    NonDeterministic,
    /// The task had no executable command attached.
    MissingCommand,
}

/// Outcome record for one attempted or skipped remediation task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanningRemediationTaskOutcome {
    /// Stable remediation task identifier.
    pub task_id: String,
    /// Short operator-facing summary.
    pub summary: String,
    /// Findings this remediation addresses.
    #[serde(default)]
    pub finding_ids: Vec<String>,
    /// Explicit skip reason when the task was not executed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip_reason: Option<PlanningRemediationSkipReason>,
}

/// Final implementation-stage status surfaced by the run bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImplementationWorkflowStatus {
    /// The implementation workflow completed successfully.
    Completed,
    /// The implementation workflow paused and awaits continuation.
    Blocked,
    /// The implementation workflow failed.
    Failed,
}

/// `emit-hook` request accepted by the Speckit scaffold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmitHookRequest {
    /// Stable run identifier.
    pub run_id: String,
    /// Host-owned hook key.
    pub hook_key: String,
    /// Related stage key.
    pub stage_key: String,
    /// Whether the stage was adapter-claimed.
    pub stage_claimed: bool,
    /// Target workspace path.
    pub workspace_ref: String,
    /// Trace payload ref.
    pub payload_ref: String,
}

/// `emit-hook` response emitted by the Speckit scaffold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmitHookResponse {
    /// Hook delivery status.
    pub status: String,
    /// Operator-facing summary.
    pub summary: String,
}

/// Standard success envelope used for stdout responses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuccessEnvelope<T> {
    /// Indicates the protocol exchange succeeded.
    pub success: bool,
    /// Command-specific response payload.
    pub data: T,
}

/// Returns the metadata exposed by the Speckit workflow bridge crate.
pub const fn bootstrap_metadata() -> SpeckitBootstrapMetadata {
    SpeckitBootstrapMetadata::new()
}

/// Renders a stable status line for the workflow bridge executable.
pub fn bootstrap_status_line() -> String {
    let metadata = bootstrap_metadata();
    format!(
        "{} adapter scaffold initialized for {} ({})",
        metadata.binary_name, metadata.contract_line, metadata.bootstrap_status
    )
}

/// Builds the stable `describe` response for the Speckit workflow bridge.
pub fn describe_response() -> DescribeResponse {
    DescribeResponse {
        protocol_line: CONTRACT_LINE.to_string(),
        adapter_id: ADAPTER_ID.to_string(),
        adapter_version: env!("CARGO_PKG_VERSION").to_string(),
        supported_boundline_range: SUPPORTED_BOUNDLINE_RANGE.to_string(),
        supported_transports: vec![stdio_json_transport()],
        declared_stage_overrides: vec![PLAN_STAGE_KEY.to_string(), RUN_STAGE_KEY.to_string()],
        declared_hook_subscriptions: vec![
            STAGE_COMPLETED_HOOK_KEY.to_string(),
            STAGE_FAILED_HOOK_KEY.to_string(),
        ],
        required_config_fields: required_config_fields(),
    }
}

/// Validates the required Speckit config values and returns a preflight result.
pub fn preflight_response(request: &PreflightRequest) -> PreflightResponse {
    preflight_response_for_request(request)
}

/// Executes the claimed-stage workflow bridge for plan and run.
pub fn execute_stage_response(
    request: &ExecuteStageRequest,
) -> Result<ExecuteStageResponse, SpeckitProfileError> {
    crate::stages::execute_stage_response(request)
}

/// Builds a stable delivered response for a subscribed Speckit hook.
pub fn emit_hook_response(request: &EmitHookRequest) -> EmitHookResponse {
    crate::hooks::emit_hook_response(request)
}

/// Wraps one response payload in the standard success envelope.
pub fn success_envelope<T>(data: T) -> SuccessEnvelope<T> {
    SuccessEnvelope {
        success: true,
        data,
    }
}

fn stdio_json_transport() -> TransportDescriptor {
    TransportDescriptor {
        transport: STDIO_TRANSPORT.to_string(),
        encoding: JSON_ENCODING.to_string(),
        request_channel: STDIN_CHANNEL.to_string(),
        response_channel: STDOUT_CHANNEL.to_string(),
    }
}
