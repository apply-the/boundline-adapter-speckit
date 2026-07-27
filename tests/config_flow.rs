use boundline_adapter_speckit::{
    ADAPTER_REPO_FIELD_KEY, ConfigValue, ConfigValueKind, PreflightRequest,
    TEMPLATE_REPO_FIELD_KEY, describe_response, preflight_response,
};

const TEMPLATE_REPO_PATH: &str = "../boundline-framework-template";
const ADAPTER_REPO_PATH: &str = "../boundline-adapter-speckit";
const QUALIFIED_BOUNDLINE_VERSION: &str = "0.90.0";
const FUTURE_PRERELEASE_VERSION: &str = "0.99.7";
const PRE_QUALIFICATION_VERSION: &str = "0.89.9";
const STABLE_VERSION: &str = "1.0.0";
const MALFORMED_VERSION: &str = "0.90";
const UNSUPPORTED_VERSION_REASON: &str = "unsupported_boundline_version";
const VERSION_RECOVERY: &str = "use a Boundline version in >=0.90.0,<1.0.0";

#[test]
fn describe_response_declares_required_speckit_path_fields() {
    let describe = describe_response();

    assert_eq!(describe.required_config_fields.len(), 2);
    assert_eq!(
        describe.required_config_fields[0].field_key,
        TEMPLATE_REPO_FIELD_KEY
    );
    assert_eq!(
        describe.required_config_fields[0].value_kind,
        ConfigValueKind::Path
    );
    assert!(describe.required_config_fields[0].required);
    assert_eq!(
        describe.required_config_fields[1].field_key,
        ADAPTER_REPO_FIELD_KEY
    );
    assert_eq!(
        describe.required_config_fields[1].value_kind,
        ConfigValueKind::Path
    );
    assert!(describe.required_config_fields[1].required);
}

#[test]
fn preflight_response_blocks_when_required_repo_paths_are_missing() {
    let response = preflight_response(&PreflightRequest {
        boundline_version: QUALIFIED_BOUNDLINE_VERSION.to_string(),
        workspace_ref: "../tmp/example-workspace".to_string(),
        non_interactive: true,
        config_values: vec![path_value(TEMPLATE_REPO_FIELD_KEY, "   ")],
    });

    assert_eq!(response.status, "blocked");
    assert_eq!(response.reason.as_deref(), Some("missing_required_config"));
    assert_eq!(
        response.missing_fields,
        vec![
            TEMPLATE_REPO_FIELD_KEY.to_string(),
            ADAPTER_REPO_FIELD_KEY.to_string(),
        ]
    );
    assert_eq!(
        response.recovery.as_deref(),
        Some("boundline adapter add speckit --workspace <workspace>")
    );
}

#[test]
fn preflight_response_normalizes_repo_paths_before_returning_ready_values() {
    let response = preflight_response(&PreflightRequest {
        boundline_version: QUALIFIED_BOUNDLINE_VERSION.to_string(),
        workspace_ref: "../tmp/example-workspace".to_string(),
        non_interactive: true,
        config_values: vec![
            path_value(
                TEMPLATE_REPO_FIELD_KEY,
                "  ../boundline-framework-template  ",
            ),
            path_value(ADAPTER_REPO_FIELD_KEY, "  ../boundline-adapter-speckit  "),
        ],
    });

    assert_eq!(response.status, "ready");
    assert_eq!(response.normalized_config_values.len(), 2);
    assert_eq!(
        response.normalized_config_values[0].path_value.as_deref(),
        Some(TEMPLATE_REPO_PATH)
    );
    assert_eq!(
        response.normalized_config_values[1].path_value.as_deref(),
        Some(ADAPTER_REPO_PATH)
    );
}

#[test]
fn preflight_accepts_the_qualified_prerelease_line() {
    for version in [QUALIFIED_BOUNDLINE_VERSION, FUTURE_PRERELEASE_VERSION] {
        let response = preflight_response(&complete_request(version));
        assert_eq!(
            response.status, "ready",
            "expected {version} to be accepted"
        );
        assert_eq!(response.reason, None);
    }
}

#[test]
fn preflight_rejects_versions_outside_the_qualified_prerelease_line() {
    for version in [PRE_QUALIFICATION_VERSION, STABLE_VERSION, MALFORMED_VERSION] {
        let response = preflight_response(&complete_request(version));
        assert_eq!(
            response.status, "blocked",
            "expected {version} to be rejected"
        );
        assert_eq!(response.reason.as_deref(), Some(UNSUPPORTED_VERSION_REASON));
        assert_eq!(response.recovery.as_deref(), Some(VERSION_RECOVERY));
        assert!(response.normalized_config_values.is_empty());
    }
}

fn complete_request(boundline_version: &str) -> PreflightRequest {
    PreflightRequest {
        boundline_version: boundline_version.to_string(),
        workspace_ref: "../tmp/example-workspace".to_string(),
        non_interactive: true,
        config_values: vec![
            path_value(TEMPLATE_REPO_FIELD_KEY, TEMPLATE_REPO_PATH),
            path_value(ADAPTER_REPO_FIELD_KEY, ADAPTER_REPO_PATH),
        ],
    }
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
