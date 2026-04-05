use std::io;

/// Error types for claude-code-rs.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ClaudeError {
    /// `claude` command not found in PATH.
    #[error("claude CLI not found in PATH")]
    CliNotFound,

    /// CLI exited with a non-zero status code.
    #[error("claude exited with code {code}: {stderr}")]
    NonZeroExit {
        /// Exit code.
        code: i32,
        /// Captured stderr content.
        stderr: String,
    },

    /// Failed to deserialize JSON / stream-json response.
    #[error("failed to parse response")]
    ParseError(#[from] serde_json::Error),

    /// Request timed out.
    #[error("request timed out")]
    Timeout,

    /// I/O error from process spawn, stdout/stderr reads, etc.
    #[error(transparent)]
    Io(#[from] io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_not_found_message() {
        let err = ClaudeError::CliNotFound;
        assert_eq!(err.to_string(), "claude CLI not found in PATH");
    }

    #[test]
    fn non_zero_exit_message() {
        let err = ClaudeError::NonZeroExit {
            code: 1,
            stderr: "something went wrong".into(),
        };
        assert_eq!(
            err.to_string(),
            "claude exited with code 1: something went wrong"
        );
    }

    #[test]
    fn timeout_message() {
        let err = ClaudeError::Timeout;
        assert_eq!(err.to_string(), "request timed out");
    }

    #[test]
    fn from_io_error() {
        let io_err = io::Error::new(io::ErrorKind::Other, "disk full");
        let err = ClaudeError::from(io_err);
        assert!(matches!(err, ClaudeError::Io(_)));
        assert_eq!(err.to_string(), "disk full");
    }

    #[test]
    fn from_serde_error() {
        let serde_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let err = ClaudeError::from(serde_err);
        assert!(matches!(err, ClaudeError::ParseError(_)));
    }
}
