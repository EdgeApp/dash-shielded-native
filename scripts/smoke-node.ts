import { mkdtempSync } from 'fs'
import { tmpdir } from 'os'
import { join } from 'path'

import { makeNodeDashShieldedModule } from '../src/node'

async function main(): Promise<void> {
  const documentDirectory = mkdtempSync(join(tmpdir(), 'dashshielded-smoke-'))
  const io = makeNodeDashShieldedModule({ documentDirectory })

  const mnemonic =
    'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art'

  const validDummy = await io.Tools.isValidAddress('not-an-address', 'testnet')
  if (validDummy) {
    throw new Error('isValidAddress should reject garbage')
  }

  const viewing = await io.Tools.deriveViewingKey(mnemonic, 'testnet')
  if (viewing.fullViewingKey.length < 64) {
    throw new Error(`unexpected viewing key ${viewing.fullViewingKey}`)
  }

  const address = await io.Tools.deriveShieldedAddress(mnemonic, 'testnet', 0)
  if (!address.startsWith('tdash1z') && !address.startsWith('tdash1')) {
    throw new Error(`unexpected testnet address ${address}`)
  }
  const addressOk = await io.Tools.isValidAddress(address, 'testnet')
  if (!addressOk) {
    throw new Error(`derived address failed validation: ${address}`)
  }

  const synchronizer = await io.makeSynchronizer({
    mnemonicSeed: mnemonic,
    account: 0,
    alias: 'smoke',
    network: 'testnet',
    dataDir: documentDirectory,
    defaultHost: 'seed-1.testnet.networks.dash.org',
    defaultPort: 1443
  })
  const derived = await synchronizer.deriveShieldedAddress()
  if (derived.shieldedAddress !== address) {
    throw new Error(`address mismatch ${derived.shieldedAddress} vs ${address}`)
  }
  const balance = await synchronizer.getBalance()
  if (balance.totalCredits !== '0') {
    throw new Error(`expected empty balance, got ${balance.totalCredits}`)
  }

  // Building the Halo 2 proving key is the heaviest thing this module does and
  // the only part a spend cannot skip, so the smoke test proves it completes
  // rather than leaving it to be discovered on a user's first send.
  if (await io.Tools.isProverReady()) {
    throw new Error('prover reported ready before warm-up')
  }
  const proverStart = Date.now()
  await io.Tools.warmUpProver()
  const proverMs = Date.now() - proverStart
  if (!(await io.Tools.isProverReady())) {
    throw new Error('prover did not become ready after warm-up')
  }

  console.log('smoke-node ok')
  console.log(JSON.stringify({ address, viewing, proverMs }, null, 2))
  await synchronizer.stop()
}

main().catch((error: unknown) => {
  console.error(error)
  process.exit(1)
})
