use anyhow::Result;
use neptune_cash::api::export::Announcement;
use neptune_cash::api::export::BlockHeight;
use neptune_cash::api::export::Tip5;
use neptune_cash::application::json_rpc::core::api::rpc::RpcApi;
use neptune_cash::prelude::triton_vm::prelude::BFieldElement;
use neptune_cash::protocol::consensus::block::block_selector::BlockSelector;
use neptune_rpc_client::http::HttpClient;

use crate::wallet::state::Wallet;

pub mod wallet;

#[tokio::main]
async fn main() -> Result<()> {
    let mnemonic = "belt expose monkey vapor tiny noble crater guilt have submit before fat rude tide shoulder practice hybrid record".to_string();
    let wallet = Wallet::new(mnemonic);

    let client = HttpClient::new("http://127.0.0.1:9797");

    let tip_height = client.height().await.unwrap().height;
    let tip_height: BlockHeight = tip_height.into();

    let mut current_height = BlockHeight::new(BFieldElement::new(17500));

    while current_height <= tip_height {
        let transaction_kernel = client
            .get_block_transaction_kernel(BlockSelector::Height(current_height))
            .await
            .unwrap()
            .kernel
            .unwrap();
        let announcements: Vec<Announcement> = transaction_kernel
            .announcements
            .clone()
            .into_iter()
            .map(Into::into)
            .collect();

        let utxos = wallet.scan(announcements);

        for (utxo, mut mock_proof) in utxos {
            let commitment = utxo.addition_record().canonical_commitment;
            let index = transaction_kernel
                .outputs
                .iter()
                .position(|r| r.0 == commitment)
                .unwrap(); // This might panic bcs of a malicious announcement.
            println!(
                "Found {} on block {current_height} on index {}",
                commitment.to_hex(),
                index
            );

            let block_body = client
                .get_block_body(BlockSelector::Height(current_height))
                .await
                .unwrap()
                .body
                .unwrap();

            let expected_aocl_index =
                block_body.mutator_set_accumulator.aocl.leaf_count - index as u64 + 1 as u64;
            let digest = client
                .get_utxo_digest(expected_aocl_index)
                .await
                .unwrap()
                .digest
                .unwrap();
            println!("matches: {}", commitment == digest);

            mock_proof.aocl_leaf_index = expected_aocl_index;
            let item = Tip5::hash(&utxo.utxo);
            let index_set = mock_proof.compute_indices(item);

            let proof = client
                .restore_membership_proof(vec![index_set.into()])
                .await
                .unwrap()
                .snapshot;

            let actual_proof = proof.membership_proofs[0]
                .clone()
                .extract_ms_membership_proof(
                    expected_aocl_index,
                    mock_proof.sender_randomness,
                    mock_proof.receiver_preimage,
                )
                .unwrap();
            println!("recovered the msmembership proof!!!")
        }

        current_height = current_height.next();
    }
    Ok(())
}
