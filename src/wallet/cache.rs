use std::collections::HashMap;

use neptune_cash::{
    api::export::{KeyType, SpendingKey},
    state::wallet::wallet_entropy::WalletEntropy,
};

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

    pub fn all_keys(&self) -> impl Iterator<Item = (KeyType, &SpendingKey)> {
        self.keys
            .iter()
            .flat_map(|(k_type, list)| list.iter().map(move |k| (*k_type, k)))
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
}
