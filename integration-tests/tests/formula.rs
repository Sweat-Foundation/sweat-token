use approx::abs_diff_eq;
use serde::Deserialize;
use serde_json::json;
use tracing::info;

mod common;
use common::prepare::Context;

const EPS: f64 = 0.00001;
const CASES_JSON: &str = include_str!("../data/formula_cases.json");

#[derive(Deserialize)]
struct Case {
    steps_since_tge: String,
    steps: u32,
    expected: f64,
}

#[tokio::test]
#[tracing::instrument]
async fn test_formula() -> anyhow::Result<()> {
    let context = Context::builder().build().await?;

    info!("view get_steps_since_tge");
    let steps_since_tge: String = context.sweat.view("get_steps_since_tge").await?.json()?;
    info!(value = %steps_since_tge, "steps_since_tge");
    assert_eq!(0_u64, steps_since_tge.parse::<u64>()?);

    let cases: Vec<Case> = serde_json::from_str(CASES_JSON)?;
    info!(cases = cases.len(), "iterating formula cases");

    for (i, case) in cases.iter().enumerate() {
        let raw: String = context
            .sweat
            .view("formula")
            .args_json(json!({ "steps_since_tge": case.steps_since_tge, "steps": case.steps }))
            .await?
            .json()?;
        let value = raw.parse::<u128>()? as f64 / 1e18;
        assert!(
            abs_diff_eq!(value, case.expected, epsilon = EPS),
            "case #{i} (tge={}, steps={}): expected {}, got {} (diff {})",
            case.steps_since_tge,
            case.steps,
            case.expected,
            value,
            value - case.expected,
        );
    }

    info!(passed = cases.len(), "all cases passed");
    Ok(())
}
