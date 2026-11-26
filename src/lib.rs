//! Stage 2 — Signal & Intent Routing.
//!
//! * `reveal`  — account-level (never person-level) IP-to-company matching on CIDR ranges,
//!   with ISP / residential / VPN ranges excluded and a consent gate.
//! * `intent`  — time-decayed intent scoring per account, surge detection against baseline.
//! * `route`   — first-match routing rules, territory round-robin, event dedupe and a
//!   Slack-style alert payload.

mod api;
pub mod intent;
pub mod reveal;
pub mod route;
pub mod util;

pub use api::dispatch;
use serde_json::Value;

pub const OPS: &[&str] = &["reveal_accounts", "intent_scores", "route_signals"];

pub fn handle(op: &str, i: &Value) -> Result<Value, String> {
    match op {
        "reveal_accounts" => reveal::reveal_op(i),
        "intent_scores" => intent::score_op(i),
        "route_signals" => route::route_op(i),
        _ => Err(format!("unknown op `{op}`; known: {}", OPS.join(", "))),
    }
}
