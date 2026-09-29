use field_service_onboarding::field_onboarding::{
    onboard_technician, FieldServiceSignup, InfraiClient,
};

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let input_path = std::env::args()
        .nth(1)
        .ok_or("usage: cargo run --bin field_onboarding -- signup.json")?;
    let input = std::fs::read_to_string(input_path)?;
    let signup: FieldServiceSignup = serde_json::from_str(&input)?;
    let client = InfraiClient::from_env()?;
    let receipt = onboard_technician(&client, signup).await?;
    println!("{}", serde_json::to_string_pretty(&receipt)?);
    Ok(())
}
