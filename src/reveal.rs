//! Account-level website reveal. Maps a visit's IPv4 address to a company through CIDR
//! ranges. Visits from ISP, residential, mobile or VPN ranges are never matched, visits
//! without analytics consent are dropped, and no person-level data is produced.
use crate::util::*;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn parse_ip(s: &str) -> Option<u32> {
    let p: Vec<&str> = s.trim().split('.').collect();
    if p.len() != 4 {
        return None;
    }
    let mut v = 0u32;
    for x in p {
        if x.is_empty() || x.len() > 3 || !x.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let n: u32 = x.parse().ok()?;
        if n > 255 {
            return None;
        }
        v = (v << 8) | n;
    }
    Some(v)
}

/// Returns (network, prefix length).
pub fn parse_cidr(s: &str) -> Option<(u32, u32)> {
    let (ip, len) = s.split_once('/')?;
    let len: u32 = len.parse().ok()?;
    if len > 32 {
        return None;
    }
    let mask = if len == 0 { 0 } else { u32::MAX << (32 - len) };
    Some((parse_ip(ip)? & mask, len))
}

fn contains(net: (u32, u32), ip: u32) -> bool {
    let mask = if net.1 == 0 { 0 } else { u32::MAX << (32 - net.1) };
    ip & mask == net.0
}

pub fn reveal_op(i: &Value) -> R<Value> {
    let ranges = arr(i, "ranges")?;
    let visits = arr(i, "visits")?;
    let mut table = vec![];
    for r in ranges {
        let c = s(r, "cidr")?;
        let net = parse_cidr(c).ok_or(format!("bad CIDR `{c}`"))?;
        table.push((net, s_or(r, "company", "").to_string(), s_or(r, "type", "business").to_string(), r.get("domain").cloned().unwrap_or(Value::Null)));
    }
    // most specific prefix wins
    table.sort_by(|a, b| b.0 .1.cmp(&a.0 .1));
    let mut accounts: BTreeMap<String, (Value, usize, Vec<String>, std::collections::BTreeSet<String>)> = BTreeMap::new();
    let (mut no_consent, mut excluded, mut unknown) = (0, 0, 0);
    for v in visits {
        if !v.get("consent").and_then(Value::as_bool).unwrap_or(false) {
            no_consent += 1;
            continue;
        }
        let ip = parse_ip(s(v, "ip")?).ok_or("bad visit IP")?;
        match table.iter().find(|t| contains(t.0, ip)) {
            None => unknown += 1,
            Some(t) if t.2 != "business" => excluded += 1,
            Some(t) => {
                let e = accounts.entry(t.1.clone()).or_insert((t.3.clone(), 0, vec![], Default::default()));
                e.1 += 1;
                let page = s_or(v, "page", "/").to_string();
                if !e.2.contains(&page) {
                    e.2.push(page);
                }
                e.3.insert(s_or(v, "session", "?").to_string());
            }
        }
    }
    let mut out: Vec<Value> = accounts
        .into_iter()
        .map(|(c, (d, n, pages, sess))| {
            let high = pages.iter().any(|p| p.contains("pricing") || p.contains("demo") || p.contains("security"));
            json!({"company": c, "domain": d, "pageviews": n, "sessions": sess.len(), "pages": pages, "high_intent_pages": high})
        })
        .collect();
    out.sort_by(|a, b| b["pageviews"].as_u64().cmp(&a["pageviews"].as_u64()));
    Ok(json!({
        "matched_accounts": out,
        "dropped": {"no_consent": no_consent, "isp_or_residential": excluded, "unknown_range": unknown},
        "privacy": "account-level only; no names, emails or person identifiers are produced",
    }))
}

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn cidr_and_specificity() {
        assert_eq!(parse_cidr("10.1.2.3/8").unwrap(), (10 << 24, 8));
        assert!(parse_ip("1.2.3").is_none() && parse_ip("1.2.3.256").is_none() && parse_ip("01a.1.1.1").is_none());
        let i = json!({"ranges":[{"cidr":"10.0.0.0/8","company":"BigCo"},{"cidr":"10.5.0.0/16","company":"Sub"},{"cidr":"9.0.0.0/8","company":"ISP","type":"isp"}],
            "visits":[{"ip":"10.5.1.1","consent":true,"page":"/pricing"},{"ip":"10.9.1.1","consent":true},{"ip":"9.1.1.1","consent":true},{"ip":"10.5.1.2"}]});
        let r = reveal_op(&i).unwrap();
        assert_eq!(r["matched_accounts"].as_array().unwrap().len(), 2);
        assert_eq!(r["dropped"], json!({"no_consent":1,"isp_or_residential":1,"unknown_range":0}));
        assert!(r["matched_accounts"].as_array().unwrap().iter().any(|a| a["company"] == "Sub" && a["high_intent_pages"] == true));
    }
}
