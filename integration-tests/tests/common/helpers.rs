use tracing_subscriber::EnvFilter;

/// Installs a `tracing` subscriber once per test process. Idempotent —
/// safe to call from every test setup.
///
/// Output is routed through `with_test_writer()` so `cargo test`'s
/// stdout capture works (otherwise spans bypass capture and leak into
/// passing-test output).
///
/// Override verbosity with `RUST_LOG`, e.g.
/// `RUST_LOG=debug cargo test -- --nocapture`.
///
/// Default filter is `info,near_workspaces=warn` — info-level events from
/// our tests, but quiet down near-workspaces' chatty sandbox lifecycle logs.
pub(crate) fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,near_workspaces=warn"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_test_writer()
        .try_init();
}

/// Splits a minted amount into `(amount_for_user, fee)` using the contract's
/// 5%-oracle-fee policy (rounded up). Mirrors `Contract::calculate_tokens_amount`
/// in the sweat contract; kept in sync so tests can predict expected balances
/// without calling into the contract.
pub fn payout(value: u128) -> (u128, u128) {
    let fee = (value * 5).div_ceil(100);
    (value - fee, fee)
}
