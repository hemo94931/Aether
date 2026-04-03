//! Dedicated WebSocket writer task.
//!
//! All frame writes go through an mpsc channel to a single writer task,
//! avoiding contention on the WebSocket sink.  The writer also sends
//! periodic WebSocket Ping frames to keep the connection alive through
//! intermediary proxies (Nginx, Cloudflare, etc.).

use std::time::Duration;

use aether_runtime::{bounded_queue, BoundedQueueSender};
use futures_util::SinkExt;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, trace};

use super::protocol::Frame;

/// Sender half — cloned by stream handlers and heartbeat.
pub type FrameSender = BoundedQueueSender<Frame>;

/// Spawn the writer task. Returns the sender and a JoinHandle for cleanup.
///
/// `ping_interval` controls WebSocket-level Ping frequency (typically 15s).
/// This keeps the connection alive through intermediary proxies/load-balancers.
pub fn spawn_writer<S>(mut sink: S, ping_interval: Duration) -> (FrameSender, JoinHandle<()>)
where
    S: SinkExt<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin + Send + 'static,
{
    let (tx, mut rx) = bounded_queue::<Frame>(256);

    let handle = tokio::spawn(async move {
        let mut ping_ticker = tokio::time::interval(ping_interval);
        ping_ticker.tick().await; // skip first immediate tick

        loop {
            tokio::select! {
                frame = rx.recv() => {
                    match frame {
                        Some(frame) => {
                            let data = frame.encode();
                            if let Err(e) = sink.send(Message::Binary(data.into())).await {
                                error!(error = %e, "failed to write frame to WebSocket");
                                break;
                            }
                        }
                        None => break, // all senders dropped
                    }
                }
                _ = ping_ticker.tick() => {
                    if let Err(e) = sink.send(Message::Ping(vec![])).await {
                        error!(error = %e, "failed to send WebSocket ping");
                        break;
                    }
                    trace!("sent WebSocket ping");
                }
            }
        }
        debug!("writer task exiting");
        let _ = sink.close().await;
    });

    (tx, handle)
}
