use aether_data::repository::video_tasks::VideoTaskLookupKey;
use aether_data::DataLayerError;

use super::gemini::map_gemini_video_task_to_read_response;
use super::openai::map_openai_video_task_to_read_response;
use super::state::GatewayDataState;
use crate::gateway::video_tasks::{
    extract_gemini_short_id_from_path, extract_openai_task_id_from_path, LocalVideoTaskReadResponse,
};

pub(super) async fn read_video_task_response(
    state: &GatewayDataState,
    route_family: Option<&str>,
    request_path: &str,
) -> Result<Option<LocalVideoTaskReadResponse>, DataLayerError> {
    match route_family {
        Some("openai") => read_openai_video_task_response(state, request_path).await,
        Some("gemini") => read_gemini_video_task_response(state, request_path).await,
        _ => Ok(None),
    }
}

async fn read_openai_video_task_response(
    state: &GatewayDataState,
    request_path: &str,
) -> Result<Option<LocalVideoTaskReadResponse>, DataLayerError> {
    let Some(task_id) = extract_openai_task_id_from_path(request_path) else {
        return Ok(None);
    };

    let Some(task) = state
        .find_video_task(VideoTaskLookupKey::Id(task_id))
        .await?
    else {
        return Ok(None);
    };

    if !matches!(task.provider_api_format.as_deref(), Some("openai:video")) {
        return Ok(None);
    }

    Ok(Some(map_openai_video_task_to_read_response(task)))
}

async fn read_gemini_video_task_response(
    state: &GatewayDataState,
    request_path: &str,
) -> Result<Option<LocalVideoTaskReadResponse>, DataLayerError> {
    let Some(short_id) = extract_gemini_short_id_from_path(request_path) else {
        return Ok(None);
    };

    let Some(task) = state
        .find_video_task(VideoTaskLookupKey::ShortId(short_id))
        .await?
    else {
        return Ok(None);
    };

    if !matches!(task.provider_api_format.as_deref(), Some("gemini:video")) {
        return Ok(None);
    }

    Ok(Some(map_gemini_video_task_to_read_response(task)))
}
