use anyhow::Result;
use neptune_rpc_client::http::HttpClient;

use crate::wallet::state::Wallet;

pub mod wallet;

#[tokio::main]
async fn main() -> Result<()> {
    let client = HttpClient::new("http://127.0.0.1:9797");
    let mnemonic = "belt expose monkey vapor tiny noble crater guilt have submit before fat rude tide shoulder practice hybrid record".to_string();
    let wallet = Wallet::new(client, mnemonic);

    wallet.main_loop().await;

    Ok(())
}
