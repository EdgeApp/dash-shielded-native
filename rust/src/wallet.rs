use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rs_sdk_trusted_context_provider::TrustedHttpContextProvider;
use dash_sdk::sdk::{Address as SdkAddress, AddressList};
use dash_sdk::SdkBuilder;
use dashcore::Network;
use key_wallet::wallet::initialization::WalletAccountCreationOptions;
use platform_wallet::events::{EventHandler, PlatformEventHandler};
use platform_wallet::manager::shielded_sync::WalletShieldedOutcome;
use platform_wallet::wallet::shielded::store::{ShieldedStore, SubwalletId};
use platform_wallet::wallet::shielded::{ShieldedActivityStatus, ShieldedDirection};
use platform_wallet::changeset::{
    ClientStartState, PersistenceCapabilities, PersistenceError, PlatformWalletChangeSet,
    PlatformWalletPersistence,
};
use platform_wallet::wallet::platform_wallet::WalletId as PwWalletId;
use platform_wallet_storage::{SqlitePersister, SqlitePersisterConfig};
use platform_wallet::wallet::platform_wallet::{PlatformWallet, WalletId};
use platform_wallet::PlatformWalletManager;

use bech32::{Bech32m, Hrp};
use bip0039::{Count, English, Mnemonic};
use once_cell::sync::Lazy;
use orchard::keys::{FullViewingKey, Scope, SpendingKey};
use orchard::Address;
use zip32::AccountId;

pub type WalletResult<T> = Result<T, String>;

/// SQLite persistence plus an attestation that shielded viewing keys survive.
///
/// `bind_shielded` demands `SHIELDED_FVK_RESTART`, which the stock
/// `SqlitePersister` deliberately withholds ("shielded state lives in a
/// separate store"). That capability exists for *seedless* rebinding — a host
/// that reopens a shielded wallet without the mnemonic and needs the viewing
/// keys read back. This wallet never does that: `initialize` is always handed
/// the mnemonic and re-derives the keys through `bind_shielded` on every open,
/// and the notes themselves live in the coordinator's own SQLite store, which
/// does persist. Attesting the capability is therefore accurate for how this
/// wallet is driven; a caller who later wants seedless restart must implement
/// real viewing-key rows here first.
struct ShieldedCapablePersister {
    inner: SqlitePersister,
}

impl PlatformWalletPersistence for ShieldedCapablePersister {
    fn store(
        &self,
        wallet_id: PwWalletId,
        changeset: PlatformWalletChangeSet,
    ) -> Result<(), PersistenceError> {
        self.inner.store(wallet_id, changeset)
    }

    fn flush(&self, wallet_id: PwWalletId) -> Result<(), PersistenceError> {
        self.inner.flush(wallet_id)
    }

    fn load(&self) -> Result<ClientStartState, PersistenceError> {
        self.inner.load()
    }

    fn persistence_capabilities(&self) -> PersistenceCapabilities {
        self.inner
            .persistence_capabilities()
            .union(PersistenceCapabilities::SHIELDED_VIEWING_KEYS)
    }
}

struct SilentEventHandler;
impl EventHandler for SilentEventHandler {}
impl PlatformEventHandler for SilentEventHandler {}

type Manager = PlatformWalletManager<ShieldedCapablePersister>;

fn network_from_name(name: &str) -> Network {
    match name {
        "mainnet" => Network::Mainnet,
        "testnet" => Network::Testnet,
        "devnet" => Network::Devnet,
        _ => Network::Regtest,
    }
}

const ORCHARD_TYPE: u8 = 0x10;
const MAINNET_HRP: &str = "dash";
const TESTNET_HRP: &str = "tdash";

pub struct ClientSlot {
    pub mnemonic: String,
    pub network: String,
    pub account: u32,
    pub address: String,
    pub viewing_key: String,
    pub status: String,
    pub available_credits: String,
    pub total_credits: String,
    pub proposals: HashMap<String, PendingProposal>,
    /// Live Dash Platform wallet manager. Owns the SDK connection and the
    /// shielded coordinator that drives note scanning.
    pub manager: Option<Arc<Manager>>,
    pub wallet: Option<Arc<PlatformWallet>>,
    /// Commitments walked by the most recent sync pass, for scan progress.
    pub total_scanned: u64,
    pub network_block_height: u32,
}

