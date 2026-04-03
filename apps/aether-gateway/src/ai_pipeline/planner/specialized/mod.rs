//! Non-matrix AI surfaces such as files and video.

mod files;
mod video;

pub(crate) use self::files::{
    maybe_build_stream_local_gemini_files_decision_payload,
    maybe_build_sync_local_gemini_files_decision_payload,
    maybe_execute_stream_via_local_gemini_files_decision,
    maybe_execute_sync_via_local_gemini_files_decision,
};
pub(crate) use self::video::{
    maybe_build_sync_local_video_decision_payload, maybe_execute_sync_via_local_video_decision,
};
