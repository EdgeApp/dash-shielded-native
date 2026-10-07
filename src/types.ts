export type Network = 'mainnet' | 'testnet'

export interface ViewingKeySet {
  fullViewingKey: string
}

export interface InitializerConfig {
  network: Network
  seedHex?: string
  mnemonicSeed?: string
  account: number
  alias: string
  dataDir: string
  defaultHost: string
  defaultPort: number
}

export interface ProposeTransferOpts {
  toAddress: string
  amountCredits: string
  memo?: string
}

export interface CreateTransferOpts {
  proposalId: string
  mnemonicSeed: string
}

export interface ProposalSuccess {
  proposalId: string
  feeCredits: string
}

export interface SpendSuccess {
  txid: string
}

export interface SpendFailure {
  errorMessage?: string
}

export interface BalanceEvent {
  availableCredits: string
  totalCredits: string
}

export interface StatusEvent {
  alias: string
  name: 'STOPPED' | 'DISCONNECTED' | 'SYNCING' | 'SYNCED'
}

export interface DashShieldedTx {
  txid: string
  blockTimeInSeconds: number
  minedHeight: number
  value: string
  fee?: string
  toAddress?: string
  memos: string[]
}

export interface TransactionEvent {
  transactions: DashShieldedTx[]
}

export interface UpdateEvent {
  alias: string
  scanProgress: number
  networkBlockHeight: number
}

export interface ErrorEvent {
  alias: string
  level: 'critical' | 'error'
  message: string
}

export interface SynchronizerCallbacks {
  onBalanceChanged: (balance: BalanceEvent) => void
  onStatusChanged: (status: StatusEvent) => void
  onTransactionsChanged: (transactions: TransactionEvent) => void
  onUpdate: (event: UpdateEvent) => void
  onError: (error: ErrorEvent) => void
}

export interface Addresses {
  shieldedAddress: string
}

/** Transparent (L1) balance in duffs, with the SPV client's header height. */
export interface CoreBalance {
  confirmedDuffs: string
  unconfirmedDuffs: string
  totalDuffs: string
  syncedHeight: number
}

/**
 * One asset lock the wallet is tracking. `status` runs `built`, `broadcast`,
 * `instantSendLocked` in the native casing `instant_send_locked`,
 * `chain_locked`, then `consumed`. Anything short of `consumed` is value in a
 * lock that never became a note, which `resumeShieldFromAssetLock` takes.
 *
 * A lock rebuilt from the wallet's chain records in a later process reads
 * `recovered_from_chain`: Core has finalized it, and the wallet cannot tell
 * locally whether Platform already consumed it. Resuming one is how a host
 * finds out. A stranded lock shields; a spent one returns an error, moves no
 * value and keeps this status.
 */
export interface AssetLock {
  txid: string
  vout: number
  status: string
}
