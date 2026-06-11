# SWEAT the TOKEN

The SWEAT fungible token (NEP-141 / NEP-148) for the Sweat Economy move-to-earn
protocol. Tokens are minted by trusted Oracles in proportion to user steps, using
a decaying issuance formula tied to the global step count since the Token
Generation Event (TGE).

## What the contract does

- **Deferred minting.** Oracles submit batches of `(account, steps)` via
  `defer_batch`. For each batch the contract computes the SWEAT payout using
  the issuance formula in `math.rs`, splits it **95% user / 5% Oracle fee**,
  and performs an XCC to the configured **holding contract**
  (`holding_account_id`). On a successful callback (`on_record`), the user
  portion is minted to the holding contract and the fee to the calling Oracle.
  On failure, the `steps_since_tge` counter is rolled back. End users later
  claim from the holding contract — see
  [sweat-claim](https://github.com/sweatco/sweat-claim).
- **Denylist.** A `DenylistManager` role can mark accounts as restricted via
  `set_restricted`. Restricted accounts cannot send, receive, or burn tokens.
- **Pausability.** `PauseManager` / `UnpauseManager` roles can pause/unpause
  groups of methods. Two pause groups are defined: `minting` (covers
  `defer_batch`) and `token` (covers transfers and `burn`).
- **Access control.** All privileged roles (`Oracle`, `DenylistManager`,
  `PauseManager`, `UnpauseManager`, plus the Super Admin) are managed by the
  [`near-plugins`](https://github.com/aurora-is-near/near-plugins)
  `AccessControllable` plugin. Use `acl_grant_role` / `acl_revoke_role`.
- **NEP-141 / NEP-145 / NEP-148.** Standard FT core, storage management, and
  metadata are implemented. Transfers add denylist + pause checks on top of
  the standard.

See `recon-report.md` for a deeper architectural walkthrough.

## Module layout

```
contract/src/
├── api.rs         Public traits: SweatApi, RestrictionApi, SweatDefer
├── core.rs        NEP-141 transfers with denylist + pause checks
├── defer.rs       defer_batch / on_record — the deferred-mint flow
├── lib.rs         Contract state, init, plugin wiring
├── math.rs        Decaying issuance formula
├── meta.rs        NEP-148 metadata (SWEAT / SWEAT / 18 decimals)
├── migration.rs   Upgrade migration entry point
└── storage.rs     NEP-145 storage management
```

## Dependencies

- Rust toolchain (see `rust-toolchain.toml`) — `rustup target add wasm32-unknown-unknown`
- [`cargo-near`](https://github.com/near/cargo-near) for building the contract
- [`near-cli-rs`](https://github.com/near/near-cli-rs) for deployment / interaction
  (the legacy JS `near-cli` is **not** what the commands below use — install via
  `cargo install near-cli-rs` or the install script in that repo)
- Docker (only if you want reproducible builds via `make dock`)

## Vendored `near-contract-standards`

`vendors/near-contract-standards/` is a vendored copy of upstream
[`near-contract-standards`](https://crates.io/crates/near-contract-standards)
with local modifications (notably `LookupMapAdapter` for the SWEAT account-id
storage layout). **The vendored directory is the source of truth** — edit it
directly and commit. There is no separate patch file to keep in sync.

To refresh the vendor from upstream — for example after bumping `near-sdk` in
the workspace `Cargo.toml`:

```bash
make sync
```

The script reads the currently vendored version, diffs it against pristine
upstream to capture local changes, replaces the vendor with the target
upstream version (from `[workspace.dependencies] near-sdk`), and re-applies
the captured diff. If a hunk fails, `*.rej` files are written and the script
aborts — resolve manually and commit; the next `make sync` will derive a
fresh diff from the resolved state.

## Build, lint, test

All common tasks are wired through the `Makefile`:

```bash
make help              # list all targets
make install           # npm i near-cli && cargo build
make build             # local contract build (./scripts/build.sh)
make build-integration # contract build with `integration-test` feature
make build-stub        # build the holding-contract stub
make dock              # reproducible build in Docker
make test              # cargo test --package contract
make int               # integration tests (builds with integration-test feature first)
make int-log           # same, with logs streamed serially (override RUST_LOG)
make cov               # cargo llvm-cov coverage report
make fmt               # cargo +nightly fmt --all
make lint              # clippy
```

## Deploy & initialize

All examples below use `near-cli-rs`. Swap `network-config testnet` for
`network-config mainnet` as appropriate.

```bash
export TOKEN_ACCOUNT_ID=your-token-account-id
export HOLDING_ACCOUNT_ID=your-holding-account-id

near contract deploy $TOKEN_ACCOUNT_ID \
  use-file res/sweat.wasm \
  with-init-call new \
    json-args '{"postfix":".u.sweat.testnet","holding_account_id":"'$HOLDING_ACCOUNT_ID'"}' \
    prepaid-gas '100.0 Tgas' \
    attached-deposit '0 NEAR' \
  network-config testnet \
  sign-with-keychain \
  send
```

The deploying account becomes the **Super Admin** and can grant any role.

## Granting roles

Roles are managed through `near-plugins` ACL. Replace `Oracle` with
`DenylistManager`, `PauseManager`, or `UnpauseManager` as needed.

```bash
export ORACLE_ACCOUNT_ID=your-oracle-account-id

near contract call-function as-transaction $TOKEN_ACCOUNT_ID acl_grant_role \
  json-args '{"role":"Oracle","account_id":"'$ORACLE_ACCOUNT_ID'"}' \
  prepaid-gas '100.0 Tgas' \
  attached-deposit '0 NEAR' \
  sign-as $TOKEN_ACCOUNT_ID \
  network-config testnet \
  sign-with-keychain \
  send

near contract call-function as-read-only $TOKEN_ACCOUNT_ID acl_get_grantees \
  json-args '{"role":"Oracle","skip":0,"limit":100}' \
  network-config testnet now
```

## View methods

```bash
near contract call-function as-read-only $TOKEN_ACCOUNT_ID get_steps_since_tge \
  json-args '{}' network-config testnet now

near contract call-function as-read-only $TOKEN_ACCOUNT_ID formula \
  json-args '{"steps_since_tge":"1","steps":1000}' network-config testnet now

near contract call-function as-read-only $TOKEN_ACCOUNT_ID ft_balance_of \
  json-args '{"account_id":"some-account.testnet"}' network-config testnet now

near contract call-function as-read-only $TOKEN_ACCOUNT_ID ft_metadata \
  json-args '{}' network-config testnet now

near contract call-function as-read-only $TOKEN_ACCOUNT_ID get_holding_account_id \
  json-args '{}' network-config testnet now

near contract call-function as-read-only $TOKEN_ACCOUNT_ID is_restricted \
  json-args '{"account_id":"some-account.testnet"}' network-config testnet now
```

## Submit steps as an Oracle (deferred mint)

```bash
near contract call-function as-transaction $TOKEN_ACCOUNT_ID defer_batch \
  json-args '{"steps_batch":[["user-1.testnet",10000],["user-2.testnet",20000]]}' \
  prepaid-gas '300.0 Tgas' \
  attached-deposit '0 NEAR' \
  sign-as $ORACLE_ACCOUNT_ID \
  network-config testnet \
  sign-with-keychain \
  send
```

For each entry, 95% of the computed payout is minted to the holding contract
(claimable by the user via [sweat-claim](https://github.com/sweatco/sweat-claim))
and 5% is minted to `$ORACLE_ACCOUNT_ID` as a fee. If the XCC to the holding
contract fails, `steps_since_tge` is rolled back and nothing is minted.

## Transfers, burn, storage

```bash
# Transfer (subject to pause + denylist on sender & receiver)
near contract call-function as-transaction $TOKEN_ACCOUNT_ID ft_transfer \
  json-args '{"receiver_id":"<receiver>","amount":"100","memo":"hi"}' \
  prepaid-gas '30.0 Tgas' \
  attached-deposit '1 yoctoNEAR' \
  sign-as <sender> \
  network-config testnet sign-with-keychain send

# Burn (subject to pause + denylist on caller)
near contract call-function as-transaction $TOKEN_ACCOUNT_ID burn \
  json-args '{"amount":"100"}' \
  prepaid-gas '30.0 Tgas' \
  attached-deposit '1 yoctoNEAR' \
  sign-as <holder> \
  network-config testnet sign-with-keychain send

# NEP-145 storage registration
near contract call-function as-transaction $TOKEN_ACCOUNT_ID storage_deposit \
  json-args '{"account_id":"user-1.testnet"}' \
  prepaid-gas '30.0 Tgas' \
  attached-deposit '0.00235 NEAR' \
  sign-as <payer> \
  network-config testnet sign-with-keychain send

near contract call-function as-read-only $TOKEN_ACCOUNT_ID storage_balance_of \
  json-args '{"account_id":"user-1.testnet"}' network-config testnet now
```

## Denylist & pause operations

```bash
# Restrict / unrestrict an account (requires DenylistManager role)
near contract call-function as-transaction $TOKEN_ACCOUNT_ID set_restricted \
  json-args '{"account_id":"bad.testnet","is_restricted":true}' \
  prepaid-gas '30.0 Tgas' \
  attached-deposit '0 NEAR' \
  sign-as <denylist-manager> \
  network-config testnet sign-with-keychain send

# Pause / unpause a group of methods (requires PauseManager / UnpauseManager)
# Known pause groups: "minting" (covers defer_batch), "token" (covers transfers + burn)
near contract call-function as-transaction $TOKEN_ACCOUNT_ID pa_pause_feature \
  json-args '{"key":"minting"}' \
  prepaid-gas '30.0 Tgas' \
  attached-deposit '0 NEAR' \
  sign-as <pause-manager> \
  network-config testnet sign-with-keychain send

near contract call-function as-transaction $TOKEN_ACCOUNT_ID pa_unpause_feature \
  json-args '{"key":"minting"}' \
  prepaid-gas '30.0 Tgas' \
  attached-deposit '0 NEAR' \
  sign-as <unpause-manager> \
  network-config testnet sign-with-keychain send
```

## Deferred mint — sequence diagram

![Smart contracts interaction](doc/contracts_interaction.png)

For the full claim flow on the receiving side, see the
[sweat-claim repo](https://github.com/sweatco/sweat-claim).
