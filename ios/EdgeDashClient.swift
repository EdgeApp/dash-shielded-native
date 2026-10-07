import Foundation

/// Thin names over the UniFFI `dash` namespace so RNDashShielded can keep
/// Objective-C selector names without shadowing the Rust functions.
enum EdgeDashClient {
  static func rustSetDocumentDirectory(path: String) throws {
    try setDocumentDirectory(path: path)
  }

  static func rustInitialize(
    mnemonicSeed: String,
    account: UInt32,
    alias: String,
    networkName: String,
    defaultHost: String,
    defaultPort: UInt32
  ) throws {
    try initialize(
      mnemonicSeed: mnemonicSeed,
      account: account,
      alias: alias,
      networkName: networkName,
      defaultHost: defaultHost,
      defaultPort: defaultPort
    )
  }

  static func rustStop(alias: String) throws -> String {
    try stop(alias: alias)
  }

  static func rustStartSync(alias: String) throws {
    try startSync(alias: alias)
  }

  static func rustStopSync(alias: String) throws {
    try stopSync(alias: alias)
  }

  static func rustDeriveShieldedAddress(alias: String) throws -> Addresses {
    try deriveShieldedAddress(alias: alias)
  }

  static func rustCoreReceiveAddress(alias: String, account: UInt32) throws -> String {
    try coreReceiveAddress(alias: alias, account: account)
  }

  static func rustStartCoreSync(alias: String, fromHeight: UInt32) throws {
    try startCoreSync(alias: alias, fromHeight: fromHeight)
  }

  static func rustCoreBalance(alias: String) throws -> CoreBalance {
    try coreBalance(alias: alias)
  }

  static func rustShieldFromAssetLock(
    alias: String, amountDuffs: String, accountIndex: UInt32, mnemonicSeed: String
  ) throws -> String {
    try shieldFromAssetLock(
      alias: alias, amountDuffs: amountDuffs, accountIndex: accountIndex,
      mnemonicSeed: mnemonicSeed)
  }

  static func rustPlatformReceiveAddress(alias: String, account: UInt32) throws -> String {
    try platformReceiveAddress(alias: alias, account: account)
  }

  static func rustTrackedAssetLocks(alias: String) throws -> [AssetLock] {
    try trackedAssetLocks(alias: alias)
  }

  static func rustResumeShieldFromAssetLock(
    alias: String, txid: String, vout: UInt32, mnemonicSeed: String
  ) throws -> String {
    try resumeShieldFromAssetLock(
      alias: alias, txid: txid, vout: vout, mnemonicSeed: mnemonicSeed)
  }

  static func rustUnshield(
    alias: String, toAddress: String, amountCredits: String, mnemonicSeed: String
  ) throws -> String {
    try unshield(
      alias: alias, toAddress: toAddress, amountCredits: amountCredits,
      mnemonicSeed: mnemonicSeed)
  }

  static func rustShieldedWithdraw(
    alias: String, toCoreAddress: String, amountCredits: String, coreFeePerByte: UInt32,
    mnemonicSeed: String
  ) throws -> String {
    try shieldedWithdraw(
      alias: alias, toCoreAddress: toCoreAddress, amountCredits: amountCredits,
      coreFeePerByte: coreFeePerByte, mnemonicSeed: mnemonicSeed)
  }

  static func rustIsValidAddress(address: String, network: String) -> Bool {
    isValidAddress(address: address, network: network)
  }

  static func rustDeriveViewingKey(mnemonicSeed: String, network: String) throws -> String {
    try deriveViewingKey(mnemonicSeed: mnemonicSeed, network: network)
  }

  static func rustWarmUpProver() throws {
    try warmUpProver()
  }

  static func rustIsProverReady() -> Bool {
    isProverReady()
  }

  static func rustPoll(alias: String) throws -> Poll {
    try poll(alias: alias)
  }

  static func rustProposeTransfer(
    alias: String, amountCredits: String, toAddress: String, memo: String?
  ) throws -> String {
    try proposeTransfer(
      alias: alias,
      amountCredits: amountCredits,
      toAddress: toAddress,
      memo: memo
    )
  }

  static func rustCreateTransfer(
    alias: String, proposalId: String, mnemonicSeed: String
  ) throws -> String {
    try createTransfer(
      alias: alias, proposalId: proposalId, mnemonicSeed: mnemonicSeed)
  }

  static func rustDeriveShieldedAddressFromSeed(
    mnemonicSeed: String, network: String, account: UInt32
  ) throws -> String {
    try deriveShieldedAddressFromSeed(
      mnemonicSeed: mnemonicSeed, network: network, account: account)
  }
}