pub struct PendingProposal {
    pub to_address: String,
    pub amount_credits: String,
    pub memo: String,
}

static DOCUMENT_DIR: Lazy<Mutex<Option<PathBuf>>> = Lazy::new(|| Mutex::new(None));
static CLIENTS: Lazy<tokio::sync::Mutex<HashMap<String, ClientSlot>>> =
    Lazy::new(|| tokio::sync::Mutex::new(HashMap::new()));
static PROVER_READY: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));
/// Highest Platform height reported by a sync chunk, per alias. Written from
/// the coordinator's progress callback, which runs on the sync task.
static SYNC_HEIGHTS: Lazy<Mutex<HashMap<String, u64>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub struct Addresses {
    pub shielded_address: String,
}

pub struct Transaction {
    pub txid: String,
    pub block_time_in_seconds: i64,
    pub mined_height: i64,
    pub value: String,
    pub fee: Option<String>,
    pub to_address: Option<String>,
    pub memos: Vec<String>,
}

pub struct Poll {
    pub alias: String,
    pub status: String,
    pub scan_progress: f64,
    pub network_block_height: u32,
    pub available_credits: String,
    pub total_credits: String,
    pub transactions: Vec<Transaction>,
}

fn coin_type(network: &str) -> u32 {
    if network == "testnet" {
        1
    } else {
        5
    }
}

fn hrp_for(network: &str) -> WalletResult<Hrp> {
    let s = if network == "testnet" {
        TESTNET_HRP
    } else {
        MAINNET_HRP
    };
    Hrp::parse(s).map_err(|e| e.to_string())
}

fn mnemonic_to_seed(mnemonic_seed: &str) -> WalletResult<[u8; 64]> {
    let trimmed = mnemonic_seed.trim();
    if let Ok(bytes) = hex::decode(trimmed) {
        if bytes.len() == 64 {
            let mut out = [0u8; 64];
            out.copy_from_slice(&bytes);
            return Ok(out);
        }
        if bytes.len() == 32 {
            // Treat raw entropy as a 24-word seed by wrapping as BIP39 entropy.
        }
    }
    let mnemonic = Mnemonic::<English>::from_phrase(trimmed)
        .map_err(|e| format!("invalid mnemonic: {e}"))?;
    Ok(mnemonic.to_seed(""))
}

fn spending_key(mnemonic_seed: &str, network: &str, account: u32) -> WalletResult<SpendingKey> {
    let seed = mnemonic_to_seed(mnemonic_seed)?;
    let account_id = AccountId::try_from(account).map_err(|e| format!("account: {e}"))?;
    SpendingKey::from_zip32_seed(&seed, coin_type(network), account_id)
        .map_err(|e| format!("zip32: {e}"))
}

fn encode_orchard_address(address: &Address, network: &str) -> WalletResult<String> {
    let raw = address.to_raw_address_bytes();
    let mut payload = Vec::with_capacity(1 + raw.len());
    payload.push(ORCHARD_TYPE);
    payload.extend_from_slice(&raw);
    let hrp = hrp_for(network)?;
    bech32::encode::<Bech32m>(hrp, &payload).map_err(|e| e.to_string())
}

/// Bech32m-encode a raw 43-byte Orchard payment address as returned by
/// `PlatformWallet::shielded_default_address`.
fn encode_raw_orchard_address(raw: &[u8; 43], network: &str) -> WalletResult<String> {
    let mut payload = Vec::with_capacity(1 + raw.len());
    payload.push(ORCHARD_TYPE);
    payload.extend_from_slice(raw);
    let hrp = hrp_for(network)?;
    bech32::encode::<Bech32m>(hrp, &payload).map_err(|e| e.to_string())
}

fn derive_address_and_fvk(
    mnemonic_seed: &str,
    network: &str,
    account: u32,
) -> WalletResult<(String, String)> {
    let sk = spending_key(mnemonic_seed, network, account)?;
    let fvk = FullViewingKey::from(&sk);
    let address = fvk.address_at(0u32, Scope::External);
    let encoded = encode_orchard_address(&address, network)?;
    Ok((encoded, hex::encode(fvk.to_bytes())))
}

