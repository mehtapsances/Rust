use std::str::FromStr;

use candid::Principal;
use ic_cdk_macros::{query, update};
use ic_web3::{types::{Address, TransactionParameters, U256, SignedTransaction}, ic::{get_public_key, pubkey_to_address, KeyInfo}, transports::ICHttp, Web3};
use secp256k1::{Message, Secp256k1, PublicKey, ecdsa::Signature};
use sha3::{Keccak256, Digest};

const KEY_NAME: &str = "dfx_test_key";

const BASE_URL: &'static str = "polygon-mainnet.g.alchemy.com";
const PATH: &'static str = "/v2/sLp6VfuskMEwx8Wx0DvaRkI8qCoVYF8f";
const CHAIN_ID: u64 = 1;

fn get_rpc_endpoint() -> String {
    format!("https://{}{}", BASE_URL, PATH)
}

#[query]
fn greet(name: String) -> String {
    format!("Hello, {}!", name)
}

#[update]
async fn eth_addr() -> String {
    let res = get_eth_addr(None, None, KEY_NAME.to_string()).await;
    match res {
        Ok(addr) => hex::encode(addr),
        Err(msg) => msg
    }
}

#[update]
async fn send_eth(to: String, value: u64) -> Result<String, String> {
    let res = send_eth_siged_tx(to, value).await;
    if let Err(msg) = res { return Err(msg) };
    let signed_tx = res.unwrap();
    ic_cdk::println!("message_hash: {:?}", signed_tx.message_hash.as_bytes());
    ic_cdk::println!("v: {:?}", signed_tx.v);
    ic_cdk::println!("r: {:?}", signed_tx.r);
    ic_cdk::println!("s: {:?}", signed_tx.s);
    ic_cdk::println!("raw_transaction: {:?}", signed_tx.raw_transaction);
    ic_cdk::println!("transaction_hash: {:?}", signed_tx.transaction_hash);
    Ok("OK".to_string())
}

#[update]
async fn verify_send_eth(to: String, value: u64) -> Result<bool, String> {
    let res = send_eth_siged_tx(to, value).await;
    if let Err(msg) = res { return Err(msg) };
    let signed_tx = res.unwrap();

    let public_key = get_public_key(None, vec![ic_cdk::id().as_slice().to_vec()], KEY_NAME.to_string()).await;
    if let Err(msg) = public_key { return Err(msg) };
    Ok(verify_tx_signature(
        &signed_tx.transaction_hash.as_bytes(),
        signed_tx.v,
        signed_tx.r.as_bytes(),
        signed_tx.s.as_bytes(),
        &public_key.unwrap()
    ))
}

async fn get_eth_addr(
    canister_id: Option<Principal>,
    derivation_path: Option<Vec<Vec<u8>>>,
    name: String
) -> Result<Address, String> {
    let path = if let Some(v) = derivation_path { v } else { vec![ic_cdk::id().as_slice().to_vec()] };
    match get_public_key(canister_id, path, name).await {
        Ok(pubkey) => { return pubkey_to_address(&pubkey); },
        Err(e) => { return Err(e); },
    };
}

pub fn verify_tx_signature(tx_hash: &[u8], v: u64, r: &[u8], s: &[u8], public_key: &[u8]) -> bool {
    let message = &tx_hash[..];
    let mut keccak256 = Keccak256::new();
    keccak256.update(&message);
    let message_hash = Message::from_slice(keccak256.finalize().as_slice()).unwrap();

    let mut signature_bytes = [0u8; 65];
    signature_bytes[0..32].copy_from_slice(&r);
    signature_bytes[32..64].copy_from_slice(&s);
    signature_bytes[64] = v as u8;

    let secp = Secp256k1::new();
    let signature = Signature::from_compact(&signature_bytes).unwrap();
    let public_key = PublicKey::from_slice(&public_key).unwrap();

    secp.verify_ecdsa(&message_hash, &signature, &public_key).is_ok()
}

pub async fn send_eth_siged_tx(to: String, value: u64) -> Result<SignedTransaction, String> {
    let derivation_path = vec![ic_cdk::id().as_slice().to_vec()];
    let key_info = KeyInfo { derivation_path: derivation_path, key_name: KEY_NAME.to_string() };

    let from_addr = get_eth_addr(None, None, KEY_NAME.to_string())
        .await
        .map_err(|e| format!("get canister eth addr failed: {}", e));
    if let Err(msg) = from_addr {
        return Err(msg)
    }
    let w3 = match ICHttp::new(&get_rpc_endpoint(), None, None) {
        Ok(v) => { Web3::new(v) },
        Err(e) => { return Err(e.to_string()) },
    };

    let to = Address::from_str(&to).unwrap();
    let tx = TransactionParameters {
        to: Some(to),
        nonce: Some(U256::from(0)), // remember to fetch nonce first
        value: U256::from(value),
        gas_price: Some(U256::exp10(10)), // 10 gwei
        gas: U256::from(21000),
        ..Default::default()
    };
    let signed_tx = w3.accounts()
        .sign_transaction(tx, hex::encode(from_addr.unwrap()), key_info, CHAIN_ID)
        .await
        .map_err(|e| format!("sign tx error: {}", e));
    signed_tx
}
