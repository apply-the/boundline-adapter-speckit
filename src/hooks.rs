//! Placeholder hook-delivery behavior for the Speckit scaffold.

use std::path::Path;

use crate::profile::{EmitHookRequest, EmitHookResponse};

const FIXTURE_DIRECTORY_NAME: &str = ".speckit";
const EMIT_HOOK_FAILED_FIXTURE_FILE_NAME: &str = "emit-hook.failed";
const STATUS_DELIVERED: &str = "delivered";
const STATUS_FAILED: &str = "failed";

/// Builds a stable hook response, with an opt-in workspace fixture that
/// exercises hook failure outcomes without changing the protocol envelope.
pub(crate) fn emit_hook_response(request: &EmitHookRequest) -> EmitHookResponse {
    if emit_hook_failure_fixture_exists(&request.workspace_ref) {
        return EmitHookResponse {
            status: STATUS_FAILED.to_string(),
            summary: format!(
                "Speckit failed hook {} for stage {}",
                request.hook_key, request.stage_key
            ),
        };
    }

    EmitHookResponse {
        status: STATUS_DELIVERED.to_string(),
        summary: format!(
            "Speckit observed hook {} for stage {}",
            request.hook_key, request.stage_key
        ),
    }
}

fn emit_hook_failure_fixture_exists(workspace_ref: &str) -> bool {
    Path::new(workspace_ref)
        .join(FIXTURE_DIRECTORY_NAME)
        .join(EMIT_HOOK_FAILED_FIXTURE_FILE_NAME)
        .is_file()
}
