use std::sync::Arc;

use neptune_privacy::{
    api::export::{NativeCurrencyAmount, Tip5, Utxo},
    application::json_rpc::core::api::rpc::RpcApi,
    util_types::mutator_set::{
        ms_membership_proof::MsMembershipProof, mutator_set_accumulator::MutatorSetAccumulator,
    },
};
use num_traits::ops::checked::CheckedSub;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::info;
use xnt_rpc_client::http::HttpClient;

use crate::core::storage::{UtxoKey, UtxosKeyspace};

#[derive(Clone, Serialize, Deserialize)]
pub struct LockedUtxo {
    pub utxo: Utxo,
    pub membership_proof: MsMembershipProof,
}

impl LockedUtxo {
    pub fn new(utxo: Utxo, membership_proof: MsMembershipProof) -> LockedUtxo {
        LockedUtxo {
            utxo,
            membership_proof,
        }
    }
}

#[derive(Clone)]
pub struct Utxos {
    client: HttpClient,
    storage: UtxosKeyspace,
    pub summary: NativeCurrencyAmount,
}

impl Utxos {
    pub fn new(client: HttpClient, storage: UtxosKeyspace) -> Self {
        info!("Initializing UTXOs cache...");

        let mut utxos = Utxos {
            client,
            storage,
            summary: NativeCurrencyAmount::from_nau(0),
        };
        utxos.load();
        utxos
    }

    pub fn record(&mut self, utxo: Utxo, membership_proof: MsMembershipProof) {
        let utxo_key = UtxoKey::new(membership_proof.aocl_leaf_index, Tip5::hash(&utxo));
        let utxo_amount = utxo.get_native_currency_amount();

        if self
            .storage
            .put(utxo_key, LockedUtxo::new(utxo, membership_proof))
        {
            self.summary += utxo_amount;
        }
    }

    pub async fn sync_proofs(&mut self) {
        let mut index_sets = Vec::new();

        for (key, utxo) in self.storage.iter() {
            index_sets.push(utxo.membership_proof.compute_indices(key.extract_digest()));
        }

        let membership_snapshot = self
            .client
            .restore_membership_proof(index_sets)
            .await
            .unwrap()
            .snapshot;

        let mut utxo_count = 0;
        for ((utxo_key, mut utxo), membership_proof) in self
            .storage
            .iter()
            .zip(membership_snapshot.membership_proofs.into_iter())
        {
            utxo.membership_proof = membership_proof
                .extract_ms_membership_proof(
                    utxo.membership_proof.aocl_leaf_index,
                    utxo.membership_proof.sender_randomness,
                    utxo.membership_proof.receiver_preimage,
                )
                .unwrap();

            self.storage.put(utxo_key, utxo);
            utxo_count += 1;
        }

        info!(
            "Synced membership proofs of {} UTXOs successfully.",
            utxo_count
        );
        self.prune(membership_snapshot.synced_mutator_set.into());
    }

    fn prune(&mut self, msa: MutatorSetAccumulator) {
        for (key, utxo) in self.storage.iter() {
            let is_available = msa.verify(key.extract_digest(), &utxo.membership_proof);

            if !is_available {
                let amount = utxo.utxo.get_native_currency_amount();
                info!(
                    "UTXO on leaf index {} is spent ({} XNT).",
                    utxo.membership_proof.aocl_leaf_index, amount
                );

                self.storage.remove(key);
                self.summary = self.summary.checked_sub(&amount).unwrap();
            }
        }
    }

    fn load(&mut self) {
        let mut utxo_count = 0;

        for (_, utxo) in self.storage.iter() {
            self.summary += utxo.utxo.get_native_currency_amount();
            utxo_count += 1;
        }

        info!("Loaded {} UTXOs containing {} XNT.", utxo_count, self.summary);
    }
}

pub type UtxosCache = Arc<RwLock<Utxos>>;
