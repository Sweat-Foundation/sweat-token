macro_rules! step {
    ($tag:expr, $($arg:tt)*) => {
        println!("• [{}] {}", $tag, format!($($arg)*));
    };
}

pub(crate) use step;

/// Splits a minted amount into `(amount_for_user, fee)` using the contract's
/// 5%-oracle-fee policy (rounded up). Mirrors `Contract::calculate_tokens_amount`
/// in the sweat contract; kept in sync so tests can predict expected balances
/// without calling into the contract.
pub fn payout(value: u128) -> (u128, u128) {
    let fee = (value * 5).div_ceil(100);
    (value - fee, fee)
}
