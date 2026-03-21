use aether_contracts::StreamFrame;
use bytes::Bytes;

use crate::ExecutorClientError;

pub fn encode_frame(frame: &StreamFrame) -> Result<Bytes, ExecutorClientError> {
    let mut raw = serde_json::to_vec(frame)?;
    raw.push(b'\n');
    Ok(Bytes::from(raw))
}

pub fn decode_frame(line: &[u8]) -> Result<StreamFrame, ExecutorClientError> {
    Ok(serde_json::from_slice(line)?)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use aether_contracts::{StreamFrame, StreamFramePayload, StreamFrameType};

    use super::{decode_frame, encode_frame};

    #[test]
    fn ndjson_round_trip_preserves_frame() {
        let frame = StreamFrame {
            frame_type: StreamFrameType::Headers,
            payload: StreamFramePayload::Headers {
                status_code: 200,
                headers: BTreeMap::from([("content-type".into(), "text/event-stream".into())]),
            },
        };

        let raw = encode_frame(&frame).expect("frame should encode");
        let decoded = decode_frame(raw.trim_ascii_end()).expect("frame should decode");
        assert_eq!(decoded, frame);
    }
}
