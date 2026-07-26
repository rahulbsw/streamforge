use std::fmt;

/// Stable, bounded error classes suitable for policy and metric decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WasmErrorKind {
    Configuration,
    Io,
    Integrity,
    Compilation,
    Abi,
    UnknownModule,
    WorldMismatch,
    InputLimit,
    OutputLimit,
    Timeout,
    ResourceLimit,
    Trap,
    Guest,
    InvalidOutput,
}

/// A classified UDF failure. Payload bytes and guest backtraces are never
/// retained, which keeps errors safe to log and hand to policy code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmError {
    kind: WasmErrorKind,
    module: Option<String>,
    message: String,
}

impl WasmError {
    pub fn new(
        kind: WasmErrorKind,
        module: Option<impl Into<String>>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            module: module.map(Into::into),
            message: sanitize_message(message.into()),
        }
    }

    pub fn kind(&self) -> WasmErrorKind {
        self.kind
    }

    pub fn module(&self) -> Option<&str> {
        self.module.as_deref()
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for WasmError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.module {
            Some(module) => write!(
                formatter,
                "WebAssembly UDF {module:?} failed ({:?}): {}",
                self.kind, self.message
            ),
            None => write!(
                formatter,
                "WebAssembly UDF runtime failed ({:?}): {}",
                self.kind, self.message
            ),
        }
    }
}

impl std::error::Error for WasmError {}

fn sanitize_message(message: String) -> String {
    const MAX_ERROR_CHARS: usize = 512;
    let mut sanitized = String::with_capacity(message.len().min(MAX_ERROR_CHARS));
    for character in message.chars().take(MAX_ERROR_CHARS) {
        if character.is_control() && !matches!(character, '\t' | '\n') {
            sanitized.push('\u{fffd}');
        } else {
            sanitized.push(character);
        }
    }
    if message.chars().count() > MAX_ERROR_CHARS {
        sanitized.push('…');
    }
    sanitized
}

pub(crate) fn error(
    kind: WasmErrorKind,
    module: Option<&str>,
    message: impl Into<String>,
) -> WasmError {
    WasmError::new(kind, module.map(str::to_owned), message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_messages_are_bounded_and_sanitized() {
        let message = format!("{}\0secret", "x".repeat(600));
        let error = WasmError::new(WasmErrorKind::Guest, Some("guest"), message);
        assert!(error.message().chars().count() <= 513);
        assert!(!error.message().contains('\0'));
    }
}
