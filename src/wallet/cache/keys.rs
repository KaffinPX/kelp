use std::collections::HashMap;

use neptune_privacy::{
    api::export::{Announcement, KeyType, SpendingKey, Utxo},
    application::json_rpc::core::model::block::transaction_kernel::RpcChunkDictionary,
    prelude::twenty_first::prelude::MmrMembershipProof,
    state::wallet::wallet_entropy::WalletEntropy,
    util_types::mutator_set::ms_membership_proof::MsMembershipProof,
};

use crate::wallet::utils::announcement::{extract_ciphertext, extract_receiver_identifier};

#[derive(Clone)]
pub struct Keys {
    entropy: WalletEntropy,
    keys: HashMap<KeyType, Vec<SpendingKey>>,
}

impl Keys {
    pub fn new(entropy: WalletEntropy) -> Self {
        let mut keys = Keys {
            entropy,
            keys: HashMap::new(),
        };

        keys.derive_next_key(KeyType::Generation);
        keys.derive_next_key(KeyType::Symmetric);

        keys
    }

    pub fn current_key(&self, key_type: KeyType) -> &SpendingKey {
        self.keys
            .get(&key_type)
            .and_then(|keys| keys.last())
            .unwrap()
    }

    pub fn derive_next_key(&mut self, key_type: KeyType) {
        let index = self.keys.get(&key_type).map_or(0, |v| v.len() as u64);

        let new_key = match key_type {
            KeyType::Generation => self.entropy.nth_generation_spending_key(index).into(),
            KeyType::Symmetric => self.entropy.nth_symmetric_key(index).into(),
        };

        self.keys
            .entry(key_type)
            .or_insert_with(Vec::new)
            .push(new_key);
    }

    pub(crate) fn all_keys(&self) -> impl Iterator<Item = (KeyType, &SpendingKey)> {
        self.keys
            .iter()
            .flat_map(|(k_type, list)| list.iter().map(move |k| (*k_type, k)))
    }

    pub fn scan(&self, announcements: Vec<Announcement>) -> Vec<(Utxo, MsMembershipProof)> {
        let mut utxos = Vec::new();

        for (key_type, key) in self.all_keys() {
            let receiver_identifier = key.receiver_identifier();
            let found_utxos: Vec<(Utxo, MsMembershipProof)> = announcements
                .iter()
                .filter(|a| matches!(KeyType::try_from(*a), Ok(k) if k == key_type))
                .filter(|a| matches!(extract_receiver_identifier(a), Some(i) if i == receiver_identifier))
                .filter_map(|a| extract_ciphertext(&a))
                .filter_map(|c| key.decrypt(&c).ok())
                .map(|(utxo, sender_randomness)| {
                    (
                      utxo,
                      MsMembershipProof {
                        sender_randomness,
                        receiver_preimage: key.privacy_preimage(),
                        auth_path_aocl: MmrMembershipProof::new(vec![]),
                        aocl_leaf_index: 0,
                        target_chunks: RpcChunkDictionary::default().into()
                      }
                    )
                })
                .collect();
            utxos.extend(found_utxos);
        }

        utxos
    }
}