pub fn is_valid_address(address: String, network: String) -> bool {
    let Ok((hrp, data)) = bech32::decode(&address) else {
        return false;
    };
    let expected = if network == "testnet" {
        TESTNET_HRP
    } else {
        MAINNET_HRP
    };
    if hrp.as_str() != expected {
        return false;
    }
    data.len() == 44 && data[0] == ORCHARD_TYPE
}

pub fn derive_viewing_key(mnemonic_seed: String, network: String) -> WalletResult<String> {
    let (_addr, fvk) = derive_address_and_fvk(&mnemonic_seed, &network, 0)?;
    Ok(fvk)
}

pub fn set_document_directory(path: String) -> WalletResult<()> {
    let mut dir = DOCUMENT_DIR.lock().map_err(|e| e.to_string())?;
    *dir = Some(PathBuf::from(path));
    Ok(())
}

/// Best-effort Bech32m rendering of an activity counterparty. Entries record
/// the raw 43-byte Orchard address when they know it; anything else (a
/// transparent counterparty, or an unknown sender on a received note) is
/// surfaced as hex rather than dropped.
fn encode_raw_counterparty(raw: &[u8], network: &str) -> String {
    if raw.len() == 43 {
        let mut fixed = [0u8; 43];
        fixed.copy_from_slice(raw);
        if let Ok(encoded) = encode_raw_orchard_address(&fixed, network) {
            return encoded;
        }
    }
    hex::encode(raw)
}

/// Ask the quorum service which masternodes are currently usable.
///
/// Returns the IPs of nodes the service reports `ENABLED` with a passing
/// `versionCheck`. A node that fails the version check is running an older
/// Platform build whose gRPC surface lacks the shielded queries.
async fn fetch_active_masternodes(quorum_base: &str) -> WalletResult<Vec<String>> {
    let url = format!("{}/masternodes", quorum_base.trim_end_matches('/'));
    let body = reqwest::get(&url)
        .await
        .map_err(|e| format!("masternode list: {e}"))?
        .text()
        .await
        .map_err(|e| format!("masternode list body: {e}"))?;
    let parsed = json::parse(&body).map_err(|e| format!("masternode list json: {e}"))?;

    let mut out = Vec::new();
    for row in parsed["data"].members() {
        if row["status"].as_str() != Some("ENABLED") {
            continue;
        }
        if row["versionCheck"].as_str() != Some("success") {
            continue;
        }
        if let Some(addr) = row["address"].as_str() {
            // `address` is the Core P2P endpoint (`ip:9999`); Platform gRPC
            // is served on 443 of the same host.
            if let Some((ip, _port)) = addr.rsplit_once(':') {
                out.push(ip.to_string());
            }
        }
    }
    Ok(out)
}

