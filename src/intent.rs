//! Intent scoring: each signal has a weight; its value halves every `half_life_days`.
//! A surge is the last-7-day score over the account's prior weekly baseline.
use crate::util::*;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn score_op(i: &Value) -> R<Value> {
    let now = f(i, "now_day")?;
    let hl = f_or(i, "half_life_days", 14.0);
    if hl <= 0.0 {
        return Err("half_life_days must be > 0".into());
    }
    let weights = obj(i, "weights")?;
    let surge_x = f_or(i, "surge_multiplier", 2.0);
    let min_score = f_or(i, "min_score", 10.0);
    let mut acc: BTreeMap<String, (f64, f64, f64, BTreeMap<String, f64>)> = BTreeMap::new();
    for e in arr(i, "signals")? {
        let a = s(e, "account")?.to_string();
        let kind = s(e, "type")?;
        let day = f(e, "day")?;
        if day > now {
            return Err(format!("signal for {a} is in the future (day {day} > now {now})"));
        }
        let w = weights.get(kind).and_then(Value::as_f64).ok_or(format!("no weight for signal type `{kind}`"))?;
        let age = now - day;
        let v = w * 0.5f64.powf(age / hl);
        let x = acc.entry(a).or_default();
        x.0 += v;
        if age < 7.0 {
            x.1 += w;
        } else if age < 35.0 {
            x.2 += w / 4.0; // weekly average over the 4 prior weeks
        }
        *x.3.entry(kind.to_string()).or_default() += v;
    }
    let mut out: Vec<Value> = acc
        .into_iter()
        .map(|(a, (score, last7, base, by))| {
            let surge = last7 >= surge_x * base.max(1.0) && score >= min_score;
            let top = by.iter().max_by(|x, y| x.1.total_cmp(y.1)).map(|x| x.0.clone());
            json!({"account": a, "score": rd(score, 1), "last_7d": rd(last7, 1), "baseline_weekly": rd(base, 1),
                   "surging": surge, "top_signal": top, "trigger": if surge {"route_now"} else if score >= min_score {"nurture"} else {"ignore"}})
        })
        .collect();
    out.sort_by(|a, b| b["score"].as_f64().unwrap().total_cmp(&a["score"].as_f64().unwrap()));
    Ok(json!({"accounts": out}))
}

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn decay_and_surge() {
        let i = json!({"now_day":100,"half_life_days":10,"weights":{"pricing":10,"g2":20},"signals":[
            {"account":"A","type":"pricing","day":100},{"account":"A","type":"g2","day":99},
            {"account":"B","type":"g2","day":90}]});
        let r = score_op(&i).unwrap();
        assert_eq!(r["accounts"][0]["account"], "A");
        assert_eq!(r["accounts"][0]["surging"], true);
        assert_eq!(r["accounts"][1]["score"], 10.0);
        assert!(score_op(&json!({"now_day":1,"weights":{},"signals":[{"account":"A","type":"x","day":0}]})).is_err());
    }
}
