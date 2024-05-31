use std::str::FromStr;

use candid::{Principal, CandidType};
use ic_cdk::api::management_canister::http_request::{TransformArgs, HttpResponse};
use ic_cdk_macros::{query, update};
use ic_web3::{types::{Address, SignedTransaction, U64}, ic::{get_public_key as get_public_key_internal, pubkey_to_address as pubkey_to_address_internal, KeyInfo }, transports::ICHttp, Web3, contract::{tokens::Tokenize, Contract, Options}};

const KEY_NAME: &str = "dfx_test_key";

// For Polygon Mainnet
// const BASE_URL: &'static str = "polygon-mainnet.g.alchemy.com";
// const PATH: &'static str = "/v2/sLp6VfuskMEwx8Wx0DvaRkI8qCoVYF8f";
// const CHAIN_ID: u64 = 1;
// const DAI_ADDR: &'static str = "";

// For Polygon Testnet (Mumbai)
const BASE_URL: &'static str = "polygon-mumbai.g.alchemy.com";
const PATH: &'static str = "/v2/6GLIzI5pL0n4bp4c3jESZTRfXxE5XJ_Z";
const CHAIN_ID: u64 = 80001;
const DAI_ADDR: &'static str = "001B3B4d0F3714Ca98ba10F6042DaEbF0B1B7b6F";

const ERC20_ABI: &[u8] = include_bytes!("../../abi/erc20.json");
const MINTABLE_ERC20_ABI: &[u8] = include_bytes!("../../abi/mintable_erc20.json");

#[derive(CandidType)]
struct AccountInfo {
    pub address: String,
    pub pub_key: String
}
#[derive(CandidType)]
struct CandidSignedTransaction {
    pub message_hash: String,
    pub v: u64,
    pub r: String,
    pub s: String,
    pub raw_transaction: String,
    pub transaction_hash: String,
}

fn get_rpc_endpoint() -> String {
    format!("https://{}{}", BASE_URL, PATH)
}

fn default_derivation_key() -> Vec<u8> {
    ic_cdk::id().as_slice().to_vec()
}

#[update]
async fn account_info() -> Result<AccountInfo, String> {
    let pub_key = get_public_key(None, vec![ic_cdk::id().as_slice().to_vec()], KEY_NAME.to_string()).await;
    if let Err(msg) = pub_key { return Err(msg) };
    let pub_key = pub_key.unwrap();

    let addr = pubkey_to_address(&pub_key);
    if let Err(msg) = addr { return Err(msg) };
    let addr = addr.unwrap();

    return Ok(AccountInfo {
        address: format!("0x{}", hex::encode(addr)),
        pub_key: format!("0x{}", hex::encode(pub_key)),
    })
}

#[update]
fn rpc_endpoint() -> String {
    get_rpc_endpoint()
}

#[update]
async fn pub_key() -> String {
    match get_public_key(None, vec![default_derivation_key()], KEY_NAME.to_string()).await {
        Ok(v) => format!("0x{}", hex::encode(v)),
        Err(msg) => msg
    }
}

#[update]
async fn eth_addr() -> String {
    let res = get_eth_addr(None, None, KEY_NAME.to_string()).await;
    match res {
        Ok(v) => format!("0x{}", hex::encode(v)),
        Err(msg) => msg
    }
}

#[query(name = "transform")]
fn transform(response: TransformArgs) -> HttpResponse {
    response.response
}

#[update]
async fn balance_of(contract_addr: String, holder_addr: String) -> Result<u128, String> {
    let w3 = generate_web3_client()
        .map_err(|e| format!("generate_web3_client failed: {}", e))?;
    let contract = generate_contract_client(w3, &contract_addr, ERC20_ABI)?;

    let addr = Address::from_str(&holder_addr).unwrap();
    contract
        .query("balanceOf", addr, None, Options::default(), None)
        .await
        .map_err(|e| format!("query contract error: {}", e))
}

#[update]
async fn balance_of_dai(holder_addr: String) -> Result<u128, String> {
    balance_of(DAI_ADDR.to_string(), holder_addr).await
}

#[update]
async fn send_erc20_signed_tx(
    token_addr: String, // TODO: enable 0x
    to_addr: String, // TODO: enable 0x
    value: u64
) -> Result<CandidSignedTransaction, String> {
    let w3 = generate_web3_client()
        .map_err(|e| format!("generate_web3_client failed: {}", e))?;
    match send_erc20_signed_tx_internal(
        w3.clone(),
        token_addr,
        to_addr,
        value,
    ).await {
        Ok(signed_tx) => 
            Ok(CandidSignedTransaction {
                message_hash: format!("0x{}", hex::encode(signed_tx.message_hash)),
                v: signed_tx.v,
                r: format!("0x{}", hex::encode(signed_tx.r)),
                s: format!("0x{}", hex::encode(signed_tx.s)),
                raw_transaction: format!("0x{}", hex::encode(signed_tx.raw_transaction.0)),
                transaction_hash: format!("0x{}", hex::encode(signed_tx.transaction_hash)),
            }),
        Err(msg) => Err(msg)
    }
}
#[update]
async fn send_erc20(
    token_addr: String, // TODO: enable 0x
    to_addr: String, // TODO: enable 0x
    value: u64
) -> Result<String, String> {
    let w3 = generate_web3_client()
        .map_err(|e| format!("generate_web3_client failed: {}", e))?;
    let signed_tx = send_erc20_signed_tx_internal(
        w3.clone(),
        token_addr,
        to_addr,
        value,
    ).await?;
    match w3.eth().send_raw_transaction(signed_tx.raw_transaction).await {
        Ok(v) => Ok(format!("0x{}", hex::encode(v))),
        Err(msg) => Err(format!("send_raw_transaction failed: {}", msg))
    }
}
async fn send_erc20_signed_tx_internal(
    w3: Web3<ICHttp>,
    token_addr: String,
    to_addr: String,
    value: u64
) -> Result<SignedTransaction, String> {
    let to_addr = Address::from_str(&to_addr).unwrap();
    sign(
        w3,
        &token_addr,
        ERC20_ABI,
        &"transfer",
        (to_addr, value,)
    ).await
}

