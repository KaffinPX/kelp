use std::panic;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;
use xnt_rpc_client::http::HttpClient;

use crate::wallet::flow::Wallet;

pub mod core;
pub mod wallet;

#[derive(Parser)]
#[command(name = "kelp")]
#[command(about = "A Neptune daemon wallet")]
struct Args {
    /// Mnemonic to import
    #[arg(long)]
    mnemonic: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let filter = EnvFilter::try_from_default_env().unwrap_or(EnvFilter::new("kelp=info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        default_hook(panic_info);
        std::process::exit(1);
    }));

    let client = HttpClient::new("http://45.149.206.49:8080");
    let wallet = Wallet::new(client, args.mnemonic);

    core::console::start_console(wallet.clone()).await;

    wallet.main_loop().await;
    Ok(())
}