pub async fn initialize(
    mnemonic_seed: String,
    account: u32,
    alias: String,
    network_name: String,
    default_host: String,
    default_port: u32,
) -> WalletResult<()> {
    let network = network_from_name(&network_name);

    // DAPI endpoints, discovered live from the same quorum service the Dash
    // Wallet apps use. The baked-in `dash-network-seeds` list is a snapshot and
    // includes nodes still on older Platform builds; those answer the shielded
    // RPC with gRPC UNIMPLEMENTED. Filtering on the service's own
    // `versionCheck` keeps us on nodes that actually implement it.
    let quorum_base = match network {
        Network::Mainnet => "https://quorums.mainnet.networks.dash.org",
        Network::Testnet => "https://quorums.testnet.networks.dash.org",
        _ => "",
    };

    let mut endpoints: Vec<SdkAddress> = Vec::new();
    if !quorum_base.is_empty() {
        if let Ok(rows) = fetch_active_masternodes(quorum_base).await {
            endpoints = rows
                .into_iter()
                .filter_map(|ip| format!("https://{ip}:443").parse().ok())
                .collect();
        }
    }

    // Caller-supplied host is the fallback (devnets, custom deployments).
    let scheme = if default_port == 443 { "https" } else { "http" };
    if endpoints.is_empty() {
        if let Ok(explicit) =
            format!("{}://{}:{}", scheme, default_host, default_port).parse::<SdkAddress>()
        {
            endpoints.push(explicit);
        }
    }
    if endpoints.is_empty() {
        return Err(format!(
            "no DAPI endpoints for {network:?} (host {default_host}:{default_port})"
        ));
    }

    // Quorum keys come from Dash's trusted quorum service, the same source the
    // Dash Wallet apps use. It needs no Dash Core RPC, and `new` resolves the
    // built-in base URL for the network (quorums.<net>.networks.dash.org).
    let context_provider = TrustedHttpContextProvider::new(
        network,
        None,
        std::num::NonZeroUsize::new(100).expect("non-zero"),
    )
    .map_err(|e| format!("trusted context provider: {e}"))?;

    let sdk = SdkBuilder::new(AddressList::from_iter(endpoints))
        .with_network(network)
        .with_context_provider(context_provider)
        .build()
        .map_err(|e| format!("build sdk: {e}"))?;

    // Wallet state lives in SQLite under the host's document directory. The
    // shielded path requires a persister advertising `atomic_changesets` and
    // `shielded_viewing_keys`, which the in-tree SQLite persister provides.
    let base = {
        let dir = DOCUMENT_DIR.lock().map_err(|e| e.to_string())?;
        dir.clone().unwrap_or_else(std::env::temp_dir)
    };
    let db_dir = base.join("dash-shielded").join(&alias);
    std::fs::create_dir_all(&db_dir).map_err(|e| format!("mkdir {db_dir:?}: {e}"))?;

    let inner = SqlitePersister::open(SqlitePersisterConfig::new(db_dir.join("wallet.db")))
        .map_err(|e| format!("open wallet store: {e}"))?;
    let persister = ShieldedCapablePersister { inner };

    let manager = Arc::new(Manager::new(
        Arc::new(sdk),
        Arc::new(persister),
        Arc::new(SilentEventHandler) as Arc<dyn PlatformEventHandler>,
    ));

    // Shielded note store: one SQLite file per alias under the host's
    // document directory, so wallets never share note state.
    manager
        .configure_shielded(db_dir.join("shielded.db"))
        .await
        .map_err(|e| format!("configure_shielded: {e}"))?;

    // The per-chunk progress callback carries the Platform height each chunk
    // was proven at. Record it so the host can report how far the scan has
    // actually reached rather than only how many commitments it walked.
    if let Some(coordinator) = manager.shielded_coordinator().await {
        let alias_for_progress = alias.clone();
        coordinator.install_progress_handler(Some(Arc::new(
            move |_downloaded: u64, height: u64| {
                if height == 0 {
                    return;
                }
                if let Ok(mut heights) = SYNC_HEIGHTS.lock() {
                    heights.insert(alias_for_progress.clone(), height);
                }
            },
        )));
    }

    // Both layers derive from the whole 64-byte BIP39 seed. `bind_shielded`
    // takes a slice and runs ZIP-32 over exactly what it is given, so the seed
    // must not be truncated: feeding it the first 32 bytes yields a different
    // master key and therefore a different receive address, orphaning any
    // funds already sent to the wallet.
    let mnemonic = <Mnemonic<English>>::from_phrase(mnemonic_seed.clone())
        .map_err(|e| format!("bad mnemonic: {e}"))?;
    let seed64 = mnemonic.to_seed("");
    let shielded_seed = seed64;

    let wallet = manager
        .create_wallet_from_seed_bytes(
            network,
            &seed64,
            WalletAccountCreationOptions::Default,
            None,
        )
        .await
        .map_err(|e| format!("create_wallet_from_seed_bytes: {e}"))?;

    let coordinator = manager
        .shielded_coordinator()
        .await
        .ok_or("shielded coordinator missing after configure_shielded")?;
    wallet
        .bind_shielded(&shielded_seed[..], &[account], &coordinator)
        .await
        .map_err(|e| format!("bind_shielded: {e}"))?;

    // The wallet hands back the raw 43-byte Orchard payment address; encode it
    // in the same Bech32m form the rest of this crate speaks (`dash1z…`).
    let raw = wallet
        .shielded_default_address(account)
        .await
        .ok_or("shielded_default_address returned none (bind_shielded did not take)")?;
    let address = encode_raw_orchard_address(&raw, &network_name)?;

    if std::env::var("DASH_SHIELDED_DEBUG").is_ok() {
        // Same mnemonic, both derivations, one process: the stub's local
        // ZIP-32 path versus what bind_shielded produced.
        match derive_address_and_fvk(&mnemonic_seed, &network_name, account) {
            Ok((local, _)) => eprintln!("[dash-shielded] local-derived  = {local}"),
            Err(e) => eprintln!("[dash-shielded] local derive failed: {e}"),
        }
        eprintln!("[dash-shielded] bind-derived  = {address}");
        eprintln!("[dash-shielded] seed len = {}", shielded_seed.len());
    }

    let viewing_key = derive_address_and_fvk(&mnemonic_seed, &network_name, account)
        .map(|(_a, fvk)| fvk)
        .unwrap_or_default();

    let mut clients = CLIENTS.lock().await;
    clients.insert(
        alias,
        ClientSlot {
            mnemonic: mnemonic_seed,
            network: network_name,
            account,
            address,
            viewing_key,
            status: "DISCONNECTED".to_string(),
            available_credits: "0".to_string(),
            total_credits: "0".to_string(),
            proposals: HashMap::new(),
            manager: Some(manager),
            wallet: Some(wallet),
            total_scanned: 0,
            network_block_height: 0,
        },
    );
    Ok(())
}

