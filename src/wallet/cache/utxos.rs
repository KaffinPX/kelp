use neptune_privacy::{
    api::export::{NativeCurrencyAmount, Tip5, Utxo},
    util_types::mutator_set::{
        ms_membership_proof::MsMembershipProof, mutator_set_accumulator::MutatorSetAccumulator,
    },
};

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
    pub summary: NativeCurrencyAmount,
    pub utxos: Vec<LockedUtxo>,
}

impl Utxos {
    pub fn new() -> Self {
        Utxos {
            summary: NativeCurrencyAmount::from_nau(0),
            utxos: vec![],
        }
    }

    pub fn record_utxo(&mut self, utxo: Utxo, membership_proof: MsMembershipProof) {
        self.summary = self.summary + utxo.get_native_currency_amount();
        self.utxos.push(LockedUtxo::new(utxo, membership_proof));
    }

    pub fn prune_spent(&mut self, msa: MutatorSetAccumulator) {
        for utxo in &self.utxos {
            let is_valid = msa.verify(Tip5::hash(&utxo.utxo), &utxo.membership_proof);
        }
    }
}
