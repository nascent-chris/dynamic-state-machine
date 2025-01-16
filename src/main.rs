mod config;
mod models;
mod state_machine;

use state_machine::StateMachine;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let mut state_machine = StateMachine::new("config.json").await?;
    state_machine.run().await?;

    tracing::info!("State machine execution completed.");
    Ok(())
}