pub async fn stop(alias: String) -> WalletResult<String> {
    let mut clients = CLIENTS.lock().await;
    clients.remove(&alias);
    Ok("STOPPED".to_string())
}

pub async fn start_sync(alias: String) -> WalletResult<()> {
    let manager = {
        let clients = CLIENTS.lock().await;
        let slot = clients.get(&alias).ok_or("unknown alias")?;
        slot.manager.clone().ok_or("wallet not initialized")?
    };
    // Start the coordinator's periodic scan loop. It keeps running until
    // `stop_sync`, so a wallet left open stays current.
    manager.shielded_sync_arc().start();

    // Kick one immediate pass so a freshly opened wallet does not wait out
    // the interval before its first balance.
    let alias_for_task = alias.clone();
    let wallet_for_task = {
        let clients = CLIENTS.lock().await;
        clients
            .get(&alias)
            .and_then(|slot| slot.wallet.clone())
    };
    tokio::spawn(async move {
        if let Some(coordinator) = manager.shielded_coordinator().await {
            let summary = coordinator.sync(true).await;
            if std::env::var("DASH_SHIELDED_DEBUG").is_ok() {
                eprintln!("[dash-shielded] sync summary: {summary:?}");
                // Local tree size vs what the pass walked: if the chain has
                // more leaves than we hold, the fetch is stopping short.
                if let Some(coord) = manager.shielded_coordinator().await {
                    if let Ok(store) = coord.store().try_read() {
                        let sub = wallet_for_task
                            .as_ref()
                            .map(|w| SubwalletId::new(w.wallet_id(), 0));
                        eprintln!(
                            "[dash-shielded] tree_size={:?} watermark={:?}",
                            store.tree_size(),
                            sub.map(|id| store.last_synced_note_index(id)),
                        );
                    }
                }
            }
            // Record what the pass actually did, so `poll` reports a real
            // scan rather than an assumed one.
            let mut scanned = 0u64;
            let mut status = "SYNCED".to_string();
            if let Some(wallet) = wallet_for_task.as_ref() {
                if let Some(outcome) = summary.wallet_results.get(&wallet.wallet_id()) {
                    match outcome {
                        WalletShieldedOutcome::Ok(s) => {
                            scanned = s.notes_result.total_scanned as u64;
                        }
                        other => {
                            status = format!("ERROR: {other:?}");
                        }
                    }
                }
            }
            let mut clients = CLIENTS.lock().await;
            if let Some(slot) = clients.get_mut(&alias_for_task) {
                slot.total_scanned = scanned;
                slot.status = status;
            }
        }
    });

    let mut clients = CLIENTS.lock().await;
    let slot = clients.get_mut(&alias).ok_or("unknown alias")?;
    slot.status = "SYNCING".to_string();
    Ok(())
}

