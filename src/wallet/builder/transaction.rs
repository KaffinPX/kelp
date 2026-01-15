use itertools::Itertools;
use neptune_privacy::api::export::Claim;
use neptune_privacy::application::json_rpc::core::api::rpc::RpcApi;
use neptune_privacy::application::json_rpc::core::model::wallet::transaction::{
    RpcTransaction, RpcTransactionProof,
};
use neptune_privacy::protocol::consensus::transaction::transaction_kernel::TransactionKernelField;
use neptune_privacy::protocol::consensus::transaction::validity::collect_type_scripts::CollectTypeScripts;
use neptune_privacy::protocol::consensus::transaction::validity::kernel_to_outputs::KernelToOutputs;
use neptune_privacy::protocol::proof_abstractions::tasm::program::ConsensusProgram;
use neptune_privacy::{
    api::export::{
        Digest, KeyType, NativeCurrencyAmount, Network, ReceivingAddress, Timestamp, Tip5,
        TransactionDetails,
    },
    prelude::triton_vm::{self, stark::Stark, vm::PublicInput},
    protocol::{
        consensus::transaction::{
            primitive_witness::PrimitiveWitness,
            validity::{
                collect_lock_scripts::CollectLockScripts, proof_collection::ProofCollection,
                removal_records_integrity::RemovalRecordsIntegrity,
            },
        },
        proof_abstractions::{SecretWitness, mast_hash::MastHash},
    },
    state::wallet::{transaction_output::TxOutput, unlocked_utxo::UnlockedUtxo},
};

use tracing::info;
use xnt_rpc_client::http::HttpClient;

use crate::wallet::cache::{keys::KeysCache, utxos::UtxosCache};

#[derive(Clone)]
pub struct TransactionBuilder {
    client: HttpClient,
    pub keys: KeysCache,
    pub utxos: UtxosCache,
}

impl TransactionBuilder {
    pub fn new(client: HttpClient, keys: KeysCache, utxos: UtxosCache) -> Self {
        TransactionBuilder {
            client: client.clone(),
            keys,
            utxos,
        }
    }

    pub async fn send(
        &self,
        recipient: ReceivingAddress,
        amount: NativeCurrencyAmount,
        fee: NativeCurrencyAmount,
    ) {
        let mut utxos = self.utxos.write().await;
        utxos.sync_proofs().await;

        // Select UTXOs and prepare them for use.
        let (locked_utxos, excess_amount) = utxos.select_utxos(amount + fee);
        let mut inputs = Vec::new();

        for utxo in locked_utxos {
            let keys = self.keys.read().await;
            let unlocking_key = keys
                .find_spending_key_for_utxo(&utxo.utxo)
                .expect("to find key");
            inputs.push(UnlockedUtxo::unlock(
                utxo.utxo.clone(),
                unlocking_key.lock_script_and_witness(),
                utxo.membership_proof.clone(),
            ));
        }

        // Prepare output UTXOs (including the change output).
        let outputs = vec![
            TxOutput::onchain_native_currency(
                amount,
                Digest::default(), // TODO: Proper generation (as this might leak privacy).
                recipient,
                false,
            ),
            TxOutput::onchain_native_currency_as_change(
                excess_amount,
                Digest::default(), // TODO: Proper generation (as this might leak privacy).
                self.keys
                    .read()
                    .await
                    .current_key(KeyType::Symmetric)
                    .to_address(), // TODO: increment symmetric index?
            ),
        ];

        // Prepare the PrimitiveWitness and then create a ProofCollection for announcing.
        info!(
            "Preparing transaction with {} inputs, {} outputs...",
            inputs.len(),
            outputs.len()
        );
        let transaction = TransactionDetails::new_without_coinbase(
            inputs,
            outputs,
            fee,
            Timestamp::now(),
            utxos.msa.clone(),
            Network::Main,
        );
        let primitive_witness = transaction.primitive_witness();
        let transaction_kernel = primitive_witness.kernel.clone();
        let proof_collection = tokio::task::spawn_blocking(move || Self::prove(&primitive_witness))
            .await
            .expect("proving task panicked");

        let tx = RpcTransaction {
            proof: RpcTransactionProof::ProofCollection(Box::new(proof_collection.into())),
            kernel: (&transaction_kernel).into(),
        };
        self.client.submit_transaction(tx).await.unwrap();
        info!("Succesfully submitted transaction!");
    }

