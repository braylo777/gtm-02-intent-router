# gtm-02-intent-router

**GTM Engineering — Stage 2 of 12: Signal & Intent Routing**

Turn buying signals into the right rep's Slack in minutes: account-level website reveal with privacy gates, time-decayed intent scores with surge detection, and first-match routing with round-robin and dedupe.

**Live demo:** https://gtm-02-intent-router.braylo777.workers.dev — the Rust core runs in your browser as WebAssembly. No server, no tracking.

## Why this matters

Buyers research long before they fill a form. The team that sees the signal first and routes it to one accountable owner with context wins the meeting. Most stacks fail in the plumbing: duplicate webhooks, alerts to the wrong rep, residential IPs matched to companies, stale signals treated as hot. This repo is that plumbing, with the privacy rules built in.

## What it does

- IPv4 CIDR matching, most-specific prefix wins; ISP / residential / VPN ranges are never matched to a company
- Consent gate: visits without analytics consent are dropped before matching; output is account-level only, no person data
- Intent scoring: per-signal weights, exponential decay with a half-life, 7-day surge vs 4-week baseline, route / nurture / ignore trigger
- Routing: rule conditions (eq, neq, in, gte, lte, contains), first match wins, account-owner override, round-robin pools, event-id dedupe, fallback queue
- Slack-style alert payload with account, signal, page, score, owner and SLA

## What it does not do (yet)

- No Clearbit / 6sense / Bombora connection: IP ranges and signals are synthetic JSON
- IPv4 only; no IPv6 ranges
- Builds the Slack payload but does not post it, and does not verify webhook signatures

## Operations

Every operation takes JSON in and gives JSON out. The CLI, the tests and the demo page all use the same entry point.

| Operation | What it does |
|---|---|
| `reveal_accounts` | Match consented visits to companies by IP range. ISP and residential ranges are excluded. All IPs are from documentation ranges. |
| `intent_scores` | Score accounts from weighted signals that decay over time, and flag accounts surging against their own baseline. |
| `route_signals` | Dedupe events, apply the first matching rule, assign an owner (account owner first, then round-robin) and build the Slack alert. |

## Run it

You need Rust (stable). No Node, no npm.

```bash
git clone https://github.com/braylo777/gtm-02-intent-router
cd gtm-02-intent-router
cargo test
cargo run -- ops
```

Try the first demo example from the command line:

```bash
python3 -c 'import json;print(json.dumps(json.load(open("web/examples.json"))["ops"][0]["input"]))' | cargo run -q -- reveal_accounts
```

## Build and deploy the demo

```bash
./build.sh    # tests, then compiles the core to web/app.wasm
./deploy.sh   # serves web/ as a Cloudflare Workers static-assets site (needs a Cloudflare login)
```

`web/` is a plain static site (`index.html`, `app.js`, `app.wasm`). It has no build step, so any static host works.

## Layout

```
src/lib.rs        operation list and router
src/api.rs        JSON entry point + the 3 WASM exports (alloc, dealloc, run)
src/main.rs       CLI
src/*.rs          one module per capability, each with unit tests
tests/            integration tests (every demo example must pass)
web/              demo page; examples.json drives both the page and this README
```

## Design choices

- **Rust, few dependencies.** Only `serde_json`, pinned to exact versions with a committed `Cargo.lock`.
- **Deterministic.** Time, randomness and keys are inputs, so every result can be replayed in a test or an audit.
- **Fail closed.** Unknown input or missing fields give an error, never a silent default.

## Part of a series

This is one of 12 repos that work through a public Go-To-Market (GTM) Engineering roadmap: [12 stages by Suraj Sharma (@suraj_sharma14)](https://x.com/suraj_sharma14/status/2125552583452270692). The roadmap is the inspiration; the code here is original. All 12: [github.com/braylo777?q=gtm-](https://github.com/braylo777?tab=repositories&q=gtm-).

## Data and privacy

Every example uses synthetic data. Nothing calls a paid API, scrapes a site, identifies a real visitor or sends an email.

## License

MIT — see [LICENSE](LICENSE). Built by [Braylo SI](https://braylo.si).
