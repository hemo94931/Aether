use aether_data::repository::video_tasks::{StoredVideoTask, VideoTaskStatus};
use serde_json::json;

use crate::gateway::video_tasks::LocalVideoTaskReadResponse;

pub(super) fn map_gemini_video_task_to_read_response(
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
        VideoTaskStatus::Completed => LocalVideoTaskReadResponse {
            status_code: 200,
            body_json: build_gemini_completed_body(task),
        },
        VideoTaskStatus::Failed | VideoTaskStatus::Expired => LocalVideoTaskReadResponse {
            status_code: 200,
            body_json: build_gemini_failed_body(task),
        },
        _ => LocalVideoTaskReadResponse {
            status_code: 200,
            body_json: build_gemini_pending_body(task),
        },
    }
}

fn build_gemini_completed_body(task: StoredVideoTask) -> serde_json::Value {
    let operation_name = operation_name(&task);
    let short_id = task.short_id.unwrap_or_default();

    json!({
        "name": operation_name,
        "done": true,
        "response": {
            "generateVideoResponse": {
                "generatedSamples": [
                    {
                        "video": {
                            "uri": format!("/v1beta/files/aev_{short_id}:download?alt=media"),
                            "mimeType": "video/mp4"
                        }
                    }
                ]
            }
        }
    })
}

fn build_gemini_failed_body(task: StoredVideoTask) -> serde_json::Value {
    json!({
        "name": operation_name(&task),
        "done": true,
        "error": {
            "code": task.error_code.unwrap_or_else(|| "UNKNOWN".to_string()),
            "message": task
                .error_message
                .unwrap_or_else(|| "Video generation failed".to_string()),
        }
    })
}

fn build_gemini_pending_body(task: StoredVideoTask) -> serde_json::Value {
    json!({
        "name": operation_name(&task),
        "done": false,
        "metadata": {}
    })
}

fn operation_name(task: &StoredVideoTask) -> String {
    let model = task.model.clone().unwrap_or_else(|| "unknown".to_string());
    let short_id = task.short_id.clone().unwrap_or_else(|| task.id.clone());
    format!("models/{model}/operations/{short_id}")
}
