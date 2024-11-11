use async_stream::try_stream;
use futures::{Stream, StreamExt};
use reqwest::Response;
use std::pin::Pin;
use std::{
    fmt::{self, Display, Formatter},
    time::Duration,
};
use thiserror::Error;

const EVENT_DELIMITER: &str = "\n\n";
const FIELD_SEPARATOR: char = ':';

/// Possible errors that can occur while parsing SSE events
#[derive(Error, Debug)]
pub enum EventError {
    #[error("failed to parse retry value: {0}")]
    RetryParse(std::num::ParseIntError),
    #[error("invalid event format: event contains no data")]
    InvalidFormat,
}

impl From<std::num::ParseIntError> for EventError {
    fn from(err: std::num::ParseIntError) -> Self {
        Self::RetryParse(err)
    }
}

/// Represents a Server-Sent Event (SSE) with its associated fields.
///
/// Each event can contain:
/// - An optional ID
/// - An optional event type
/// - The event data (required)
/// - An optional retry timeout
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// Unique identifier for the event
    pub id: Option<String>,
    /// Type of the event (defaults to "message" in SSE spec)
    pub event_type: Option<String>,
    /// The event payload
    pub data: String,
    /// Reconnection time in case of connection failure
    pub retry: Option<Duration>,
}

impl Default for Event {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for Event {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Event {{ id: {:?}, event_type: {:?}, data: {}, retry: {:?} }}",
            self.id, self.event_type, self.data, self.retry
        )
    }
}

impl Event {
    /// Creates a new empty Event.
    pub const fn new() -> Self {
        Self {
            id: None,
            event_type: None,
            data: String::new(),
            retry: None,
        }
    }

    /// Parses an SSE event from a string slice.
    ///
    /// # Arguments
    ///
    /// * `input` - The string slice containing the event data
    ///
    /// # Returns
    ///
    /// Returns `Ok(Event)` if parsing succeeds and the event contains data,
    /// or `Err(EventError)` if parsing fails or the event is empty.
    pub fn parse(input: &str) -> Result<Self, EventError> {
        let mut event = Self::new();
        let mut data_lines = Vec::new();

        for line in input.lines() {
            if line.is_empty() {
                continue;
            }

            if let Some((field, value)) = line.split_once(FIELD_SEPARATOR) {
                let value = value.trim_start();
                match field {
                    "id" => event.id = Some(value.to_string()),
                    "event" => event.event_type = Some(value.to_string()),
                    "data" => data_lines.push(value),
                    "retry" => {
                        let ms = value.parse::<u64>()?;
                        event.retry = Some(Duration::from_millis(ms));
                    }
                    _ => {} // Ignore unknown fields as per SSE spec
                }
            }
        }

        if data_lines.is_empty() {
            return Err(EventError::InvalidFormat);
        }

        // Preallocate string with estimated capacity
        let total_len = data_lines.iter().map(|s| s.len() + 1).sum::<usize>() - 1;
        let mut data = String::with_capacity(total_len);

        // Join lines efficiently
        for (i, line) in data_lines.iter().enumerate() {
            if i > 0 {
                data.push('\n');
            }
            data.push_str(line);
        }

        event.data = data;
        Ok(event)
    }
}

/// Extension trait for converting a Response into a Stream of SSE Events.
pub trait EventSourceExt {
    /// Converts the response into a Stream of Events.
    ///
    /// # Returns
    ///
    /// Returns a pinned Stream that yields Result<Event, `reqwest::Error`>
    fn events(self) -> Pin<Box<dyn Stream<Item = Result<Event, reqwest::Error>> + Send>>;
}

impl EventSourceExt for Response {
    fn events(self) -> Pin<Box<dyn Stream<Item = Result<Event, reqwest::Error>> + Send>> {
        Box::pin(try_stream! {
            let mut stream = self.bytes_stream();
            let mut buffer = String::with_capacity(1024); // Pre-allocate with reasonable size

            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                buffer.push_str(&String::from_utf8_lossy(&chunk));

                // Process complete events
                while let Some(event_end) = buffer.find(EVENT_DELIMITER) {
                    let event_str = &buffer[..event_end];
                    if let Ok(event) = Event::parse(event_str) {
                        yield event;
                    }
                    buffer.drain(..event_end + EVENT_DELIMITER.len());
                }
            }

            // Process any remaining data in the buffer
            if !buffer.is_empty() {
                if let Ok(event) = Event::parse(&buffer) {
                    yield event;
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_parse_empty() {
        assert!(matches!(Event::parse(""), Err(EventError::InvalidFormat)));
    }

    #[test]
    fn test_event_parse_no_data() {
        assert!(matches!(
            Event::parse("id: 123\nevent: test\n"),
            Err(EventError::InvalidFormat)
        ));
    }

    #[test]
    fn test_event_parse_simple() {
        let input = "data: hello\n\n";
        let event = Event::parse(input).unwrap();
        assert_eq!(event.data, "hello");
        assert!(event.id.is_none());
        assert!(event.event_type.is_none());
    }

    #[test]
    fn test_event_parse_complex() {
        let input = "id: 123\nevent: update\ndata: line1\ndata: line2\nretry: 5000\n\n";
        let event = Event::parse(input).unwrap();
        assert_eq!(event.id, Some("123".to_string()));
        assert_eq!(event.event_type, Some("update".to_string()));
        assert_eq!(event.data, "line1\nline2");
        assert_eq!(event.retry, Some(Duration::from_millis(5000)));
    }

    #[test]
    fn test_event_parse_invalid_retry() {
        let input = "retry: invalid\ndata: test\n\n";
        assert!(matches!(
            Event::parse(input),
            Err(EventError::RetryParse(_))
        ));
    }
}
