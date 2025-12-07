use neptune_cash::{
    api::export::{NativeCurrencyAmount, Utxo},
    application::json_rpc::core::model::block::transaction_kernel::RpcAbsoluteIndexSet,
};

#[derive(Clone)]
pub struct Utxos {
    pub summary: NativeCurrencyAmount,
}

impl Utxos {
    pub fn new() -> Self {
        Utxos {
            summary: NativeCurrencyAmount::from_nau(0),
        }
    }

    pub fn record_utxo(&mut self, utxo: Utxo, absolute_index_set: RpcAbsoluteIndexSet) {
        self.summary = self.summary + utxo.get_native_currency_amount();
    }
}
