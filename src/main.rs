use anyhow::Result;
use neptune_cash::api::export::Announcement;
use neptune_cash::api::export::BlockHeight;
use neptune_cash::api::export::Tip5;
use neptune_cash::application::json_rpc::core::api::rpc::RpcApi;
use neptune_cash::prelude::triton_vm::prelude::BFieldElement;
use neptune_cash::protocol::consensus::block::block_selector::BlockSelector;
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
