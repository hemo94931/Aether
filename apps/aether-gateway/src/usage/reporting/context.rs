use aether_data::repository::video_tasks::VideoTaskLookupKey;
use serde_json::{Map, Value};

use crate::gateway::AppState;

pub(crate) fn report_context_is_locally_actionable(report_context: Option<&Value>) -> bool {
    let Some(context) = report_context else {
        return false;
    };

    has_non_empty_str(context, "request_id")
        && (has_non_empty_str(context, "candidate_id")
            || has_u64(context, "candidate_index")
            || has_non_empty_str(context, "provider_id")
            || has_non_empty_str(context, "endpoint_id")
            || has_non_empty_str(context, "key_id"))
}

pub(crate) async fn resolve_locally_actionable_report_context(
    state: &AppState,
    report_context: Option<&Value>,
) -> Option<Value> {
    let context = report_context?.clone();
    if report_context_is_locally_actionable(Some(&context)) {
        return Some(context);
    }

    if let Some(resolved) =
        resolve_locally_actionable_report_context_from_request_candidates(state, &context).await
    {
        return Some(resolved);
    }

    let context = resolve_locally_actionable_report_context_from_video_task(state, &context)
        .await
        .unwrap_or(context);

    if let Some(resolved) =
        resolve_locally_actionable_report_context_from_request_candidates(state, &context).await
    {
        return Some(resolved);
    }

    report_context_is_locally_actionable(Some(&context)).then_some(context)
}

async fn resolve_locally_actionable_report_context_from_request_candidates(
    state: &AppState,
    context: &Value,
) -> Option<Value> {
    let request_id = context
        .get("request_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let existing_candidates = state
        .read_request_candidates_by_request_id(request_id)
        .await
        .ok()?;
    if existing_candidates.len() != 1 {
        return None;
    }

    let mut object = context.as_object()?.clone();
    let candidate = &existing_candidates[0];
    insert_missing_string_value(&mut object, "candidate_id", Some(&candidate.id));
    if !object.contains_key("candidate_index") {
        object.insert(
            "candidate_index".to_string(),
            Value::Number(candidate.candidate_index.into()),
        );
    }
    insert_missing_optional_string_value(
        &mut object,
        "provider_id",
        candidate.provider_id.as_deref(),
    );
    insert_missing_optional_string_value(
        &mut object,
        "endpoint_id",
        candidate.endpoint_id.as_deref(),
    );
    insert_missing_optional_string_value(&mut object, "key_id", candidate.key_id.as_deref());
    insert_missing_optional_string_value(&mut object, "user_id", candidate.user_id.as_deref());
    insert_missing_optional_string_value(
        &mut object,
        "api_key_id",
        candidate.api_key_id.as_deref(),
    );

    let resolved = Value::Object(object);
    report_context_is_locally_actionable(Some(&resolved)).then_some(resolved)
}

async fn resolve_locally_actionable_report_context_from_video_task(
    state: &AppState,
    context: &Value,
) -> Option<Value> {
    let local_task_id = context
        .get("local_task_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let local_short_id = context
        .get("local_short_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let task_id = context
        .get("task_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let user_id = context
        .get("user_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let task = if let Some(task_id) = local_task_id {
        state
            .data
            .find_video_task(VideoTaskLookupKey::Id(task_id))
            .await
            .ok()??
    } else if let Some(short_id) = local_short_id {
        state
            .data
            .find_video_task(VideoTaskLookupKey::ShortId(short_id))
            .await
            .ok()??
    } else if let Some(task_id) = task_id {
        if let Some(task) = state
            .data
            .find_video_task(VideoTaskLookupKey::Id(task_id))
            .await
            .ok()?
        {
            task
        } else {
            let user_id = user_id?;
            state
                .data
                .find_video_task(VideoTaskLookupKey::UserExternal {
                    user_id,
                    external_task_id: task_id,
                })
                .await
                .ok()??
        }
    } else {
        return None;
    };

    let mut object = context.as_object()?.clone();
    insert_missing_string_value(&mut object, "request_id", Some(task.request_id.as_str()));
    insert_missing_optional_string_value(&mut object, "provider_id", task.provider_id.as_deref());
    insert_missing_optional_string_value(&mut object, "endpoint_id", task.endpoint_id.as_deref());
    insert_missing_optional_string_value(&mut object, "key_id", task.key_id.as_deref());
    insert_missing_optional_string_value(&mut object, "user_id", task.user_id.as_deref());
    insert_missing_optional_string_value(&mut object, "api_key_id", task.api_key_id.as_deref());
    insert_missing_optional_string_value(
        &mut object,
        "client_api_format",
        task.client_api_format.as_deref(),
    );
    insert_missing_optional_string_value(
        &mut object,
        "provider_api_format",
        task.provider_api_format.as_deref(),
    );
    Some(Value::Object(object))
}

fn insert_missing_string_value(object: &mut Map<String, Value>, key: &str, value: Option<&str>) {
    if object.contains_key(key) {
        return;
    }
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return;
    };
    object.insert(key.to_string(), Value::String(value.to_string()));
}

fn insert_missing_optional_string_value(
    object: &mut Map<String, Value>,
    key: &str,
    value: Option<&str>,
) {
    insert_missing_string_value(object, key, value);
}

fn has_non_empty_str(value: &Value, key: &str) -> bool {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
}

fn has_u64(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_u64).is_some()
}
