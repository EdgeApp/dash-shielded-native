import { mkdirSync } from 'fs'

import { loadNativeAddon, NativeDashAddon } from './load-addon'
import {
  Addresses,
  AssetLock,
  CoreBalance,
  CreateTransferOpts,
  InitializerConfig,
  Network,
  ProposalSuccess,
  ProposeTransferOpts,
  SpendFailure,
  SpendSuccess,
  SynchronizerCallbacks,
  ViewingKeySet
} from './types'

export * from './types'

export interface MakeNodeDashShieldedOpts {
  documentDirectory: string
}

function parseJsonObject(value: string): unknown {
  try {
    return JSON.parse(value)
  } catch {
    return { errorMessage: value }
  }
}

export const Tools = {
  deriveViewingKey: (
    mnemonicSeed: string,
    network: Network
  ): Promise<ViewingKeySet> => {
    const addon = loadNativeAddon()
    return Promise.resolve({
      fullViewingKey: addon.deriveViewingKey(mnemonicSeed, network)
    })
  },
  deriveShieldedAddress: async (
    mnemonicSeed: string,
    network: Network,
    account: number = 0
  ): Promise<string> => {
    const addon = loadNativeAddon()
    const alias = `tools-addr-${network}-${account}`
    await addon.initialize(mnemonicSeed, account, alias, network, '', 0)
    const { shieldedAddress } = await addon.deriveShieldedAddress(alias)
    await addon.stop(alias)
    return shieldedAddress
  },
  isValidAddress: (
    address: string,
    network: Network = 'mainnet'
  ): Promise<boolean> => {
    const addon = loadNativeAddon()
    return Promise.resolve(addon.isValidAddress(address, network))
  },
  warmUpProver: async (): Promise<void> => {
    const addon = loadNativeAddon()
    await addon.warmUpProver()
  },
  isProverReady: (): Promise<boolean> => {
    const addon = loadNativeAddon()
    return Promise.resolve(addon.isProverReady())
  }
}

export class Synchronizer {
  alias: string
  network: Network
  private readonly addon: NativeDashAddon
  private timer?: ReturnType<typeof setTimeout>
  private callbacks?: SynchronizerCallbacks
  private lastStatus?: string

  constructor(alias: string, network: Network, addon: NativeDashAddon) {
    this.alias = alias
    this.network = network
    this.addon = addon
  }

  async stop(): Promise<string> {
    this.unsubscribe()
    return await this.addon.stop(this.alias)
  }

  async initialize(config: InitializerConfig): Promise<void> {
    const seed = config.mnemonicSeed ?? config.seedHex
    if (seed == null) throw new Error('Missing mnemonicSeed')
    await this.addon.initialize(
      seed,
      config.account,
      config.alias,
      config.network,
      config.defaultHost,
      config.defaultPort
    )
  }

  async startSync(): Promise<void> {
    await this.addon.startSync(this.alias)
  }

  async stopSync(): Promise<void> {
    await this.addon.stopSync(this.alias)
  }

  async deriveShieldedAddress(): Promise<Addresses> {
    return await this.addon.deriveShieldedAddress(this.alias)
  }

  /**
   * Next unused transparent (L1) receive address, for getting value into the
   * wallet in the first place. The shielded pool is only reachable from L1, so
   * a fresh wallet has to be funded here before any shielded operation works.
   */
  async coreReceiveAddress(account: number = 0): Promise<string> {
    return await this.addon.coreReceiveAddress(this.alias, account)
  }

  /**
   * Next unused transparent Platform address, the counterpart to
   * `coreReceiveAddress` one layer up. This is where `unshield` puts value that
   * leaves the Orchard pool but stays on Platform.
   */
  async platformReceiveAddress(account: number = 0): Promise<string> {
    return await this.addon.platformReceiveAddress(this.alias, account)
  }

  /**
   * Start the Core (L1) SPV sync. The Platform connection cannot see L1, so
   * transparent balance stays at zero until this runs. `fromHeight` skips
   * history older than the wallet.
   */
  async startCoreSync(fromHeight: number): Promise<void> {
    await this.addon.startCoreSync(this.alias, fromHeight)
  }

  async coreBalance(): Promise<CoreBalance> {
    return await this.addon.coreBalance(this.alias)
  }

  /**
   * Move transparent balance into the shielded pool through an asset lock.
   *
   * Takes the seed per call rather than reusing the one `initialize` was given.
   * An alias is not a secret, so a resident seed would make knowing the alias
   * enough to move funds.
   * Builds and broadcasts the L1 lock, waits for its InstantSend or ChainLock
   * proof, then proves and broadcasts the shielding transition, so this takes
   * seconds and needs a synced Core balance to spend.
   */
  async shieldFromAssetLock(
    amountDuffs: string,
    mnemonicSeed: string,
    accountIndex: number = 0
  ): Promise<string> {
    return await this.addon.shieldFromAssetLock(
      this.alias,
      amountDuffs,
      accountIndex,
      mnemonicSeed
    )
  }

