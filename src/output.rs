//! Human/JSON output for the `lineage` CLI.
//!
//! The JSON envelope is stable: `{ "ok": bool, "data": <payload|null>,
//! "error": <problem|null> }`. `Reporter::ok` prints either the envelope
//! (`--json`) or a caller-supplied human line; `Reporter::fail` prints
//! either the envelope or the message to stderr.

use serde::Serialize;
use serde_json::{json, Value};

use crate::exit::Code;

pub struct Reporter {
    pub json: bool,
    pub quiet: bool,
}

impl Reporter {
    pub fn new(json: bool, quiet: bool) -> Self {
        Self { json, quiet }
    }

    /// Build the success envelope without printing it (kept separate so it
    /// can be unit tested without capturing stdout).
    pub fn ok_envelope<T: Serialize>(&self, data: &T) -> Value {
        json!({ "ok": true, "data": data, "error": Value::Null })
    }

    /// Build the failure envelope without printing it.
    pub fn fail_envelope(&self, code: Code, message: &str, detail: Option<Value>) -> Value {
        json!({
            "ok": false,
            "data": Value::Null,
            "error": {
                "code": code.as_u8(),
                "message": message,
                "detail": detail,
            }
        })
    }

    /// Report success: JSON mode prints the envelope; text mode prints the
    /// human line (suppressed by `--quiet`).
    pub fn ok<T: Serialize>(&self, data: &T, human: &str) {
        if self.json {
            println!("{}", self.ok_envelope(data));
        } else if !self.quiet {
            println!("{}", human);
        }
    }

    /// Report failure: JSON mode prints the envelope to stdout; text mode
    /// prints the message to stderr. `--quiet` never suppresses failures.
    pub fn fail(&self, code: Code, message: &str, detail: Option<Value>) {
        if self.json {
            println!("{}", self.fail_envelope(code, message, detail));
        } else {
            eprintln!("{}", message);
        }
    }
}

/// Map an SDK error to an exit code and a JSON-serializable problem payload.
pub fn render_error(err: &lineage_sdk::Error) -> (Code, Value) {
    match err {
        lineage_sdk::Error::Http(_) => (Code::Network, json!({ "message": err.to_string() })),
        lineage_sdk::Error::Api(problem) => (
            Code::Runtime,
            json!({
                "status": problem.status,
                "title": problem.title,
                "detail": problem.detail,
                "request_id": problem.request_id,
            }),
        ),
        lineage_sdk::Error::Decode(_) => (Code::Runtime, json!({ "message": err.to_string() })),
        lineage_sdk::Error::Keystore(msg) => (Code::Runtime, json!({ "message": msg })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lineage_sdk::ApiProblem;

    #[test]
    fn ok_envelope_has_the_stable_shape() {
        let reporter = Reporter::new(true, false);
        let envelope = reporter.ok_envelope(&json!({ "total": 42 }));
        assert_eq!(
            envelope,
            json!({ "ok": true, "data": { "total": 42 }, "error": null })
        );
    }

    #[test]
    fn ok_envelope_carries_null_data_for_unit_payloads() {
        let reporter = Reporter::new(true, false);
        let envelope = reporter.ok_envelope(&());
        assert_eq!(envelope, json!({ "ok": true, "data": null, "error": null }));
    }

    #[test]
    fn fail_envelope_has_the_stable_shape() {
        let reporter = Reporter::new(true, false);
        let envelope = reporter.fail_envelope(Code::Denied, "over the daily cap", None);
        assert_eq!(
            envelope,
            json!({
                "ok": false,
                "data": null,
                "error": { "code": 3, "message": "over the daily cap", "detail": null }
            })
        );
    }

    #[test]
    fn fail_envelope_passes_detail_through() {
        let reporter = Reporter::new(true, false);
        let detail = json!({ "status": 404 });
        let envelope = reporter.fail_envelope(Code::Runtime, "not found", Some(detail.clone()));
        assert_eq!(envelope["error"]["detail"], detail);
    }

    #[test]
    fn maps_api_error_to_runtime_with_problem_passed_through() {
        let err = lineage_sdk::Error::Api(ApiProblem {
            status: 404,
            title: Some("Not Found".into()),
            detail: Some("no block at that height".into()),
            request_id: Some("req-1".into()),
        });
        let (code, problem) = render_error(&err);
        assert_eq!(code, Code::Runtime);
        assert_eq!(problem["status"], 404);
        assert_eq!(problem["detail"], "no block at that height");
        assert_eq!(problem["request_id"], "req-1");
    }

    #[test]
    fn maps_decode_error_to_runtime() {
        let decode_err = serde_json::from_str::<i32>("not json").unwrap_err();
        let err = lineage_sdk::Error::Decode(decode_err);
        let (code, _problem) = render_error(&err);
        assert_eq!(code, Code::Runtime);
    }

    #[test]
    fn maps_keystore_error_to_runtime() {
        let err = lineage_sdk::Error::Keystore("locked".into());
        let (code, problem) = render_error(&err);
        assert_eq!(code, Code::Runtime);
        assert_eq!(problem["message"], "locked");
    }

    // The `Http` variant wraps `reqwest::Error`, which has no public
    // constructor and no dev-dependency on `reqwest` in this crate today.
    // Rather than add `reqwest` as a dev-dependency just to fabricate one,
    // we cover the Http -> Network(4) mapping with a live integration test
    // once the read commands land (Task 4, e.g. a request against a closed
    // port), where a real `reqwest::Error` is produced naturally by the SDK.
}