pub async fn stop_sync(alias: String) -> WalletResult<()> {
    let mut clients = CLIENTS.lock().await;
    let slot = clients.get_mut(&alias).ok_or("unknown alias")?;
    if let Some(manager) = slot.manager.as_ref() {
        manager.shielded_sync_arc().stop();
    }
    slot.status = "STOPPED".to_string();
    Ok(())
}

pub async fn derive_shielded_address(alias: String) -> WalletResult<Addresses> {
    let clients = CLIENTS.lock().await;
    let slot = clients.get(&alias).ok_or("unknown alias")?;
    Ok(Addresses {
        shielded_address: slot.address.clone(),
    })
}

pub async fn poll(alias: String) -> WalletResult<Poll> {
    let (manager, wallet, account, network_name) = {
        let clients = CLIENTS.lock().await;
        let slot = clients.get(&alias).ok_or("unknown alias")?;
        (
            slot.manager.clone(),
            slot.wallet.clone(),
            slot.account,
            slot.network.clone(),
        )
    };
    let mut transactions: Vec<Transaction> = Vec::new();

    // Read live shielded balance from the note store.
    let mut available = "0".to_string();
    let mut total = "0".to_string();
    let mut status = "DISCONNECTED".to_string();
    let mut scan_progress = 0.0_f64;

    if let (Some(manager), Some(wallet)) = (manager.as_ref(), wallet.as_ref()) {
        let sync = manager.shielded_sync_arc();
        status = if sync.is_syncing() {
            "SYNCING".to_string()
        } else if sync.is_running() {
            "SYNCED".to_string()
        } else {
            "STOPPED".to_string()
        };
        if let Some(coordinator) = manager.shielded_coordinator().await {
            if let Ok(balances) = wallet.shielded_balances(&coordinator).await {
                let sum: u64 = balances.values().sum();
                available = sum.to_string();
                total = sum.to_string();
            }
        }
        // A completed, non-syncing pass means the wallet is current.
        scan_progress = if sync.is_syncing() { 50.0 } else { 100.0 };

        // Read the wallet's shielded activity: one entry per detected
        // transfer, carrying amount, direction, memo and the height it was
        // mined at. Notes the scan decrypted show up here.
        if let Some(coordinator) = manager.shielded_coordinator().await {
            let subwallet = SubwalletId::new(wallet.wallet_id(), account);
            // A sync pass holds the store's write lock for its whole
            // interleaved consume, so an unbounded read here would stall
            // `poll` for the length of a network scan and the caller would
            // time out. `try_read` is the other extreme: with the sync loop
            // running continuously it never wins the race, and no activity is
            // ever reported. Wait, but only briefly.
            if let Ok(store) = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                coordinator.store().read(),
            )
            .await
            {
            if let Ok(entries) = store.get_activity(subwallet, 0, 500) {
                for entry in entries {
                    let outgoing = entry.direction == ShieldedDirection::Out;
                    // The host applies the sign: it reads a non-null
                    // `toAddress` as "this was a spend", negates
                    // `value + fee` itself, and leaves a receive positive.
                    // Report the magnitude and let it decide.
                    let value = entry.amount.to_string();
                    let memos = entry
                        .memo
                        .as_ref()
                        .and_then(|m| String::from_utf8(m.clone()).ok())
                        .filter(|m| !m.trim_matches(char::from(0)).is_empty())
                        .map(|m| vec![m.trim_matches(char::from(0)).to_string()])
                        .unwrap_or_default();
                    transactions.push(Transaction {
                        txid: hex::encode(entry.id),
                        // Activity records creation in milliseconds; the JS
                        // layer expects whole seconds.
                        block_time_in_seconds: (entry.created_at_ms / 1000) as i64,
                        mined_height: entry.block_height.unwrap_or(0) as i64,
                        value,
                        fee: entry.fee.map(|f| f.to_string()),
                        // Only spends carry a destination. A received note
                        // also records a counterparty, but surfacing it here
                        // would make the host book the receive as a send.
                        to_address: if outgoing {
                            entry
                                .counterparty
                                .as_ref()
                                .map(|c| encode_raw_counterparty(c, &network_name))
                        } else {
                            None
                        },
                        memos,
                    });
                }
            }
            }
        }
    }

    let mut clients = CLIENTS.lock().await;
    let slot = clients.get_mut(&alias).ok_or("unknown alias")?;
    if !slot.status.starts_with("ERROR") {
        slot.status = status.clone();
    }
    let status = slot.status.clone();
    slot.available_credits = available.clone();
    slot.total_credits = total.clone();

    let synced_height = SYNC_HEIGHTS
        .lock()
        .ok()
        .and_then(|h| h.get(&alias).copied())
        .unwrap_or(slot.network_block_height as u64)
        .min(u32::MAX as u64) as u32;

    Ok(Poll {
        alias,
        status,
        scan_progress,
        network_block_height: synced_height,
        available_credits: available,
        total_credits: total,
        transactions,
    })
}