  /**
   * The asset locks this wallet is tracking. A host that crashed mid-shield has
   * no other way to learn the outpoint `resumeShieldFromAssetLock` needs, since
   * the failing call returns an error rather than the lock it broadcast.
   *
   * Locks live only in memory: the persister writes their rows but its `load`
   * never reads them back, so this is empty on a fresh open however much
   * history the store holds.
   */
  async trackedAssetLocks(): Promise<AssetLock[]> {
    return await this.addon.trackedAssetLocks(this.alias)
  }

  /**
   * Finish a shield whose L1 asset lock is already on chain. A failure between
   * broadcasting the lock and proving the transition strands the locked value,
   * and only resuming that outpoint recovers it; building a fresh lock cannot.
   */
  async resumeShieldFromAssetLock(
    txid: string,
    vout: number,
    mnemonicSeed: string
  ): Promise<string> {
    return await this.addon.resumeShieldFromAssetLock(
      this.alias,
      txid,
      vout,
      mnemonicSeed
    )
  }

  /** Move shielded value out to a transparent Platform address. */
  async unshield(
    toAddress: string,
    amountCredits: string,
    mnemonicSeed: string
  ): Promise<string> {
    return await this.addon.unshield(
      this.alias,
      toAddress,
      amountCredits,
      mnemonicSeed
    )
  }

  /**
   * Move shielded value out to a Core L1 address. `coreFeePerByte` prices the
   * L1 transaction the network builds on the far side, in duffs per byte.
   */
  async shieldedWithdraw(
    toCoreAddress: string,
    amountCredits: string,
    mnemonicSeed: string,
    coreFeePerByte: number = 1
  ): Promise<string> {
    return await this.addon.shieldedWithdraw(
      this.alias,
      toCoreAddress,
      amountCredits,
      coreFeePerByte,
      mnemonicSeed
    )
  }

  async getBalance(): Promise<{
    availableCredits: string
    totalCredits: string
  }> {
    const snap = await this.addon.poll(this.alias)
    return {
      availableCredits: snap.availableCredits,
      totalCredits: snap.totalCredits
    }
  }

  async getTransactions(): Promise<
    Array<{
      txid: string
      blockTimeInSeconds: number
      minedHeight: number
      value: string
      fee?: string
      toAddress?: string
      memos: string[]
    }>
  > {
    const snap = await this.addon.poll(this.alias)
    return snap.transactions
  }

  async proposeTransfer(opts: ProposeTransferOpts): Promise<ProposalSuccess> {
    const raw = await this.addon.proposeTransfer(
      this.alias,
      opts.amountCredits,
      opts.toAddress,
      opts.memo
    )
    return parseJsonObject(raw) as ProposalSuccess
  }

  async createTransfer(
    opts: CreateTransferOpts
  ): Promise<SpendSuccess | SpendFailure> {
    try {
      const raw = await this.addon.createTransfer(
        this.alias,
        opts.proposalId,
        opts.mnemonicSeed
      )
      return parseJsonObject(raw) as SpendSuccess
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : String(error)
      return { errorMessage }
    }
  }

  subscribe(callbacks: SynchronizerCallbacks): void {
    this.callbacks = callbacks
    this.pump().catch(error => {
      callbacks.onError({
        alias: this.alias,
        level: 'error',
        message: `event pump failed: ${String(error)}`
      })
    })
  }

  unsubscribe(): void {
    if (this.timer != null) {
      clearTimeout(this.timer)
      this.timer = undefined
    }
    this.callbacks = undefined
  }

  private async pump(): Promise<void> {
    if (this.callbacks == null) return
    const snap = await this.addon.poll(this.alias)
    const {
      onBalanceChanged,
      onStatusChanged,
      onTransactionsChanged,
      onUpdate
    } = this.callbacks

    onBalanceChanged({
      availableCredits: snap.availableCredits,
      totalCredits: snap.totalCredits
    })

    if (snap.status !== this.lastStatus) {
      this.lastStatus = snap.status
      onStatusChanged({
        alias: this.alias,
        name: snap.status as 'STOPPED' | 'DISCONNECTED' | 'SYNCING' | 'SYNCED'
      })
    }

    onTransactionsChanged({ transactions: snap.transactions })
    onUpdate({
      alias: this.alias,
      scanProgress: snap.scanProgress,
      networkBlockHeight: snap.networkBlockHeight
    })

    const delay = snap.status === 'SYNCING' ? 500 : 2000
    this.timer = setTimeout(() => {
      this.pump().catch(error => {
        this.callbacks?.onError({
          alias: this.alias,
          level: 'error',
          message: `event pump failed: ${String(error)}`
        })
      })
    }, delay)
  }
}

export const makeSynchronizer = async (
  config: InitializerConfig
): Promise<Synchronizer> => {
  const addon = loadNativeAddon()
  const synchronizer = new Synchronizer(config.alias, config.network, addon)
  await synchronizer.initialize(config)
  return synchronizer
}

export function makeNodeDashShieldedModule(opts: MakeNodeDashShieldedOpts): {
  Tools: typeof Tools
  makeSynchronizer: typeof makeSynchronizer
} {
  mkdirSync(opts.documentDirectory, { recursive: true })
  const addon = loadNativeAddon()
  addon.setDocumentDirectory(opts.documentDirectory)
  return { Tools, makeSynchronizer }
}
