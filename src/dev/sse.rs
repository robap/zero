//! SSE endpoint `/_zero/events` and broadcast bus for dev-mode reload.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::stream::{self, StreamExt};
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;

use crate::dev::server::AppState;

/// Shared reload-event bus. Cheap to clone (`Sender` is `Clone`).
#[derive(Clone)]
pub struct ReloadBus {
    tx: broadcast::Sender<String>,
}

impl Default for ReloadBus {
    fn default() -> Self {
        Self::new()
    }
}

impl ReloadBus {
    /// Create a fresh bus with the standard capacity.
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(16);
        Self { tx }
    }

    /// Broadcast a reload event. Returns the receiver count (0 if no clients connected).
    pub fn send(&self, path: String) -> usize {
        self.tx.send(path).unwrap_or(0)
    }

    /// Subscribe a new receiver (used by the SSE handler on connect).
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }
}

/// `GET /_zero/events` — holds the connection open and fans out reload events.
pub async fn sse_handler(
    State(state): State<Arc<AppState>>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let rx = state.bus.subscribe();
    let hello =
        stream::once(async { Ok::<_, Infallible>(Event::default().event("hello").data("ok")) });
    let reloads = BroadcastStream::new(rx).filter_map(|res| async move {
        res.ok()
            .map(|path| Ok::<_, Infallible>(Event::default().event("reload").data(path)))
    });
    // End the stream when shutdown is signaled (or the sender is dropped) so
    // ctrl-c can complete graceful shutdown while a browser is connected.
    let mut shutdown = state.shutdown.clone();
    let combined = hello.chain(reloads).take_until(async move {
        let _ = shutdown.wait_for(|v| *v).await;
    });
    Sse::new(combined).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bus_send_with_no_subscribers_does_not_error() {
        let bus = ReloadBus::new();
        let count = bus.send("foo".into());
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn bus_fanout_delivers_to_multiple_subscribers() {
        let bus = ReloadBus::new();
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();
        bus.send("foo".into());
        assert_eq!(rx1.recv().await.unwrap(), "foo");
        assert_eq!(rx2.recv().await.unwrap(), "foo");
    }
}
