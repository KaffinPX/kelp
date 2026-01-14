use std::marker::PhantomData;
use std::path::Path;

use fjall::{KeyspaceCreateOptions, Readable, SingleWriterTxDatabase, SingleWriterTxKeyspace};
use neptune_privacy::api::export::{BlockHeight, Digest, KeyType};
use serde_json;

use crate::wallet::cache::utxos::LockedUtxo;

pub type KeysKeyspace = Keyspace<KeyType, u64>;
pub type UtxosKeyspace = Keyspace<UtxoKey, LockedUtxo>;
pub type WalletKeyspace = Keyspace<(), ()>;

pub const KEYSPACE_KEYS: &str = "keys";
pub const KEYSPACE_UTXOS: &str = "utxos";
pub const KEYSPACE_WALLET: &str = "wallet";

pub struct Storage {
    pub keys: KeysKeyspace,
    pub utxos: UtxosKeyspace,
    pub wallet: WalletKeyspace,
}

impl Storage {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        let db = SingleWriterTxDatabase::builder(path).open().unwrap();

        Storage {
            keys: Keyspace::new(db.clone(), KEYSPACE_KEYS),
            utxos: Keyspace::new(db.clone(), KEYSPACE_UTXOS),
            wallet: Keyspace::new(db, KEYSPACE_WALLET),
        }
    }
}

#[derive(Clone)]
pub struct Keyspace<K, V> {
    db: SingleWriterTxDatabase,
    pub handle: SingleWriterTxKeyspace,
    pub key: PhantomData<K>,
    pub value: PhantomData<V>,
}

impl<K, V> Keyspace<K, V> {
    pub fn new(db: SingleWriterTxDatabase, name: &str) -> Self {
        let handle = db.keyspace(name, KeyspaceCreateOptions::default).unwrap();

        Self {
            db,
            handle,
            key: PhantomData,
            value: PhantomData,
        }
    }
}

impl Keyspace<KeyType, u64> {
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
}

#[derive(Clone)]
pub struct UtxoKey(Vec<u8>);

impl UtxoKey {
    pub fn new(leaf_index: u64, digest: Digest) -> Self {
        let mut key = Vec::new();

        key.extend_from_slice(&leaf_index.to_be_bytes());
        key.extend_from_slice(digest.to_hex().as_bytes());

        Self(key)
    }

    pub fn extract_digest(&self) -> Digest {
        let digest_hex_bytes = &self.0[8..];
        let digest_hex = String::from_utf8(digest_hex_bytes.to_vec()).unwrap();

        Digest::try_from_hex(&digest_hex).unwrap()
    }
}

impl AsRef<[u8]> for UtxoKey {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl Keyspace<UtxoKey, LockedUtxo> {
    pub fn get(&self, key: UtxoKey) -> Option<LockedUtxo> {
        self.handle
            .get(key)
            .unwrap()
            .map(|bytes| serde_json::from_slice(&bytes).expect("invalid utxo json"))
    }

    pub fn put(&self, key: UtxoKey, utxo: LockedUtxo) -> bool {
        self.handle
            .fetch_update(key.as_ref(), |_| {
                Some(
                    serde_json::to_vec(&utxo)
                        .expect("utxo serialization failed")
                        .into(),
                )
            })
            .unwrap()
            .is_none()
    }

    pub fn remove(&self, key: UtxoKey) {
        self.handle.remove(key.as_ref()).unwrap();
    }

    pub fn iter(&self) -> impl Iterator<Item = (UtxoKey, LockedUtxo)> + '_ {
        let tx = self.db.read_tx();
        tx.iter(&self.handle).map(|guard| {
            let (key, value) = guard.into_inner().unwrap();
            (
                UtxoKey(key.to_vec()),
                serde_json::from_slice(&value).expect("invalid utxo json"),
            )
        })
    }
}

impl Keyspace<(), ()> {
    pub fn set_height(&self, height: BlockHeight) {
        self.handle
            .insert("height", height.value().to_be_bytes())
            .unwrap();
    }

    pub fn get_height(&self) -> BlockHeight {
        self.handle
            .get("height")
            .unwrap()
            .map(|bytes| u64::from_be_bytes(bytes.to_vec().try_into().unwrap()).into())
            .unwrap_or(BlockHeight::genesis())
    }
}
