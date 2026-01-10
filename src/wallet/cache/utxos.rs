use std::sync::Arc;

use neptune_privacy::{
    api::export::{NativeCurrencyAmount, Tip5, Utxo},
    application::json_rpc::core::api::rpc::RpcApi,
    util_types::mutator_set::{
        ms_membership_proof::MsMembershipProof, mutator_set_accumulator::MutatorSetAccumulator,
    },
};
use num_traits::ops::checked::CheckedSub;
use tokio::sync::RwLock;
use tracing::info;
use xnt_rpc_client::http::HttpClient;

#[derive(Clone)]
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
    pub summary: NativeCurrencyAmount,
    pub utxos: Vec<LockedUtxo>,
}

impl Utxos {
    pub fn new(client: HttpClient) -> Self {
        Utxos {
            client,
            summary: NativeCurrencyAmount::from_nau(0),
            utxos: vec![],
        }
    }

    pub fn record(&mut self, utxo: Utxo, membership_proof: MsMembershipProof) {
        self.summary = self.summary + utxo.get_native_currency_amount();
        self.utxos.push(LockedUtxo::new(utxo, membership_proof));
    }

    pub async fn sync_proofs(&mut self) {
        let mut index_sets = Vec::with_capacity(self.utxos.len());

        for utxo in &self.utxos {
            let item = Tip5::hash(&utxo.utxo);
            index_sets.push(utxo.membership_proof.compute_indices(item));
        }

        let membership_snapshot = self
            .client
            .restore_membership_proof(index_sets)
            .await
            .unwrap()
            .snapshot;

        for (utxo, membership_proof) in self
            .utxos
            .iter_mut()
            .zip(membership_snapshot.membership_proofs.into_iter())
        {
            utxo.membership_proof = membership_proof
                .extract_ms_membership_proof(
                    utxo.membership_proof.aocl_leaf_index,
                    utxo.membership_proof.sender_randomness,
                    utxo.membership_proof.receiver_preimage,
                )
                .unwrap();
        }

        self.prune(membership_snapshot.synced_mutator_set.into());
    }

    fn prune(&mut self, msa: MutatorSetAccumulator) {
        self.utxos.retain(|utxo| {
            let is_spendable = msa.verify(Tip5::hash(&utxo.utxo), &utxo.membership_proof);

            if !is_spendable {
                let amount = utxo.utxo.get_native_currency_amount();
                self.summary = self.summary.checked_sub(&amount).unwrap();

                info!(
                    "UTXO on leaf index {} is spent ({} XNT).",
                    utxo.membership_proof.aocl_leaf_index, amount
                );
            }

            is_spendable
        });
    }
}

pub type UtxosCache = Arc<RwLock<Utxos>>;
