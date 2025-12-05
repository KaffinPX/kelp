use std::sync::{Arc, RwLock};

use neptune_cash::{
    api::export::{Announcement, KeyType, UtxoTriple},
    application::json_rpc::core::model::block::transaction_kernel::RpcChunkDictionary,
    prelude::twenty_first::prelude::MmrMembershipProof,
    state::wallet::wallet_entropy::WalletEntropy,
    util_types::mutator_set::ms_membership_proof::MsMembershipProof,
};

use crate::wallet::{
    cache::Keys,
    utils::announcement::{extract_ciphertext, extract_receiver_identifier},
};

#[derive(Clone)]
pub struct Wallet {
    keys: Arc<RwLock<Keys>>,
}

impl Wallet {
    pub fn new(mnemonic: String) -> Self {
        let words: Vec<String> = mnemonic.split(' ').map(|p| p.to_string()).collect();
        let entropy = WalletEntropy::from_phrase(&words).unwrap();

        Wallet {
            keys: Arc::new(RwLock::new(Keys::new(entropy))),
        }
    }

    pub fn scan(&self, announcements: Vec<Announcement>) -> Vec<(UtxoTriple, MsMembershipProof)> {
        let keys = self.keys.read().unwrap();
        let mut utxos = Vec::new();

        for (key_type, key) in keys.all_keys() {
            let receiver_identifier = key.receiver_identifier();
            let found_utxos: Vec<(UtxoTriple, MsMembershipProof)> = announcements
                .iter()
                .filter(|a| matches!(KeyType::try_from(*a), Ok(k) if k == key_type))
                .filter(|a| matches!(extract_receiver_identifier(a), Some(i) if i == receiver_identifier))
                .filter_map(|a| extract_ciphertext(&a))
                .filter_map(|c| key.decrypt(&c).ok())
                .map(|(utxo, sender_randomness)| {
                    (UtxoTriple { utxo, sender_randomness, receiver_digest: key.to_address().privacy_digest()},
                    MsMembershipProof { sender_randomness, receiver_preimage: key.privacy_preimage(), auth_path_aocl: MmrMembershipProof::new(vec![]), aocl_leaf_index: 0, target_chunks: RpcChunkDictionary::default().into()})
                })
                .collect();
            utxos.extend(found_utxos);
        }

        utxos
    }
}
