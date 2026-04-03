use aether_data::repository::video_tasks::{StoredVideoTask, VideoTaskStatus};
use serde_json::{json, Value};

use crate::gateway::video_tasks::LocalVideoTaskReadResponse;

pub(super) fn map_openai_video_task_to_read_response(
    task: StoredVideoTask,
) -> LocalVideoTaskReadResponse {
    match task.status {
        VideoTaskStatus::Cancelled => LocalVideoTaskReadResponse {
            status_code: 404,
            body_json: json!({"detail": "Video task was cancelled"}),
        },
        VideoTaskStatus::Deleted => LocalVideoTaskReadResponse {
            status_code: 404,
            body_json: json!({"detail": "Video task not found"}),
        },
        status => LocalVideoTaskReadResponse {
            status_code: 200,
            body_json: build_openai_video_task_body(task, status),
        },
    }
}

fn build_openai_video_task_body(task: StoredVideoTask, status: VideoTaskStatus) -> Value {
    let mut body = json!({
        "id": task.id,
        "object": "video",
        "status": map_openai_video_status(status),
        "progress": task.progress_percent,
        "created_at": task.created_at_unix_secs,
    });

    if let Some(model) = task.model {
        body["model"] = Value::String(model);
    }
    if let Some(prompt) = task.prompt {
        body["prompt"] = Value::String(prompt);
    }
    if let Some(size) = task.size {
        body["size"] = Value::String(size);
    }
    if let Some(video_url) = task.video_url {
        body["video_url"] = Value::String(video_url);
    }
    if matches!(
        status,
        VideoTaskStatus::Failed | VideoTaskStatus::Expired | VideoTaskStatus::Cancelled
    ) {
        body["error"] = json!({
            "code": task.error_code.unwrap_or_else(|| "unknown".to_string()),
            "message": task
                .error_message
                .unwrap_or_else(|| "Video generation failed".to_string()),
        });
    }

    body
}

fn map_openai_video_status(status: VideoTaskStatus) -> &'static str {
    match status {
        VideoTaskStatus::Pending | VideoTaskStatus::Submitted | VideoTaskStatus::Queued => "queued",
        VideoTaskStatus::Processing => "processing",
        VideoTaskStatus::Completed => "completed",
        VideoTaskStatus::Failed | VideoTaskStatus::Cancelled | VideoTaskStatus::Expired => "failed",
        VideoTaskStatus::Deleted => "deleted",
    }
}
