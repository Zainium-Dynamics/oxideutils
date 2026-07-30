//! CLI small helpers.

use std::process::ExitCode;

use crate::error::OxideError;

/// Convert `Result` into process exit code with GNU-ish status.
pub fn exit_from_result<T>(r: Result<T, OxideError>) -> ExitCode {
    match r {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            crate::error::eprint_error(&e);
            ExitCode::from(e.exit_code() as u8)
        }
    }
}

/// Exit code accumulator for multi-file tools (continue on error).
#[derive(Default)]
pub struct Status {
    code: i32,
}

impl Status {
    pub fn ok() -> Self {
        Self { code: 0 }
    }

    pub fn record(&mut self, r: Result<(), OxideError>) {
        if let Err(e) = r {
            crate::error::eprint_error(&e);
            self.code = self.code.max(e.exit_code());
        }
    }

    pub fn exit_code(self) -> ExitCode {
        ExitCode::from(self.code as u8)
    }

    pub fn code(&self) -> i32 {
        self.code
    }
}
