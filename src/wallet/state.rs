use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

use neptune_cash::{
    api::export::{Announcement, BlockHeight, Tip5},
    application::json_rpc::core::api::rpc::RpcApi,
    protocol::consensus::block::block_selector::BlockSelector,
    state::wallet::wallet_entropy::WalletEntropy,
};
use neptune_rpc_client::http::HttpClient;

use crate::wallet::cache::{keys::Keys, utxos::Utxos};

#[derive(Clone)]
pub struct Wallet {
    client: HttpClient,
    height: Arc<RwLock<BlockHeight>>,
    keys: Arc<RwLock<Keys>>,
    utxos: Arc<RwLock<Utxos>>,
}

impl Wallet {
    pub fn new(client: HttpClient, mnemonic: String) -> Self {
        let words: Vec<String> = mnemonic.split(' ').map(|p| p.to_string()).collect();
        let entropy = WalletEntropy::from_phrase(&words).unwrap();

        Wallet {
            client,
            // Ideally we should have a default in-memory storage and a Trait and a backend in a seperate crate prob AND read height and UTXOs always from db
            height: Arc::new(RwLock::new(BlockHeight::new(17500.into()))),
            keys: Arc::new(RwLock::new(Keys::new(entropy))),
            utxos: Arc::new(RwLock::new(Utxos::new())),
        }
    }

    pub async fn main_loop(&self) {
        let mut interval = tokio::time::interval(Duration::from_secs(10));

        loop {
            interval.tick().await;
            self.scan_blocks().await;
        }
    }

    pub async fn scan_blocks(&self) {
        let remote_height = self.client.height().await.unwrap().height;
        let remote_height: BlockHeight = remote_height.into(); // TODO: Can be removed after "mining" PR

        let mut height_guard = self.height.write().unwrap();

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

            let utxos = self.keys.read().unwrap().scan(announcements);

            for (utxo, mut mock_proof) in utxos {
                let commitment = mock_proof
                    .addition_record(Tip5::hash(&utxo))
                    .canonical_commitment;
                let index = transaction_kernel
                    .outputs
                    .iter()
                    .position(|r| r.0 == commitment)
                    .unwrap(); // This might panic bcs of a malicious announcement.
                println!(
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
                    block_body.mutator_set_accumulator.aocl.leaf_count - index as u64 + 1;
                let absolute_index_set = mock_proof.compute_indices(Tip5::hash(&utxo));

                self.utxos
                    .write()
                    .unwrap()
                    .record_utxo(utxo, absolute_index_set.into());
            }

            *height_guard = current_height.next();
        }
    }
}
