import {
  EventSubscription,
  NativeEventEmitter,
  NativeModules
} from 'react-native'

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

const { RNDashShielded } = NativeModules

function parseJsonObject(value: string): unknown {
  try {
    return JSON.parse(value)
  } catch {
    return { errorMessage: value }
  }
}

export const Tools = {
  deriveViewingKey: async (
    mnemonicSeed: string,
    network: Network
  ): Promise<ViewingKeySet> => {
    const fullViewingKey = await RNDashShielded.deriveViewingKey(
      mnemonicSeed,
      network
    )
    return { fullViewingKey }
  },
  deriveShieldedAddress: async (
    mnemonicSeed: string,
    network: Network,
    account: number = 0
  ): Promise<string> => {
    return RNDashShielded.deriveShieldedAddressFromSeed(
      mnemonicSeed,
      network,
      account
    )
  },
  isValidAddress: async (
    address: string,
    network: Network = 'mainnet'
  ): Promise<boolean> => {
    return RNDashShielded.isValidAddress(address, network)
  },
  warmUpProver: async (): Promise<void> => {
    await RNDashShielded.warmUpProver()
  },
  isProverReady: async (): Promise<boolean> => {
    return RNDashShielded.isProverReady()
  }
}

export class Synchronizer {
  eventEmitter: NativeEventEmitter
  subscriptions: EventSubscription[]
  alias: string
  network: Network
  private timer?: ReturnType<typeof setTimeout>
  private callbacks?: SynchronizerCallbacks
  private lastStatus?: string

  constructor(alias: string, network: Network) {
    this.eventEmitter = new NativeEventEmitter(RNDashShielded)
    this.subscriptions = []
    this.alias = alias
    this.network = network
  }

  async stop(): Promise<string> {
    this.unsubscribe()
    return RNDashShielded.stop(this.alias)
  }

  async initialize(config: InitializerConfig): Promise<void> {
    const seed = config.mnemonicSeed ?? config.seedHex
    if (seed == null) throw new Error('Missing mnemonicSeed')
    await RNDashShielded.initialize(
      seed,
      config.account,
      config.alias,
      config.network,
      config.defaultHost,
      config.defaultPort
    )
  }

  async startSync(): Promise<void> {
    await RNDashShielded.startSync(this.alias)
  }

  async stopSync(): Promise<void> {
    await RNDashShielded.stopSync(this.alias)
  }

  async deriveShieldedAddress(): Promise<Addresses> {
    return RNDashShielded.deriveShieldedAddress(this.alias)
  }

  async coreReceiveAddress(account: number = 0): Promise<string> {
    return RNDashShielded.coreReceiveAddress(this.alias, account)
  }

  async startCoreSync(fromHeight: number): Promise<void> {
    await RNDashShielded.startCoreSync(this.alias, fromHeight)
  }

  async coreBalance(): Promise<CoreBalance> {
    return RNDashShielded.coreBalance(this.alias)
  }

  async shieldFromAssetLock(
    amountDuffs: string,
    mnemonicSeed: string,
    accountIndex: number = 0
  ): Promise<string> {
    return RNDashShielded.shieldFromAssetLock(
      this.alias,
      amountDuffs,
      accountIndex,
      mnemonicSeed
    )
  }

  async platformReceiveAddress(account: number = 0): Promise<string> {
    return RNDashShielded.platformReceiveAddress(this.alias, account)
  }

  async trackedAssetLocks(): Promise<AssetLock[]> {
    return RNDashShielded.trackedAssetLocks(this.alias)
  }

  async resumeShieldFromAssetLock(
    txid: string,
    vout: number,
    mnemonicSeed: string
  ): Promise<string> {
    return RNDashShielded.resumeShieldFromAssetLock(
      this.alias,
      txid,
      vout,
      mnemonicSeed
    )
  }

  async unshield(
    toAddress: string,
    amountCredits: string,
    mnemonicSeed: string
  ): Promise<string> {
    return RNDashShielded.unshield(
      this.alias,
      toAddress,
      amountCredits,
      mnemonicSeed
    )
  }

  async shieldedWithdraw(
    toCoreAddress: string,
    amountCredits: string,
    mnemonicSeed: string,
    coreFeePerByte: number = 1
  ): Promise<string> {
    return RNDashShielded.shieldedWithdraw(
      this.alias,
      toCoreAddress,
      amountCredits,
      coreFeePerByte,
      mnemonicSeed
    )
  }

  async proposeTransfer(opts: ProposeTransferOpts): Promise<ProposalSuccess> {
    const result = await RNDashShielded.proposeTransfer(
      this.alias,
      opts.amountCredits,
      opts.toAddress,
      opts.memo
    )
    return parseJsonObject(result) as ProposalSuccess
  }

  async createTransfer(
    opts: CreateTransferOpts
  ): Promise<SpendSuccess | SpendFailure> {
    try {
      const result = await RNDashShielded.createTransfer(
        this.alias,
        opts.proposalId,
        opts.mnemonicSeed
      )
      return parseJsonObject(result) as SpendSuccess
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
    this.subscriptions.forEach(subscription => {
      subscription.remove()
    })
    this.subscriptions = []
  }

  private async pump(): Promise<void> {
    if (this.callbacks == null) return
    const snap = await RNDashShielded.poll(this.alias)
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
  const synchronizer = new Synchronizer(config.alias, config.network)
  await synchronizer.initialize(config)
  return synchronizer
}