#[update]
async fn mint_erc20_signed_tx(
    token_addr: String, // TODO: enable 0x
    value: u64
) -> Result<CandidSignedTransaction, String> {
    let w3 = generate_web3_client()
        .map_err(|e| format!("generate_web3_client failed: {}", e))?;
    match mint_erc20_signed_tx_internal(
        w3.clone(),
        token_addr,
        value,
    ).await {
        Ok(signed_tx) => 
            Ok(CandidSignedTransaction {
                message_hash: format!("0x{}", hex::encode(signed_tx.message_hash)),
                v: signed_tx.v,
                r: format!("0x{}", hex::encode(signed_tx.r)),
                s: format!("0x{}", hex::encode(signed_tx.s)),
                raw_transaction: format!("0x{}", hex::encode(signed_tx.raw_transaction.0)),
                transaction_hash: format!("0x{}", hex::encode(signed_tx.transaction_hash)),
            }),
        Err(msg) => Err(msg)
    }
}
#[update]
async fn mint_erc20(
    token_addr: String, // TODO: enable 0x
    value: u64
) -> Result<String, String> {
    let w3 = generate_web3_client()
        .map_err(|e| format!("generate_web3_client failed: {}", e))?;
    let signed_tx = mint_erc20_signed_tx_internal(
        w3.clone(),
        token_addr,
        value,
    ).await?;
    match w3.eth().send_raw_transaction(signed_tx.raw_transaction).await {
        Ok(v) => Ok(format!("0x{}", hex::encode(v))),
        Err(msg) => Err(format!("send_raw_transaction failed: {}", msg))
    }
}
#[update]
async fn mint_dai_signed_tx(
    value: u64
) -> Result<CandidSignedTransaction, String> {
    mint_erc20_signed_tx(DAI_ADDR.to_string(), value).await
}
#[update]
async fn mint_dai(
    value: u64
) -> Result<String, String> {
    mint_erc20(DAI_ADDR.to_string(), value).await
}

async fn mint_erc20_signed_tx_internal(
    w3: Web3<ICHttp>,
    token_addr: String,
    value: u64
) -> Result<SignedTransaction, String> {
    sign(
        w3,
        &token_addr,
        MINTABLE_ERC20_ABI,
        &"mint",
        (value,)
    ).await
}

async fn get_eth_addr(
    canister_id: Option<Principal>,
    derivation_path: Option<Vec<Vec<u8>>>,
    name: String
) -> Result<Address, String> {
    let path = if let Some(v) = derivation_path { v } else { vec![default_derivation_key()] };
    match get_public_key(canister_id, path, name).await {
        Ok(pubkey) => { return pubkey_to_address_internal(&pubkey); },
        Err(e) => { return Err(e); },
    };
}

async fn get_public_key(
    canister_id: Option<Principal>,
    derivation_path: Vec<Vec<u8>>,
    name: String
) -> Result<Vec<u8>, String> {
    get_public_key_internal(canister_id, derivation_path, name).await
}

fn pubkey_to_address(pubkey: &[u8]) -> Result<Address, String> {
    pubkey_to_address_internal(&pubkey)
}

async fn sign(
    w3: Web3<ICHttp>,
    contract_addr: &str,
    abi: &[u8],
    func: &str,
    params: impl Tokenize,
) -> Result<SignedTransaction, String> {
    let contract = generate_contract_client(w3.clone(), contract_addr, abi)
        .map_err(|e| format!("generate_contract_client failed: {}", e))?;
    let canister_addr = get_eth_addr(None, None, KEY_NAME.to_string()).await
        .map_err(|e| format!("get_eth_addr failed: {}", e))?;
    
    let tx_count = w3.eth()
        .transaction_count(canister_addr, None)
        .await
        .map_err(|e| format!("get tx count error: {}", e))?;
    let gas_price = w3.eth()
        .gas_price()
        .await
        .map_err(|e| format!("get gas_price error: {}", e))?;
    let options = Options::with(|op| { 
        op.nonce = Some(tx_count);
        op.gas_price = Some(gas_price);
        op.transaction_type = Some(U64::from(2)) // EIP1559_TX_ID
    });
    
    match contract.sign(
        func,
        params,
        options,
        hex::encode(canister_addr),
        KeyInfo { derivation_path: vec![default_derivation_key()], key_name: KEY_NAME.to_string() },
        CHAIN_ID // TODO: switch chain
    ).await {
        Ok(v) => Ok(v),
        Err(msg) => Err(format!("sign failed: {}", msg))
    }
}

fn generate_contract_client(w3: Web3<ICHttp>, contract_addr: &str, abi: &[u8]) -> Result<Contract<ICHttp>, String> {
    let contract_address = Address::from_str(contract_addr).unwrap();
    Contract::from_json(
        w3.eth(),
        contract_address,
        abi
    ).map_err(|e| format!("init contract failed: {}", e))
}

fn generate_web3_client() -> Result<Web3<ICHttp>, String> {
    match ICHttp::new(
        get_rpc_endpoint().as_str(),
        None, // TODO: switch local/prod
        None // TODO: switch local/prod
    ) {
        Ok(v) => Ok(Web3::new(v)),
        Err(e) => Err(e.to_string())
    }
}