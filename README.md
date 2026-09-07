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
macOS). On first run, if the file is missing, the CLI writes a default one
with two profiles: a node-backed `testnet` and a local-signing `local`.

```toml
default_profile = "testnet"

[profiles.testnet]
mempool = "https://mempool.lineage.to"
storage = "https://storage.lineage.to"
miner   = "https://miner.lineage.to"
signer  = "node"
confirm = "manual"

[profiles.local]
mempool = "https://mempool.lineage.to"
storage = "https://storage.lineage.to"
miner   = "https://miner.lineage.to"
signer  = "local"
confirm = "manual"
wallet_path = "~/.lineage/wallet.json"
```

`signer = "node"` delegates signing to the target node's own wallet;
`signer = "local"` signs with the keystore at `wallet_path`. Because the
default profile is `testnet` — node-backed, with no `wallet_path` — the
wallet commands report `profile has no wallet_path configured` unless you
select a local profile:

```
lineage --profile local wallet new
lineage --profile local wallet address
```

To make local the default (so `--profile` isn't needed each time), set
`default_profile = "local"`. Reads (`supply`, `balance`, `blocks`) behave
the same under either profile — both point at the same hosts; only the
signer differs.

A `wallet_path` may start with `~`, which expands to your home directory,
and `wallet new` creates the keystore's parent directory if it doesn't
exist. Profiles may also set guardrails — `max_amount`, `daily_cap`, and
`allowlist`. Pick any profile with `--profile <name>`, or point at an
arbitrary host with `--network <url>` (or `--network testnet`).

The passphrase never comes from an interactive prompt — set
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

## Terminal UI

`lineage tui` opens a full-screen view of whichever profile you'd
otherwise pass on the command line — good for keeping an eye on the chain
and sending payments without stringing together individual commands.

```sh
lineage tui
lineage tui --profile dev
```

It has three views:

- **Dashboard** — the current block head, token supply and the issued
  percentage, your wallet's total balance, and when it was last refreshed.
- **Wallet** — your local wallet's balance, refreshed alongside the
  dashboard.
- **Send** — a guided payment: enter a recipient and amount, review it
  against the profile's guardrails (allowlist, `max_amount`, `daily_cap`),
  confirm with a passphrase (masked as you type), then submit. A
  successful send shows the transaction hash; anything a guardrail
  rejects is shown inline rather than failing silently.

Data refreshes on a timer, or on demand:

| key | action |
|-----|--------|
| `Tab` | cycle Dashboard → Wallet → Send |
| `1` / `2` / `3` | jump to a view directly |
| `r` | refresh dashboard and wallet data |
| `Enter` | advance the Send form (fill in → review → confirm → submit) |
| `Esc` | step back, or cancel out of the Send form |
| `q` | quit |

The same profile, signer, and guardrails as the rest of the CLI apply:
`signer = "node"` submits through the target node's own wallet, `signer =
"local"` signs with the keystore at `wallet_path`, and the passphrase
comes from `LINEAGE_PASSPHRASE` or the OS keyring before the Confirm step
falls back to asking for one.
