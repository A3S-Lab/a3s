use axum::response::sse::{Event, KeepAlive, Sse};
use futures::Stream;
use std::convert::Infallible;

/// Create an SSE response from a stream of JSON-serializable values.
///
/// Each item in the stream is serialized to JSON and sent as an SSE `data` event.
/// A final `[DONE]` event is sent when the stream ends.
pub fn sse_response<S>(stream: S) -> Sse<impl Stream<Item = Result<Event, Infallible>>>
where
    S: Stream<Item = String> + Send + 'static,
{
    use futures::StreamExt;

    let event_stream = stream.map(|data| Ok(Event::default().data(data)));

    Sse::new(event_stream).keep_alive(KeepAlive::default())
}

/// Format a single SSE data line from a serializable value.
pub fn format_sse_data<T: serde::Serialize>(value: &T) -> Option<String> {
    serde_json::to_string(value).ok()
}

/// The standard SSE termination marker used by OpenAI-compatible APIs.
pub const SSE_DONE: &str = "[DONE]";
