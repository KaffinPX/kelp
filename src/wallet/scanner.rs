use neptune_privacy::{
    api::export::{Announcement, BlockHeight, Tip5},
    application::json_rpc::core::api::rpc::RpcApi,
    protocol::consensus::block::block_selector::BlockSelector,
};
use tracing::info;
use xnt_rpc_client::http::HttpClient;

use crate::{
    core::storage::WalletKeyspace,
    wallet::cache::{keys::KeysCache, utxos::UtxosCache},
};

#[derive(Clone)]
pub struct Scanner {
    client: HttpClient,
    storage: WalletKeyspace,
    pub keys: KeysCache,
    pub utxos: UtxosCache,
}

impl Scanner {
    pub fn new(
        client: HttpClient,
        storage: WalletKeyspace,
        keys: KeysCache,
        utxos: UtxosCache,
    ) -> Self {
        Scanner {
            client: client.clone(),
            storage,
            keys,
            utxos,
        }
    }

    pub fn height(&self) -> BlockHeight {
        self.storage.get_height()
    }

    // TODO: rewrite scanner with batching, reorg support etc.
    pub async fn scan(&self) {
        let remote_height = self.client.height().await.unwrap().height;
        let mut start_height = self.storage.get_height(); 
        let initial_height = start_height;

        while start_height <= remote_height {
            let current_height = start_height;

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

            start_height = start_height.next();
        }

        if start_height > initial_height {
            self.utxos.write().await.sync_proofs().await;
            self.storage.set_height(start_height);
        }
    }
}
