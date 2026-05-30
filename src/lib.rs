//! Known Speckit adapter scaffold for the Boundline framework-adapter contract.

pub use boundline_adapters::{framework_catalog, framework_protocol};

mod config;
mod hooks;
pub mod profile;
mod stages;

pub use profile::{
    ADAPTER_ID, ADAPTER_REPO_FIELD_KEY, BINARY_NAME, BOOTSTRAP_STATUS, CONTRACT_LINE,
    ConfigFieldDefinition, ConfigValue, ConfigValueKind, DescribeResponse, EmitHookRequest,
    EmitHookResponse, ExecuteStageRequest, ExecuteStageResponse, PreflightRequest,
    PreflightResponse, SUPPORTED_BOUNDLINE_RANGE, SpeckitBootstrapMetadata, SpeckitProfileError,
    SuccessEnvelope, TEMPLATE_REPO_FIELD_KEY, TransportDescriptor, bootstrap_metadata,
    bootstrap_status_line, describe_response, emit_hook_response, execute_stage_response,
    preflight_response, success_envelope,
};