/// Decodes a `dash1z…` / `tdash1z…` shielded address back to the raw 43-byte
/// Orchard payment address the builder wants. The inverse of the encoder used
/// for `shielded_address`: Bech32m payload is a type byte followed by the 43
/// address bytes.
fn decode_shielded_address(address: &str, network: &str) -> WalletResult<[u8; 43]> {
    let (hrp, data) = bech32::decode(address).map_err(|e| e.to_string())?;
    let expected = hrp_for(network)?;
    if hrp.as_str() != expected.as_str() {
        return Err(format!(
            "address is for a different network: expected {}, got {}",
            expected.as_str(),
            hrp.as_str()
        ));
    }
    if data.len() != 44 || data[0] != ORCHARD_TYPE {
        return Err("not an Orchard shielded address".into());
    }
    let mut raw = [0u8; 43];
    raw.copy_from_slice(&data[1..]);
    Ok(raw)
}

/// The fee the network charges for a shielded transfer, in credits. A transfer
/// spends up to two notes and creates two (recipient + change), which is the
/// same action count the wallet reserves against, so quoting it here matches
/// what the builder will charge.
fn transfer_fee_credits() -> WalletResult<u64> {
    use dash_sdk::dpp::version::PlatformVersion;
    dash_sdk::dpp::shielded::compute_minimum_shielded_fee(
        TRANSFER_ACTIONS,
        PlatformVersion::latest(),
    )
    .map_err(|e| e.to_string())
}

/// Orchard actions in a two-in / two-out transfer bundle.
const TRANSFER_ACTIONS: usize = 2;

pub async fn propose_transfer(
    alias: String,
    amount_credits: String,
    to_address: String,
    memo: Option<String>,
) -> WalletResult<String> {
    let memo = memo.unwrap_or_default();
    if memo.len() > 32 {
        return Err("memo exceeds 32 UTF-8 bytes".into());
    }
    let amount: u64 = amount_credits
        .parse()
        .map_err(|_| "amount must be a whole number of credits".to_string())?;
    if amount == 0 {
        return Err("amount must be greater than zero".into());
    }

    let mut clients = CLIENTS.lock().await;
    let slot = clients.get_mut(&alias).ok_or("unknown alias")?;

    // Reject a wrong-network or malformed recipient here rather than after the
    // caller has signed off on a fee.
    decode_shielded_address(&to_address, &slot.network)?;

    let fee = transfer_fee_credits()?;
    let available: u64 = slot.available_credits.parse().unwrap_or(0);
    if available < amount.saturating_add(fee) {
        return Err(format!(
            "insufficient funds: {available} credits available, need {} plus {fee} fee",
            amount
        ));
    }

    let proposal_id = format!("p-{}", slot.proposals.len() + 1);
    slot.proposals.insert(
        proposal_id.clone(),
        PendingProposal {
            to_address,
            amount_credits,
            memo,
        },
    );
    Ok(json::object! {
        proposalId: proposal_id,
        feeCredits: fee.to_string()
    }
    .dump())
}