    // TODO: Proper error handling without crashing...
    fn prove(primitive_witness: &PrimitiveWitness) -> ProofCollection {
        let (
            removal_records_integrity_witness,
            collect_lock_scripts_witness,
            kernel_to_outputs_witness,
            collect_type_scripts_witness,
        ) = ProofCollection::extract_specific_witnesses(&primitive_witness);

        let txk_mast_hash = primitive_witness.kernel.mast_hash();
        let txk_mast_hash_as_input = PublicInput::new(txk_mast_hash.reversed().values().to_vec());
        let salted_inputs_hash = Tip5::hash(&primitive_witness.input_utxos);
        let salted_outputs_hash = Tip5::hash(&primitive_witness.output_utxos);

        info!("Starting proving of {}...", txk_mast_hash.to_hex());
        info!("Proving RemovalRecordsIntegrity (1/6)...");
        let removal_records_integrity = {
            let proof = triton_vm::prove(
                Stark::default(),
                &removal_records_integrity_witness.claim(),
                RemovalRecordsIntegrity.program(),
                removal_records_integrity_witness.nondeterminism(),
            )
            .expect("RemovalRecordsIntegrity proving failed");
            proof.into()
        };

        info!("Proving CollectLockScripts (2/6)...");
        let collect_lock_scripts = {
            let proof = triton_vm::prove(
                Stark::default(),
                &collect_lock_scripts_witness.claim(),
                CollectLockScripts.program(),
                collect_lock_scripts_witness.nondeterminism(),
            )
            .expect("CollectLockScripts proving failed");
            proof.into()
        };

        info!("Proving KernelToOutputs (3/6)...");
        let kernel_to_outputs = {
            let proof = triton_vm::prove(
                Stark::default(),
                &kernel_to_outputs_witness.claim(),
                KernelToOutputs.program(),
                kernel_to_outputs_witness.nondeterminism(),
            )
            .expect("KernelToOutputs proving failed");
            proof.into()
        };

        info!("Proving CollectTypeScripts (4/6)...");
        let collect_type_scripts = {
            let proof = triton_vm::prove(
                Stark::default(),
                &collect_type_scripts_witness.claim(),
                CollectTypeScripts.program(),
                collect_type_scripts_witness.nondeterminism(),
            )
            .expect("CollectTypeScripts proving failed");
            proof.into()
        };

        info!("Proving lock scripts (5/6)...");
        let mut lock_scripts_halt = vec![];
        for lsaw in &primitive_witness.lock_scripts_and_witnesses {
            let claim = Claim::new(lsaw.program.hash())
                .with_input(txk_mast_hash_as_input.individual_tokens.clone());
            let proof = triton_vm::prove(
                Stark::default(),
                &claim,
                lsaw.program.clone(),
                lsaw.nondeterminism(),
            )
            .expect("LockScript proving failed");
            lock_scripts_halt.push(proof.into());
        }

        info!("Proving type scripts (6/6)...");
        let mut type_scripts_halt = vec![];
        for (i, tsaw) in primitive_witness
            .type_scripts_and_witnesses
            .iter()
            .enumerate()
        {
            info!(
                "Proving type script number {i}: {}",
                tsaw.program.hash().to_hex()
            );
            let input = [txk_mast_hash, salted_inputs_hash, salted_outputs_hash]
                .into_iter()
                .flat_map(|d| d.reversed().values())
                .collect_vec();
            let claim = Claim::new(tsaw.program.hash()).with_input(input);
            let proof = triton_vm::prove(
                Stark::default(),
                &claim,
                tsaw.program.clone(),
                tsaw.nondeterminism(),
            )
            .expect("Type script proving failed");
            type_scripts_halt.push(proof.into());
        }

        let lock_script_hashes = primitive_witness
            .lock_scripts_and_witnesses
            .iter()
            .map(|lsaw| lsaw.program.hash())
            .collect_vec();
        let type_script_hashes = primitive_witness
            .type_scripts_and_witnesses
            .iter()
            .map(|tsaw| tsaw.program.hash())
            .collect_vec();
        let merge_bit_mast_path = primitive_witness
            .kernel
            .mast_path(TransactionKernelField::MergeBit);

        ProofCollection {
            removal_records_integrity,
            collect_lock_scripts,
            lock_scripts_halt,
            kernel_to_outputs,
            collect_type_scripts,
            type_scripts_halt,
            lock_script_hashes,
            type_script_hashes,
            kernel_mast_hash: txk_mast_hash,
            salted_inputs_hash,
            salted_outputs_hash,
            merge_bit_mast_path,
        }
    }
}
