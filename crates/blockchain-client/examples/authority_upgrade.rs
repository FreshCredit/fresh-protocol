//! Authority-set upgrade helper: adds Charlie to Aura + GRANDPA on the live
//! chain via sudo storage writes, then forces a proper GRANDPA set change.
//!
//! Storage layouts (verified against the live runtime):
//!   Aura:Authorities    = BoundedVec<sr25519::app_sr25519::Public>
//!   Grandpa:Authorities = WeakBoundedVec<(grandpa::app::Public, u64)>
//!
//! Usage:
//!   cargo run -p blockchain-client --example authority_upgrade --            (dry run)
//!   cargo run -p blockchain-client --example authority_upgrade -- --execute  (writes)
//!
//! Requires RPC_URL (default ws://localhost:19944 — port-forward to the pod).

use anyhow::Result;
use blockchain_client::freshcredit_runtime;
use sp_core_hashing::twox_128;
use subxt::{OnlineClient, SubstrateConfig};
use subxt_signer::sr25519::dev;

fn storage_key(pallet: &str, item: &str) -> Vec<u8> {
    let mut k = twox_128(pallet.as_bytes()).to_vec();
    k.extend(twox_128(item.as_bytes()));
    k
}

#[tokio::main]
async fn main() -> Result<()> {
    let url = std::env::var("RPC_URL").unwrap_or_else(|_| "ws://localhost:19944".into());
    let execute = std::env::args().any(|a| a == "--execute");

    let api = OnlineClient::<SubstrateConfig>::from_url(&url).await?;
    println!("connected: {url}");

    // Authority accounts straight from the well-known dev keypairs.
    let alice = dev::alice().public_key().0;
    let bob = dev::bob().public_key().0;
    let charlie = dev::charlie().public_key().0;
    println!("alice  = 0x{}", hex::encode(alice));
    println!("bob    = 0x{}", hex::encode(bob));
    println!("charlie= 0x{}", hex::encode(charlie));

    let aura_key = storage_key("Aura", "Authorities");
    let grandpa_key = storage_key("Grandpa", "Authorities");

    let at = api.at_current_block().await?;
    let aura_addr = freshcredit_runtime::storage().aura().authorities();
    let cur_aura = at.storage().fetch(&aura_addr, ()).await?;
    println!("current Aura authorities: {cur_aura:?}");
    let grandpa_addr = freshcredit_runtime::storage().grandpa().authorities();
    let cur_grandpa = at.storage().fetch(&grandpa_addr, ()).await?;
    println!("current Grandpa authorities: {cur_grandpa:?}");

    if !execute {
        println!("DRY RUN — would write Aura+GRANDPA authorities = [Alice, Bob, Charlie] via sudo, then grandpa.note_stalled.");
        return Ok(());
    }

    let signer = dev::alice();

    // Aura value: BoundedVec<sr25519 Public> = compact(3) + 3×32B
    let mut aura_value = vec![3u8 << 2];
    aura_value.extend_from_slice(&alice);
    aura_value.extend_from_slice(&bob);
    aura_value.extend_from_slice(&charlie);

    // GRANDPA value: WeakBoundedVec<(Public, u64)> = compact(3) + 3×(32B + 8B weight=1)
    let mut grandpa_value = vec![3u8 << 2];
    for id in [alice, bob, charlie] {
        grandpa_value.extend_from_slice(&id);
        grandpa_value.extend_from_slice(&1u64.to_le_bytes());
    }

    let signer = dev::alice();
    for (label, key, value) in [
        ("Aura", aura_key.clone(), aura_value),
        ("Grandpa", grandpa_key.clone(), grandpa_value),
    ] {
        let inner = freshcredit_runtime::runtime_types::frame_system::pallet::Call::set_storage {
            items: vec![(key, value)],
        };
        let call = freshcredit_runtime::tx().sudo().sudo_unchecked_weight(
            freshcredit_runtime::runtime_types::freshcredit_runtime::RuntimeCall::System(inner),
            freshcredit_runtime::runtime_types::sp_weights::weight_v2::Weight {
                ref_time: 1_000_000_000,
                proof_size: 64 * 1024,
            },
        );
        let progress = api
            .tx()
            .await?
            .sign_and_submit_then_watch_default(&call, &signer)
            .await?;
        let events = progress.wait_for_finalized_success().await?;
        println!("{label} set_storage finalized: {}", events.extrinsic_hash());
    }

    // Force a proper GRANDPA set change so voters adopt the new set cleanly.
    let fin = api.at_current_block().await?.block_number();
    let note = freshcredit_runtime::tx()
        .grandpa()
        .note_stalled(2000u32, fin as u32);
    let progress = api
        .tx()
        .await?
        .sign_and_submit_then_watch_default(&note, &signer)
        .await?;
    let events = progress.wait_for_finalized_success().await?;
    println!(
        "note_stalled finalized: {} (at block {fin})",
        events.extrinsic_hash()
    );

    let at = api.at_current_block().await?;
    let now = at
        .storage()
        .fetch(&freshcredit_runtime::storage().aura().authorities(), ())
        .await?;
    println!("Aura authorities now: {now:?}");

    Ok(())
}
