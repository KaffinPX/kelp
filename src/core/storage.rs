use std::marker::PhantomData;
use std::path::Path;

use fjall::{KeyspaceCreateOptions, Readable, SingleWriterTxDatabase, SingleWriterTxKeyspace};
use neptune_privacy::api::export::KeyType;
use serde_json;

use crate::wallet::cache::utxos::LockedUtxo;

pub type KeysKeyspace = Keyspace<u64>;
pub type UtxosKeyspace = Keyspace<LockedUtxo>;

pub const KEYSPACE_KEYS: &str = "keys";
pub const KEYSPACE_UTXOS: &str = "utxos";

pub struct Storage {
    pub keys: KeysKeyspace,
    pub utxos: UtxosKeyspace,
}

impl Storage {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        let db = SingleWriterTxDatabase::builder(path).open().unwrap();

        Storage {
            keys: Keyspace::new(db.clone(), KEYSPACE_KEYS),
            utxos: Keyspace::new(db, KEYSPACE_UTXOS),
        }
    }
}

#[derive(Clone)]
pub struct Keyspace<V> {
    db: SingleWriterTxDatabase,
    pub handle: SingleWriterTxKeyspace,
    pub value: PhantomData<V>,
}

impl<V> Keyspace<V> {
    pub fn new(db: SingleWriterTxDatabase, name: &str) -> Self {
        let handle = db.keyspace(name, KeyspaceCreateOptions::default).unwrap();

        Self {
            db,
            handle,
            value: PhantomData,
        }
    }
}

impl Keyspace<u64> {
    pub fn set_mnemonic(&self, mnemonic: &str) {
        self.handle
            .insert("mnemonic", mnemonic.as_bytes().to_vec())
            .unwrap();
    }

    pub fn get_mnemonic(&self) -> Option<String> {
        self.handle.get("mnemonic").unwrap().map(|bytes| {
            String::from_utf8(bytes.to_vec()).expect("stored mnemonic is not valid UTF-8")
        })
    }

    pub fn get(&self, key: KeyType) -> u64 {
        self.handle
            .get([key as u8])
            .unwrap()
            .map(|bytes| u64::from_be_bytes(bytes.as_ref().try_into().unwrap()))
            .unwrap_or(1)
    }

    pub fn increment(&self, key: KeyType) {
        self.handle
            .fetch_update([key as u8], |old_value| {
                let value = old_value
                    .map(|bytes| u64::from_be_bytes(bytes.as_ref().try_into().unwrap()))
                    .unwrap_or(1)
                    + 1;
                Some(value.to_be_bytes().into())
            })
            .unwrap();
    }
}

impl Keyspace<LockedUtxo> {
    pub fn get<K: AsRef<[u8]>>(&self, key: K) -> Option<LockedUtxo> {
        self.handle
            .get(key)
            .unwrap()
            .map(|bytes| serde_json::from_slice(&bytes).expect("invalid utxo json"))
    }

    pub fn remove<K: AsRef<[u8]>>(&self, key: K) {
        self.handle.remove(key.as_ref()).unwrap();
    }

    pub fn set<K: AsRef<[u8]>>(&self, key: K, utxo: LockedUtxo) {
        self.handle
            .insert(
                key.as_ref(),
                serde_json::to_vec(&utxo).expect("utxo serialization failed"),
            )
            .unwrap();
    }

    pub fn iter(&self) -> impl Iterator<Item = LockedUtxo> + '_ {
        let tx = self.db.read_tx();
        tx.iter(&self.handle).map(|guard| {
            serde_json::from_slice(&guard.value().unwrap()).expect("invalid utxo json")
        })
    }
}
