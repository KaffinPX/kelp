use std::{sync::Arc, time::Duration};

use neptune_privacy::state::wallet::wallet_entropy::WalletEntropy;
use tokio::sync::RwLock;
use xnt_rpc_client::http::HttpClient;

use crate::{
    core::storage::Storage,
    wallet::{
        cache::{
            keys::{Keys, KeysCache},
            utxos::{Utxos, UtxosCache},
        },
        scanner::Scanner,
    },
};

#[derive(Clone)]
pub struct Wallet {
    pub keys: KeysCache,
    pub utxos: UtxosCache,
    pub scanner: Arc<Scanner>,
}

impl Wallet {
    pub fn new(client: HttpClient, mnemonic: Option<String>) -> Self {
        let storage = Storage::new("./wallet");
        let entropy = match mnemonic {
            Some(m) => {
                if storage.keys.get_mnemonic().is_some() {
                    panic!("A wallet is imported already")
                }

                let words: Vec<String> = m.split(' ').map(|p| p.to_string()).collect();
                let entropy = WalletEntropy::from_phrase(&words).expect("Invalid mnemonic");

                storage.keys.set_mnemonic(&m);
                entropy
            }
            None => {
                let mnemonic = storage
                    .keys
                    .get_mnemonic()
                    .expect("There is no wallet imported");
                let words: Vec<String> = mnemonic.split(' ').map(|p| p.to_string()).collect();
                let entropy = WalletEntropy::from_phrase(&words).expect("Corrupted wallet dir");

                entropy
            }
        };

        let keys = Arc::new(RwLock::new(Keys::new(entropy)));
        let utxos = Arc::new(RwLock::new(Utxos::new(client.clone())));

        Wallet {
            // Ideally we should have a default in-memory storage and a Trait and a backend in a seperate crate prob AND read height and UTXOs always from db
            keys: keys.clone(),
            utxos: utxos.clone(),
            scanner: Arc::new(Scanner::new(client, keys, utxos)),
        }
    }

    pub async fn main_loop(&self) {
        let mut interval = tokio::time::interval(Duration::from_secs(10));

        loop {
            interval.tick().await;
            self.scanner.scan().await;
        }
    }
}
