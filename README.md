# lineage

A command-line client for the Lineage network. It wraps `lineage-sdk` in a
set of subcommands you can script: read chain state, manage a wallet, and
send payments or raw items to a node, with structured JSON output and
stable exit codes so it plays nicely in shell pipelines and CI.

## Install

From source:

```sh
git clone git@github.com:lineage-foundation/cli.git
cd cli
cargo install --path .
```

Or straight from the repo without cloning:

```sh
cargo install --git https://github.com/lineage-foundation/cli lineage-cli
```

Either way you end up with a `lineage` binary on your `PATH`.

## Quick examples

Check the current supply on testnet (the default network):

```sh
lineage supply
```

Create a local wallet keystore and mint a new address:

```sh
lineage wallet new
lineage wallet address
```

Preview a payment without sending it:

```sh
lineage pay LX1exampleaddress... 5 --dry-run
```

Add `--yes` (or set `confirm = "auto"` on the profile) once you're happy
with a write, and drop `--dry-run` to actually submit it.

## Profiles and config

Settings live in `~/.config/lineage/config.toml` (or the platform
equivalent — `~/Library/Application Support/lineage/config.toml` on
macOS). If the file is missing, the CLI falls back to a built-in `testnet`
profile pointed at the public `*.lineage.to` nodes with node-side signing.

A profile with a local keystore and some guardrails looks like this:

```toml
default_profile = "dev"

[profiles.dev]
mempool = "http://localhost:8081"
storage = "http://localhost:8082"
miner   = "http://localhost:8083"
signer  = "local"
confirm = "manual"
wallet_path = "/home/you/.lineage/wallet.json"
max_amount  = 100.0
daily_cap   = 500.0
allowlist   = ["LX1knownaddress..."]

[profiles.testnet]
mempool = "https://mempool.lineage.to"
storage = "https://storage.lineage.to"
miner   = "https://miner.lineage.to"
signer  = "node"
confirm = "manual"
```

Pick a profile with `--profile dev`, or point at an arbitrary host with
`--network <url>` (or `--network testnet` to force the built-in defaults).
`signer = "local"` signs with the keystore at `wallet_path`; `signer =
"node"` delegates signing to the target node's own wallet. Either way, the
passphrase never comes from an interactive prompt — set
`LINEAGE_PASSPHRASE` in the environment, or store one in the OS keyring
under the service `lineage` with the profile name as the account.

Writes (`pay`, `items`, `tx submit`, `donate`) require confirmation: pass
`--yes` or set `confirm = "auto"` on the profile. `pay` additionally
enforces `allowlist`, `max_amount`, and `daily_cap` if they're set.

## Output and exit codes

Add `--json` to get a stable envelope instead of human-readable text:

```json
{ "ok": true, "data": { "total": 123 }, "error": null }
```

or, on failure:

```json
{ "ok": false, "data": null, "error": { "code": 4, "message": "...", "detail": null } }
```

Exit codes follow the same contract regardless of `--json`:

| code | meaning |
|------|---------|
| 0 | success |
| 1 | runtime error (bad response, decode failure, etc.) |
| 2 | usage error (bad arguments, missing config) |
| 3 | denied by a guardrail (allowlist, cap, or missing confirmation) |
| 4 | network or node unreachable |

Amounts are given and displayed in whole LNGX (decimals accepted); the
JSON payload always carries the underlying raw integer alongside it.
