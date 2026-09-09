// Walk a testnet wallet from empty to a funded shielded pool.
//
// Value can only reach the Orchard pool through Dash L1: a transparent UTXO
// becomes an asset lock, and the asset lock becomes a shielded note. This
// script drives that whole path so the shielded send path has something to
// spend.
//
//   DASH_MNEMONIC="<24 words>" node -r sucrase/register ./scripts/fund-testnet.ts
//
// It prints the wallet's L1 receive address and waits. Pay that address from
// https://faucet.testnet.networks.dash.org/ while it runs; once the balance
// lands it shields `SHIELD_DUFFS` (default 0.1 DASH) into the pool and reports
// the shielded balance.
//
// Environment:
//   DASH_MNEMONIC   BIP-39 phrase for the wallet (required)
//   DASH_DATA_DIR   where wallet + SPV state live (default: a temp dir)
//   START_HEIGHT    L1 height to start the SPV scan from (default: 1549000)
//   SHIELD_DUFFS    amount to move into the pool (default: 10000000)
//   WAIT_MINUTES    how long to wait for the faucet payment (default: 30)

import { mkdtempSync } from 'fs'
import { tmpdir } from 'os'
import { join } from 'path'

import { makeNodeDashShieldedModule } from '../src/node'

const ALIAS = 'fund'
const NETWORK = 'testnet'

const sleep = async (ms: number): Promise<void> =>
  await new Promise(resolve => setTimeout(resolve, ms))

async function main(): Promise<void> {
  const mnemonicSeed = process.env.DASH_MNEMONIC ?? ''
  if (mnemonicSeed === '') {
    throw new Error('set DASH_MNEMONIC to the wallet phrase')
  }
  const documentDirectory =
    process.env.DASH_DATA_DIR ?? mkdtempSync(join(tmpdir(), 'dash-fund-'))
  const startHeight = Number(process.env.START_HEIGHT ?? 1549000)
  const shieldDuffs = process.env.SHIELD_DUFFS ?? '10000000'
  const waitMinutes = Number(process.env.WAIT_MINUTES ?? 30)

  const io = makeNodeDashShieldedModule({ documentDirectory })
  const synchronizer = await io.makeSynchronizer({
    mnemonicSeed,
    account: 0,
    alias: ALIAS,
    network: NETWORK,
    dataDir: documentDirectory,
    defaultHost: 'seed-1.testnet.networks.dash.org',
    defaultPort: 1443
  })

  const { shieldedAddress } = await synchronizer.deriveShieldedAddress()
  const coreAddress = await synchronizer.coreReceiveAddress()
  console.log(`data dir:        ${documentDirectory}`)
  console.log(`shielded address ${shieldedAddress}`)
  console.log(`fund this L1 address: ${coreAddress}`)

  // The Platform SDK connection cannot see L1, so the transparent balance
  // needs its own SPV sync before an asset lock has anything to spend.
  await synchronizer.startCoreSync(startHeight)

  const deadline = Date.now() + waitMinutes * 60_000
  let funded = false
  while (Date.now() < deadline) {
    const core = await synchronizer.coreBalance()
    console.log(
      `L1 height ${core.syncedHeight} confirmed ${core.confirmedDuffs} unconfirmed ${core.unconfirmedDuffs}`
    )
    if (BigInt(core.confirmedDuffs) >= BigInt(shieldDuffs)) {
      funded = true
      break
    }
    await sleep(15_000)
  }
  if (!funded) {
    throw new Error(
      `no confirmed L1 balance of ${shieldDuffs} duffs after ${String(
        waitMinutes
      )} minutes`
    )
  }

  // Building the Halo 2 key first keeps its cost out of the shield timing.
  await io.Tools.warmUpProver()

  console.log(`shielding ${shieldDuffs} duffs`)
  const started = Date.now()
  const recipient = await synchronizer.shieldFromAssetLock(shieldDuffs)
  console.log(`shielded to ${recipient} in ${String(Date.now() - started)} ms`)

  await synchronizer.startSync()
  for (let i = 0; i < 40; i++) {
    await sleep(15_000)
    const balance = await synchronizer.getBalance()
    console.log(`shielded balance ${balance.totalCredits} credits`)
    if (balance.totalCredits !== '0') break
  }

  await synchronizer.stop()
}

main().catch((error: unknown) => {
  console.error(error)
  process.exit(1)
})
