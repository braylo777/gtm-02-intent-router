//! Routing: dedupe events by id, match the first rule whose conditions all hold, pick an
//! owner by round-robin inside the rule's pool (or the account owner if one exists), and
//! build a Slack-style alert. No rule match goes to a fallback queue, never dropped.
use crate::util::*;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

fn cond_ok(c: &Value, ev: &Value) -> R<bool> {
    let field = s(c, "field")?;
    let v = ev.get(field).unwrap_or(&Value::Null);
    let op = s(c, "op")?;
    let t = c.get("value").unwrap_or(&Value::Null);
    Ok(match op {
        "eq" => v == t,
        "neq" => v != t,
        "in" => t.as_array().ok_or("`in` needs an array")?.contains(v),
        "gte" => v.as_f64().zip(t.as_f64()).map(|(a, b)| a >= b).unwrap_or(false),
        "lte" => v.as_f64().zip(t.as_f64()).map(|(a, b)| a <= b).unwrap_or(false),
        "contains" => v.as_str().zip(t.as_str()).map(|(a, b)| a.contains(b)).unwrap_or(false),
        _ => return Err(format!("unknown condition op `{op}`")),
    })
}

pub fn route_op(i: &Value) -> R<Value> {
    let rules = arr(i, "rules")?;
    let fallback = s_or(i, "fallback_queue", "#gtm-unrouted");
    let owners = i.get("account_owners").and_then(Value::as_object).cloned().unwrap_or_default();
    let mut rr: BTreeMap<usize, usize> = BTreeMap::new();
    let mut seen = BTreeSet::new();
    let (mut routed, mut dups) = (vec![], 0);
    for ev in arr(i, "events")? {
        let id = s(ev, "event_id")?;
        if !seen.insert(id.to_string()) {
            dups += 1;
            continue;
        }
        let mut hit = None;
        for (n, r) in rules.iter().enumerate() {
            let conds = arr(r, "when")?;
            let mut ok = true;
            for c in conds {
                if !cond_ok(c, ev)? {
                    ok = false;
                    break;
                }
            }
            if ok {
                hit = Some(n);
                break;
            }
        }
        let account = s_or(ev, "account", "unknown");
        let row = match hit {
            None => json!({"event_id": id, "rule": null, "owner": null, "channel": fallback}),
            Some(n) => {
                let r = &rules[n];
                let owner = if let Some(o) = owners.get(account).and_then(Value::as_str) {
                    o.to_string()
                } else {
                    let pool = arr(r, "pool")?;
                    if pool.is_empty() {
                        return Err(format!("rule {} has an empty pool", s_or(r, "name", "?")));
                    }
                    let k = rr.entry(n).or_insert(0);
                    let o = pool[*k % pool.len()].as_str().unwrap_or("?").to_string();
                    *k += 1;
                    o
                };
                let ch = s_or(r, "channel", fallback);
                let text = format!(
                    ":rotating_light: *{}* — {} on `{}` (score {}). Owner: @{}. SLA: {} min.",
                    account, s_or(ev, "type", "signal"), s_or(ev, "page", "-"),
                    ev.get("score").map(|x| x.to_string()).unwrap_or("-".into()), owner, r.get("sla_minutes").and_then(Value::as_u64).unwrap_or(60)
                );
                json!({"event_id": id, "rule": s_or(r, "name", "?"), "owner": owner, "channel": ch,
                       "slack": {"channel": ch, "text": text}})
            }
        };
        routed.push(row);
    }
    let unrouted = routed.iter().filter(|r| r["rule"].is_null()).count();
    Ok(json!({"routed": routed, "duplicates_skipped": dups, "unrouted": unrouted}))
}

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn first_match_round_robin_dedupe() {
        let i = json!({"account_owners":{"Owned":"zoe"},"rules":[
            {"name":"hot","when":[{"field":"score","op":"gte","value":50}],"pool":["a","b"],"channel":"#hot"}],
            "events":[{"event_id":"1","account":"X","score":60},{"event_id":"2","account":"Y","score":70},
                      {"event_id":"2","account":"Y","score":70},{"event_id":"3","account":"Owned","score":90},{"event_id":"4","account":"Z","score":1}]});
        let r = route_op(&i).unwrap();
        let o: Vec<&Value> = r["routed"].as_array().unwrap().iter().map(|x| &x["owner"]).collect();
        assert_eq!(o, vec![&json!("a"), &json!("b"), &json!("zoe"), &Value::Null]);
        assert_eq!(r["duplicates_skipped"], 1);
        assert_eq!(r["unrouted"], 1);
    }
}
