use once_cell::sync::Lazy;
use tokio::runtime::Runtime;

use crate::wallet;

static RT: Lazy<Runtime> = Lazy::new(|| Runtime::new().expect("tokio runtime"));

#[derive(Debug, thiserror::Error)]
pub enum DashError {
    #[error("{message}")]
    Internal { message: String },
}

fn map<T>(value: wallet::WalletResult<T>) -> Result<T, DashError> {
    value.map_err(|message| DashError::Internal { message })
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    RT.block_on(future)
}

pub fn set_document_directory(path: String) -> Result<(), DashError> {
    map(wallet::set_document_directory(path))
}

pub fn initialize(
    mnemonic_seed: String,
    account: u32,
    alias: String,
    network_name: String,
    default_host: String,
    default_port: u32,
) -> Result<(), DashError> {
    map(block_on(wallet::initialize(
        mnemonic_seed,
        account,
        alias,
        network_name,
        default_host,
        default_port,
    )))
}

pub fn stop(alias: String) -> Result<String, DashError> {
    map(block_on(wallet::stop(alias)))
}

pub fn start_sync(alias: String) -> Result<(), DashError> {
    map(block_on(wallet::start_sync(alias)))
}

pub fn stop_sync(alias: String) -> Result<(), DashError> {
    map(block_on(wallet::stop_sync(alias)))
}

pub fn derive_shielded_address(alias: String) -> Result<wallet::Addresses, DashError> {
    map(block_on(wallet::derive_shielded_address(alias)))
}

pub fn core_receive_address(alias: String, account: u32) -> Result<String, DashError> {
    map(block_on(wallet::core_receive_address(alias, account)))
}

pub fn start_core_sync(alias: String, from_height: u32) -> Result<(), DashError> {
    map(block_on(wallet::start_core_sync(alias, from_height)))
}

pub fn core_balance(alias: String) -> Result<wallet::CoreBalance, DashError> {
    map(block_on(wallet::core_balance(alias)))
}

pub fn shield_from_asset_lock(
    alias: String,
    amount_duffs: String,
    account_index: u32,
    mnemonic_seed: String,
) -> Result<String, DashError> {
    map(block_on(wallet::shield_from_asset_lock(
        alias,
        amount_duffs,
        account_index,
        mnemonic_seed,
    )))
}

pub fn platform_receive_address(alias: String, account: u32) -> Result<String, DashError> {
    map(block_on(wallet::platform_receive_address(alias, account)))
}

pub fn tracked_asset_locks(alias: String) -> Result<Vec<wallet::AssetLock>, DashError> {
    map(block_on(wallet::tracked_asset_locks(alias)))
}

pub fn resume_shield_from_asset_lock(
    alias: String,
    txid: String,
    vout: u32,
    mnemonic_seed: String,
) -> Result<String, DashError> {
    map(block_on(wallet::resume_shield_from_asset_lock(
        alias,
        txid,
        vout,
        mnemonic_seed,
    )))
}

pub fn unshield(
    alias: String,
    to_address: String,
    amount_credits: String,
    mnemonic_seed: String,
) -> Result<String, DashError> {
    map(block_on(wallet::unshield(
        alias,
        to_address,
        amount_credits,
        mnemonic_seed,
    )))
}

pub fn shielded_withdraw(
    alias: String,
    to_core_address: String,
    amount_credits: String,
    core_fee_per_byte: u32,
    mnemonic_seed: String,
) -> Result<String, DashError> {
    map(block_on(wallet::shielded_withdraw(
        alias,
        to_core_address,
        amount_credits,
        core_fee_per_byte,
        mnemonic_seed,
    )))
}

pub fn is_valid_address(address: String, network: String) -> bool {
    wallet::is_valid_address(address, network)
}

pub fn derive_viewing_key(mnemonic_seed: String, network: String) -> Result<String, DashError> {
    map(wallet::derive_viewing_key(mnemonic_seed, network))
}

pub fn warm_up_prover() -> Result<(), DashError> {
    map(block_on(wallet::warm_up_prover()))
}

pub fn is_prover_ready() -> bool {
    wallet::is_prover_ready()
}

pub fn poll(alias: String) -> Result<wallet::Poll, DashError> {
    map(block_on(wallet::poll(alias)))
}

pub fn propose_transfer(
    alias: String,
    amount_credits: String,
    to_address: String,
    memo: Option<String>,
) -> Result<String, DashError> {
    map(block_on(wallet::propose_transfer(
        alias,
        amount_credits,
        to_address,
        memo,
    )))
}

pub fn create_transfer(
    alias: String,
    proposal_id: String,
    mnemonic_seed: String,
) -> Result<String, DashError> {
    map(block_on(wallet::create_transfer(
        alias,
        proposal_id,
        mnemonic_seed,
    )))
}

pub fn derive_shielded_address_from_seed(
    mnemonic_seed: String,
    network: String,
    account: u32,
) -> Result<String, DashError> {
    let alias = format!("tools-addr-{network}-{account}");
    initialize(mnemonic_seed, account, alias.clone(), network, String::new(), 0)?;
    let addresses = derive_shielded_address(alias.clone())?;
    let _ = stop(alias);
    Ok(addresses.shielded_address)
}
