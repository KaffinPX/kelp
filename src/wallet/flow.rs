use std::{sync::Arc, time::Duration};

use neptune_privacy::state::wallet::wallet_entropy::WalletEntropy;
use tokio::sync::RwLock;
use xnt_rpc_client::http::HttpClient;

use crate::{
    core::storage::{KeysKeyspace, Storage},
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
        let Storage {
            keys,
            utxos,
            wallet,
        } = Storage::new("./wallet");
        Self::initialize_mnemonic(&keys, mnemonic);

        let keys = Arc::new(RwLock::new(Keys::new(keys)));
        let utxos = Arc::new(RwLock::new(Utxos::new(client.clone(), utxos)));

        Wallet {
            keys: keys.clone(),
            utxos: utxos.clone(),
            scanner: Arc::new(Scanner::new(client, wallet, keys, utxos)),
        }
    }

    pub async fn main_loop(&self) {
        let mut interval = tokio::time::interval(Duration::from_secs(10));

        loop {
            interval.tick().await;
            self.scanner.scan().await;
        }
    }

    fn initialize_mnemonic(storage: &KeysKeyspace, mnemonic: Option<String>) {
        let Some(m) = mnemonic else {
            return;
        };
        if storage.get_mnemonic().is_some() {
            panic!("wallet already initialized; cannot overwrite mnemonic");
        }

        let words: Vec<String> = m.split_whitespace().map(String::from).collect();
        WalletEntropy::from_phrase(&words).expect("mnemonic validation failed");

        storage.set_mnemonic(&m);
    }
}
