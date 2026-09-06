//! Process exit codes for the `lineage` CLI.
//!
//! Stable contract: `0` ok, `1` runtime error, `2` usage error,
//! `3` guardrail/denied, `4` network/node unreachable.

use std::process::ExitCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Code {
    Ok = 0,
    Runtime = 1,
    Usage = 2,
    Denied = 3,
    Network = 4,
}

impl Code {
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

impl From<Code> for ExitCode {
    fn from(code: Code) -> Self {
        ExitCode::from(code.as_u8())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_map_to_the_stable_contract() {
        assert_eq!(Code::Ok.as_u8(), 0);
        assert_eq!(Code::Runtime.as_u8(), 1);
        assert_eq!(Code::Usage.as_u8(), 2);
        assert_eq!(Code::Denied.as_u8(), 3);
        assert_eq!(Code::Network.as_u8(), 4);
    }

    #[test]
    fn converts_into_process_exit_code() {
        // ExitCode has no public equality check, so we only assert the
        // conversion compiles and runs without panicking for every variant.
        let _: ExitCode = Code::Ok.into();
        let _: ExitCode = Code::Runtime.into();
        let _: ExitCode = Code::Usage.into();
        let _: ExitCode = Code::Denied.into();
        let _: ExitCode = Code::Network.into();
    }
}
