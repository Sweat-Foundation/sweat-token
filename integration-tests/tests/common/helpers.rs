use tracing_subscriber::EnvFilter;

pub(crate) fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,near_workspaces=warn"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_test_writer()
        .try_init();
}

pub fn payout(value: u128) -> (u128, u128) {
    let fee = (value * 5).div_ceil(100);
    (value - fee, fee)
}
