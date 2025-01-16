mod config;
mod models;
mod state_machine;

use anyhow::Result;
use state_machine::StateMachine;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let mut state_machine = StateMachine::new("config.json").await?;
    state_machine.run().await?;

    tracing::info!("State machine execution completed.");
    Ok(())
}
