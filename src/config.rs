//! Configuration schema and preflight normalization for the Speckit scaffold.

use crate::profile::{
    ADAPTER_REPO_FIELD_KEY, ConfigFieldDefinition, ConfigValue, ConfigValueKind, PreflightRequest,
    PreflightResponse, SUPPORTED_BOUNDLINE_RANGE, TEMPLATE_REPO_FIELD_KEY,
};
use semver::{Version, VersionReq};

const STATUS_READY: &str = "ready";
const STATUS_BLOCKED: &str = "blocked";
const REASON_MISSING_REQUIRED_CONFIG: &str = "missing_required_config";
const REASON_UNSUPPORTED_BOUNDLINE_VERSION: &str = "unsupported_boundline_version";
const RECOVERY_TEMPLATE: &str = "boundline adapter add speckit --workspace <workspace>";
const RECOVERY_SUPPORTED_BOUNDLINE_VERSION: &str = "use a Boundline version in >=0.90.0,<1.0.0";
const NON_INTERACTIVE_POLICY_FAIL: &str = "fail";
const TEMPLATE_REPO_DISPLAY_LABEL: &str = "Template repository";
const TEMPLATE_REPO_PROMPT: &str = "Path to the reusable template repo";
const TEMPLATE_REPO_HELP: &str =
    "Point this at ../boundline-framework-template or another checked-out template repo";
const ADAPTER_REPO_DISPLAY_LABEL: &str = "Adapter repository";
const ADAPTER_REPO_PROMPT: &str = "Path to the Speckit adapter repo";
const ADAPTER_REPO_HELP: &str =
    "Point this at ../boundline-adapter-speckit or another checked-out Speckit repo";

/// Returns the required config schema exposed by Speckit `describe`.
pub(crate) fn required_config_fields() -> Vec<ConfigFieldDefinition> {
    vec![
        required_path_field(
            TEMPLATE_REPO_FIELD_KEY,
            TEMPLATE_REPO_DISPLAY_LABEL,
            TEMPLATE_REPO_PROMPT,
            TEMPLATE_REPO_HELP,
        ),
        required_path_field(
            ADAPTER_REPO_FIELD_KEY,
            ADAPTER_REPO_DISPLAY_LABEL,
            ADAPTER_REPO_PROMPT,
            ADAPTER_REPO_HELP,
        ),
    ]
}

/// Validates and normalizes the Speckit preflight config payload.
pub(crate) fn preflight_response_for_request(request: &PreflightRequest) -> PreflightResponse {
    if !supports_boundline_version(&request.boundline_version) {
        return unsupported_boundline_version_response();
    }

    match SpeckitResolvedConfig::from_values(&request.config_values) {
        Ok(config) => PreflightResponse {
            status: STATUS_READY.to_string(),
            normalized_config_values: config.normalized_config_values(),
            warnings: Vec::new(),
            reason: None,
            missing_fields: Vec::new(),
            recovery: None,
        },
        Err(missing_fields) => PreflightResponse {
            status: STATUS_BLOCKED.to_string(),
            normalized_config_values: Vec::new(),
            warnings: Vec::new(),
            reason: Some(REASON_MISSING_REQUIRED_CONFIG.to_string()),
            missing_fields,
            recovery: Some(RECOVERY_TEMPLATE.to_string()),
        },
    }
}

fn supports_boundline_version(version: &str) -> bool {
    let Ok(version) = Version::parse(version) else {
        return false;
    };
    let Ok(requirement) = VersionReq::parse(SUPPORTED_BOUNDLINE_RANGE) else {
        return false;
    };
    requirement.matches(&version)
}

fn unsupported_boundline_version_response() -> PreflightResponse {
    PreflightResponse {
        status: STATUS_BLOCKED.to_string(),
        normalized_config_values: Vec::new(),
        warnings: Vec::new(),
        reason: Some(REASON_UNSUPPORTED_BOUNDLINE_VERSION.to_string()),
        missing_fields: Vec::new(),
        recovery: Some(RECOVERY_SUPPORTED_BOUNDLINE_VERSION.to_string()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SpeckitResolvedConfig {
    template_repo: String,
    adapter_repo: String,
}

impl SpeckitResolvedConfig {
    fn from_values(config_values: &[ConfigValue]) -> Result<Self, Vec<String>> {
        let template_repo = normalized_required_path_value(config_values, TEMPLATE_REPO_FIELD_KEY);
        let adapter_repo = normalized_required_path_value(config_values, ADAPTER_REPO_FIELD_KEY);

        let mut missing_fields = Vec::new();
        if template_repo.is_none() {
            missing_fields.push(TEMPLATE_REPO_FIELD_KEY.to_string());
        }
        if adapter_repo.is_none() {
            missing_fields.push(ADAPTER_REPO_FIELD_KEY.to_string());
        }

        if !missing_fields.is_empty() {
            return Err(missing_fields);
        }

        Ok(Self {
            template_repo: template_repo.unwrap_or_default(),
            adapter_repo: adapter_repo.unwrap_or_default(),
        })
    }

    fn normalized_config_values(&self) -> Vec<ConfigValue> {
        vec![
            path_value(TEMPLATE_REPO_FIELD_KEY, &self.template_repo),
            path_value(ADAPTER_REPO_FIELD_KEY, &self.adapter_repo),
        ]
    }
}

fn required_path_field(
    field_key: &str,
    display_label: &str,
    prompt_text: &str,
    help_text: &str,
) -> ConfigFieldDefinition {
    ConfigFieldDefinition {
        field_key: field_key.to_string(),
        display_label: display_label.to_string(),
        value_kind: ConfigValueKind::Path,
        required: true,
        secret: false,
        prompt_text: prompt_text.to_string(),
        help_text: help_text.to_string(),
        non_interactive_policy: NON_INTERACTIVE_POLICY_FAIL.to_string(),
    }
}

fn normalized_required_path_value(
    config_values: &[ConfigValue],
    field_key: &str,
) -> Option<String> {
    config_values.iter().find_map(|value| {
        (value.field_key == field_key && value.value_kind == ConfigValueKind::Path)
            .then(|| {
                value
                    .path_value
                    .as_deref()
                    .map(str::trim)
                    .filter(|path| !path.is_empty())
                    .map(str::to_string)
            })
            .flatten()
    })
}

fn path_value(field_key: &str, path: &str) -> ConfigValue {
    ConfigValue {
        field_key: field_key.to_string(),
        value_kind: ConfigValueKind::Path,
        string_value: None,
        path_value: Some(path.to_string()),
        bool_value: None,
        int_value: None,
    }
}
