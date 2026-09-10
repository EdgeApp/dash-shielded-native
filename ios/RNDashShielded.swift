import Foundation

@objc(RNDashShielded)
class RNDashShielded: RCTEventEmitter {
  private static var didSetDocumentDirectory = false

  override static func requiresMainQueueSetup() -> Bool {
    return true
  }

  private func ensureDocumentDirectory() throws {
    if RNDashShielded.didSetDocumentDirectory {
      return
    }
    let root = try FileManager.default.url(
      for: .documentDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
    let path = root.appendingPathComponent("native/dashshielded", isDirectory: true).path
    try EdgeDashClient.rustSetDocumentDirectory(path: path)
    RNDashShielded.didSetDocumentDirectory = true
  }

  private func run(
    _ resolve: @escaping RCTPromiseResolveBlock,
    _ reject: @escaping RCTPromiseRejectBlock,
    _ name: String,
    _ body: @escaping () throws -> Any?
  ) {
    DispatchQueue.global(qos: .userInitiated).async {
      do {
        try self.ensureDocumentDirectory()
        let value = try body()
        resolve(value)
      } catch {
        reject(name, error.localizedDescription, error)
      }
    }
  }

  private func pollDictionary(_ snap: Poll) -> NSDictionary {
    let transactions: [NSDictionary] = snap.transactions.map { tx in
      var dict: [String: Any] = [
        "txid": tx.txid,
        "blockTimeInSeconds": tx.blockTimeInSeconds,
        "minedHeight": tx.minedHeight,
        "value": tx.value,
        "memos": tx.memos,
      ]
      if let fee = tx.fee {
        dict["fee"] = fee
      }
      if let toAddress = tx.toAddress {
        dict["toAddress"] = toAddress
      }
      return dict as NSDictionary
    }
    return [
      "alias": snap.alias,
      "status": snap.status,
      "scanProgress": snap.scanProgress,
      "networkBlockHeight": snap.networkBlockHeight,
      "availableCredits": snap.availableCredits,
      "totalCredits": snap.totalCredits,
      "transactions": transactions,
    ] as NSDictionary
  }

  @objc func initialize(
    _ seed: String, _ account: Int, _ alias: String, _ networkName: String,
    _ defaultHost: String, _ defaultPort: Int,
    resolver resolve: @escaping RCTPromiseResolveBlock,
    rejecter reject: @escaping RCTPromiseRejectBlock
  ) {
    run(resolve, reject, "initialize") {
      try EdgeDashClient.rustInitialize(
        mnemonicSeed: seed,
        account: UInt32(account),
        alias: alias,
        networkName: networkName,
        defaultHost: defaultHost,
        defaultPort: UInt32(defaultPort)
      )
      return nil
    }
  }

  @objc func stop(
    _ alias: String,
    resolver resolve: @escaping RCTPromiseResolveBlock,
    rejecter reject: @escaping RCTPromiseRejectBlock
  ) {
    run(resolve, reject, "stop") {
      try EdgeDashClient.rustStop(alias: alias)
    }
  }

  @objc func startSync(
    _ alias: String,
    resolver resolve: @escaping RCTPromiseResolveBlock,
    rejecter reject: @escaping RCTPromiseRejectBlock
  ) {
    run(resolve, reject, "startSync") {
      try EdgeDashClient.rustStartSync(alias: alias)
      return nil
    }
  }

  @objc func stopSync(
    _ alias: String,
    resolver resolve: @escaping RCTPromiseResolveBlock,
    rejecter reject: @escaping RCTPromiseRejectBlock
  ) {
    run(resolve, reject, "stopSync") {
      try EdgeDashClient.rustStopSync(alias: alias)
      return nil
    }
  }

  @objc func deriveShieldedAddress(
    _ alias: String,
    resolver resolve: @escaping RCTPromiseResolveBlock,
    rejecter reject: @escaping RCTPromiseRejectBlock
  ) {
    run(resolve, reject, "deriveShieldedAddress") {
      let addresses = try EdgeDashClient.rustDeriveShieldedAddress(alias: alias)
      return ["shieldedAddress": addresses.shieldedAddress]
    }
  }

  @objc func deriveShieldedAddressFromSeed(
    _ seed: String, _ network: String, _ account: Int,
    resolver resolve: @escaping RCTPromiseResolveBlock,
    rejecter reject: @escaping RCTPromiseRejectBlock
  ) {
    run(resolve, reject, "deriveShieldedAddressFromSeed") {
      try EdgeDashClient.rustDeriveShieldedAddressFromSeed(
        mnemonicSeed: seed, network: network, account: UInt32(account))
    }
  }

  @objc func coreReceiveAddress(
    _ alias: String, _ account: Int,
    resolver resolve: @escaping RCTPromiseResolveBlock,
    rejecter reject: @escaping RCTPromiseRejectBlock
  ) {
    run(resolve, reject, "coreReceiveAddress") {
      try EdgeDashClient.rustCoreReceiveAddress(alias: alias, account: UInt32(account))
    }
  }

  @objc func startCoreSync(
    _ alias: String, _ fromHeight: Int,
    resolver resolve: @escaping RCTPromiseResolveBlock,
    rejecter reject: @escaping RCTPromiseRejectBlock
  ) {
    run(resolve, reject, "startCoreSync") {
      try EdgeDashClient.rustStartCoreSync(alias: alias, fromHeight: UInt32(fromHeight))
      return nil
    }
  }

  @objc func coreBalance(
    _ alias: String,
    resolver resolve: @escaping RCTPromiseResolveBlock,
    rejecter reject: @escaping RCTPromiseRejectBlock
  ) {
    run(resolve, reject, "coreBalance") {
      let balance = try EdgeDashClient.rustCoreBalance(alias: alias)
      return [
        "confirmedDuffs": balance.confirmedDuffs,
        "unconfirmedDuffs": balance.unconfirmedDuffs,
        "totalDuffs": balance.totalDuffs,
        "syncedHeight": balance.syncedHeight,
      ]
    }
  }

  @objc func shieldFromAssetLock(
    _ alias: String, _ amountDuffs: String, _ accountIndex: Int, _ mnemonicSeed: String,
    resolver resolve: @escaping RCTPromiseResolveBlock,
    rejecter reject: @escaping RCTPromiseRejectBlock
  ) {
    run(resolve, reject, "shieldFromAssetLock") {
      try EdgeDashClient.rustShieldFromAssetLock(
        alias: alias, amountDuffs: amountDuffs, accountIndex: UInt32(accountIndex),
        mnemonicSeed: mnemonicSeed)
    }
  }

  @objc func platformReceiveAddress(
    _ alias: String, _ account: Int,
    resolver resolve: @escaping RCTPromiseResolveBlock,
    rejecter reject: @escaping RCTPromiseRejectBlock
  ) {
    run(resolve, reject, "platformReceiveAddress") {
      try EdgeDashClient.rustPlatformReceiveAddress(alias: alias, account: UInt32(account))
    }
  }

  @objc func trackedAssetLocks(
    _ alias: String,
    resolver resolve: @escaping RCTPromiseResolveBlock,
    rejecter reject: @escaping RCTPromiseRejectBlock
  ) {
    run(resolve, reject, "trackedAssetLocks") {
      try EdgeDashClient.rustTrackedAssetLocks(alias: alias).map { lock in
        [
          "txid": lock.txid,
          "vout": lock.vout,
          "status": lock.status,
        ] as NSDictionary
      }
    }
  }

  @objc func resumeShieldFromAssetLock(
    _ alias: String, _ txid: String, _ vout: Int, _ mnemonicSeed: String,
    resolver resolve: @escaping RCTPromiseResolveBlock,
    rejecter reject: @escaping RCTPromiseRejectBlock
  ) {
    run(resolve, reject, "resumeShieldFromAssetLock") {
      try EdgeDashClient.rustResumeShieldFromAssetLock(
        alias: alias, txid: txid, vout: UInt32(vout), mnemonicSeed: mnemonicSeed)
    }
  }

  @objc func unshield(
    _ alias: String, _ toAddress: String, _ amountCredits: String, _ mnemonicSeed: String,
    resolver resolve: @escaping RCTPromiseResolveBlock,
    rejecter reject: @escaping RCTPromiseRejectBlock
  ) {
    run(resolve, reject, "unshield") {
      try EdgeDashClient.rustUnshield(
        alias: alias, toAddress: toAddress, amountCredits: amountCredits,
        mnemonicSeed: mnemonicSeed)
    }
  }

  @objc func shieldedWithdraw(
    _ alias: String, _ toCoreAddress: String, _ amountCredits: String,
    _ coreFeePerByte: Int, _ mnemonicSeed: String,
    resolver resolve: @escaping RCTPromiseResolveBlock,
    rejecter reject: @escaping RCTPromiseRejectBlock
  ) {
    run(resolve, reject, "shieldedWithdraw") {
      try EdgeDashClient.rustShieldedWithdraw(
        alias: alias, toCoreAddress: toCoreAddress, amountCredits: amountCredits,
        coreFeePerByte: UInt32(coreFeePerByte), mnemonicSeed: mnemonicSeed)
    }
  }

  @objc func isValidAddress(
    _ address: String, _ network: String,
    resolver resolve: @escaping RCTPromiseResolveBlock,
    rejecter reject: @escaping RCTPromiseRejectBlock
  ) {
    run(resolve, reject, "isValidAddress") {
      EdgeDashClient.rustIsValidAddress(address: address, network: network)
    }
  }

  @objc func deriveViewingKey(
    _ seed: String, _ network: String,
    resolver resolve: @escaping RCTPromiseResolveBlock,
    rejecter reject: @escaping RCTPromiseRejectBlock
  ) {
    run(resolve, reject, "deriveViewingKey") {
      try EdgeDashClient.rustDeriveViewingKey(mnemonicSeed: seed, network: network)
    }
  }

  @objc func warmUpProver(
    _ resolve: @escaping RCTPromiseResolveBlock,
    rejecter reject: @escaping RCTPromiseRejectBlock
  ) {
    run(resolve, reject, "warmUpProver") {
      try EdgeDashClient.rustWarmUpProver()
      return nil
    }
  }

  @objc func isProverReady(
    _ resolve: @escaping RCTPromiseResolveBlock,
    rejecter reject: @escaping RCTPromiseRejectBlock
  ) {
    run(resolve, reject, "isProverReady") {
      EdgeDashClient.rustIsProverReady()
    }
  }

  @objc func poll(
    _ alias: String,
    resolver resolve: @escaping RCTPromiseResolveBlock,
    rejecter reject: @escaping RCTPromiseRejectBlock
  ) {
    run(resolve, reject, "poll") {
      try self.pollDictionary(EdgeDashClient.rustPoll(alias: alias))
    }
  }

  @objc func proposeTransfer(
    _ alias: String, _ amountCredits: String, _ toAddress: String, _ memo: String,
    resolver resolve: @escaping RCTPromiseResolveBlock,
    rejecter reject: @escaping RCTPromiseRejectBlock
  ) {
    run(resolve, reject, "proposeTransfer") {
      try EdgeDashClient.rustProposeTransfer(
        alias: alias,
        amountCredits: amountCredits,
        toAddress: toAddress,
        memo: memo.isEmpty ? nil : memo
      )
    }
  }

  @objc func createTransfer(
    _ alias: String, _ proposalId: String, _ seed: String,
    resolver resolve: @escaping RCTPromiseResolveBlock,
    rejecter reject: @escaping RCTPromiseRejectBlock
  ) {
    run(resolve, reject, "createTransfer") {
      try EdgeDashClient.rustCreateTransfer(
        alias: alias, proposalId: proposalId, mnemonicSeed: seed)
    }
  }
}
