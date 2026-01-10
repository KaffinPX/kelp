use std::sync::Arc;

use neptune_privacy::{
    api::export::{Announcement, BlockHeight, Tip5},
    application::json_rpc::core::api::rpc::RpcApi,
    protocol::consensus::block::block_selector::BlockSelector,
};
use tokio::sync::RwLock;
use tracing::info;
use xnt_rpc_client::http::HttpClient;

use crate::wallet::cache::{keys::KeysCache, utxos::UtxosCache};

#[derive(Clone)]
pub struct Scanner {
    client: HttpClient,
    pub height: Arc<RwLock<BlockHeight>>,
    pub keys: KeysCache,
    pub utxos: UtxosCache,
}

impl Scanner {
    pub fn new(client: HttpClient, keys: KeysCache, utxos: UtxosCache) -> Self {
        Scanner {
            client: client.clone(),
            height: Arc::new(RwLock::new(BlockHeight::new(12740.into()))),
            keys,
            utxos,
        }
    }

    pub async fn scan(&self) {
        let remote_height = self.client.height().await.unwrap().height;
        let mut height_guard = self.height.write().await;

        while *height_guard <= remote_height {
            let current_height = *height_guard;

            let transaction_kernel = self
                .client
                .get_block_transaction_kernel(BlockSelector::Height(current_height))
                .await
                .unwrap()
                .kernel
                .unwrap();
            let announcements: Vec<Announcement> = transaction_kernel
                .announcements
                .clone()
                .into_iter()
                .map(Into::into)
                .collect();

            let utxos = self.keys.read().await.scan(announcements);

            for (utxo, mut mock_proof) in utxos {
                let commitment = mock_proof
                    .addition_record(Tip5::hash(&utxo))
                    .canonical_commitment;
                let index = transaction_kernel
                    .outputs
                    .iter()
                    .position(|r| r.0 == commitment)
                    .unwrap(); // This might panic bcs of a malicious announcement.
                info!(
                    "Found {} on block {current_height} on index {}",
                    commitment.to_hex(),
                    index
                );

                let block_body = self
                    .client
                    .get_block_body(BlockSelector::Height(current_height))
                    .await
                    .unwrap()
                    .body
                    .unwrap();

                mock_proof.aocl_leaf_index =
                    block_body.mutator_set_accumulator.aocl.leaf_count - index as u64 + 2;

                self.utxos.write().await.record(utxo, mock_proof);
            }

            *height_guard = current_height.next();
        }

        self.utxos.write().await.sync_proofs().await;
    }
}
