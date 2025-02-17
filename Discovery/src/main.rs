/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

// Changed
use clap::Parser;

mod app;
mod auth;
mod error;
mod graph;
mod resources;
mod state;

use crate::app::{App, Config};
use error::Result;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let config = Config::parse();
    App::new(config).await?.run().await?;
    Ok(())
}