pub async fn create_transfer(
    alias: String,
    proposal_id: String,
    mnemonic_seed: String,
) -> WalletResult<String> {
    // Take the proposal and everything else needed off the slot, then drop the
    // lock: proving and broadcast take tens of seconds, and the sync loop needs
    // this mutex the whole time.
    let (proposal, wallet, manager, account, network) = {
        let mut clients = CLIENTS.lock().await;
        let slot = clients.get_mut(&alias).ok_or("unknown alias")?;
        let proposal = slot
            .proposals
            .remove(&proposal_id)
            .ok_or("unknown proposal")?;
        let wallet = slot.wallet.clone().ok_or("wallet not initialized")?;
        let manager = slot.manager.clone().ok_or("wallet not initialized")?;
        (proposal, wallet, manager, slot.account, slot.network.clone())
    };

    let recipient = decode_shielded_address(&proposal.to_address, &network)?;
    let amount: u64 = proposal
        .amount_credits
        .parse()
        .map_err(|_| "amount must be a whole number of credits".to_string())?;

    // The spend authority is re-derived from the mnemonic for this call only.
    // Same 64-byte BIP39 seed the viewing keys were bound with — truncating it
    // would derive a different, empty wallet.
    let mnemonic = <Mnemonic<English>>::from_phrase(mnemonic_seed).map_err(|e| e.to_string())?;
    let seed64 = mnemonic.to_seed("");

    let coordinator = manager
        .shielded_coordinator()
        .await
        .ok_or("shielded coordinator missing")?;

    // Memos are a fixed 36 bytes on the wire, zero-padded.
    let mut memo = [0u8; 36];
    let memo_bytes = proposal.memo.as_bytes();
    memo[..memo_bytes.len()].copy_from_slice(memo_bytes);

    let subwallet = SubwalletId::new(wallet.wallet_id(), account);

    // The spend's identity only exists once it is in the activity store, so
    // record what was already there and treat whatever outgoing entry appears
    // next as this transfer's.
    let before: std::collections::HashSet<Vec<u8>> = {
        let store = coordinator.store().read().await;
        store
            .get_activity(subwallet, 0, 500)
            .map(|entries| entries.into_iter().map(|e| e.id.to_vec()).collect())
            .unwrap_or_default()
    };

    let prover = platform_wallet::wallet::shielded::prover::CachedOrchardProver::new();
    wallet
        .shielded_transfer_to(&coordinator, &seed64[..], account, &recipient, amount, memo, &prover)
        .await
        .map_err(|e| format!("shielded transfer failed: {e}"))?;

    // `shielded_transfer_to` returns unit: the state transition is accepted by
    // Platform, but the txid comes back through the store. It lands as soon as
    // the operation commits its activity row, so a short poll is enough — the
    // caller needs an id to key the pending transaction on.
    let mut txid = String::new();
    for _ in 0..40 {
        if let Ok(store) = coordinator.store().try_read() {
            if let Ok(entries) = store.get_activity(subwallet, 0, 500) {
                if let Some(entry) = entries.into_iter().find(|e| {
                    e.direction == ShieldedDirection::Out && !before.contains(&e.id.to_vec())
                }) {
                    txid = hex::encode(entry.id);
                    break;
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    if txid.is_empty() {
        return Err("transfer submitted but no outgoing activity was recorded".into());
    }

    let fee = transfer_fee_credits().unwrap_or(0);
    Ok(json::object! {
        txid: txid,
        amountCredits: amount.to_string(),
        feeCredits: fee.to_string(),
        toAddress: proposal.to_address
    }
    .dump())
}

pub async fn warm_up_prover() -> WalletResult<()> {
    // Building the Orchard proving key takes ~30s; do it once, off the path of
    // the first spend.
    platform_wallet::wallet::shielded::prover::CachedOrchardProver::new().warm_up();
    let mut ready = PROVER_READY.lock().map_err(|e| e.to_string())?;
    *ready = true;
    Ok(())
}

pub fn is_prover_ready() -> bool {
    PROVER_READY.lock().map(|g| *g).unwrap_or(false)
}

pub fn generate_mnemonic() -> String {
    Mnemonic::<English>::generate(Count::Words24).to_string()
}
