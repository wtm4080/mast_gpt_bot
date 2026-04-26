#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RecoverableFailure<'a> {
    FetchStatusContext,
    GenerateReply,
    PostReply,
    SaveResponseId { thread_key: &'a str },
    HandleStreamMessage,
    WebSocket,
    ConnectStreamingApi,
}

impl RecoverableFailure<'_> {
    fn log_prefix(self) -> String {
        match self {
            Self::FetchStatusContext => "Failed to fetch status context".to_string(),
            Self::GenerateReply => "Failed to generate reply".to_string(),
            Self::PostReply => "Failed to post reply".to_string(),
            Self::SaveResponseId { thread_key } => {
                format!("Failed to update last_response_id for thread {}", thread_key)
            }
            Self::HandleStreamMessage => "Error handling stream message".to_string(),
            Self::WebSocket => "WebSocket error".to_string(),
            Self::ConnectStreamingApi => "Failed to connect streaming API".to_string(),
        }
    }
}

pub(super) fn recoverable_error_message(
    failure: RecoverableFailure<'_>,
    error: &dyn std::fmt::Debug,
) -> String {
    format!("{}: {:?}", failure.log_prefix(), error)
}

pub(super) fn log_recoverable_error(failure: RecoverableFailure<'_>, error: &dyn std::fmt::Debug) {
    eprintln!("{}", recoverable_error_message(failure, error));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_recoverable_error_messages_with_existing_prefixes() {
        assert_eq!(
            recoverable_error_message(RecoverableFailure::GenerateReply, &"boom"),
            "Failed to generate reply: \"boom\""
        );
        assert_eq!(
            recoverable_error_message(
                RecoverableFailure::SaveResponseId { thread_key: "thread-1" },
                &"db down",
            ),
            "Failed to update last_response_id for thread thread-1: \"db down\""
        );
    }
}
