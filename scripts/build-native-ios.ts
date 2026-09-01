import { spawn } from 'child_process'
import { copyFile, mkdir, writeFile } from 'fs/promises'
import { join } from 'path'

const rustDir = join(__dirname, '../rust')
const iosDir = join(__dirname, '../ios')
const generated = join(rustDir, 'Generated')

const cargoEnv = {
  ...process.env,
  CARGO_NET_GIT_FETCH_WITH_CLI: 'true',
  // Match the app's deployment target; building newer makes ld warn on every
  // object file.
  IPHONEOS_DEPLOYMENT_TARGET: '15.6'
}

function run(cmd: string, args: string[], cwd: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const child = spawn(cmd, args, { cwd, stdio: 'inherit', env: cargoEnv })
    child.on('error', reject)
    child.on('exit', code => {
      if (code === 0) resolve()
      else reject(new Error(`${cmd} ${args.join(' ')} exited ${code}`))
    })
  })
}

async function main(): Promise<void> {
  await mkdir(generated, { recursive: true })

  // Swift bindings for the `dash` namespace in rust/src/dash.udl:
  await run(
    'cargo',
    [
      'run',
      '--release',
      '--no-default-features',
      '--features',
      'uniffi-backend',
      '--bin',
      'uniffi-bindgen',
      'generate',
      'src/dash.udl',
      '--language',
      'swift',
      '--out-dir',
      generated
    ],
    rustDir
  )

  // Device and Apple-Silicon simulator slices. The x86_64 simulator is not
  // built: every machine that runs this is arm64, and the extra slice doubles
  // the link time for a simulator nobody here boots.
  const targets = ['aarch64-apple-ios', 'aarch64-apple-ios-sim']
  for (const target of targets) {
    await run(
      'cargo',
      [
        'build',
        '--release',
        '--target',
        target,
        '--no-default-features',
        '--features',
        'uniffi-backend'
      ],
      rustDir
    )
  }

  await copyFile(join(generated, 'dash.swift'), join(iosDir, 'dash.swift'))

  // Each slice needs its own headers directory: xcodebuild rejects two
  // libraries that share one. Ship only module.modulemap — also shipping
  // dashFFI.modulemap makes -create-xcframework emit both, and Clang then
  // reports a dashFFI redefinition.
  const deviceHeaders = join(generated, 'ios-device')
  const simHeaders = join(generated, 'ios-sim')
  for (const dest of [deviceHeaders, simHeaders]) {
    await mkdir(dest, { recursive: true })
    await copyFile(join(generated, 'dashFFI.h'), join(dest, 'dashFFI.h'))
    await copyFile(
      join(generated, 'dashFFI.modulemap'),
      join(dest, 'module.modulemap')
    )
  }

  const xc = join(iosDir, 'libdashshielded.xcframework')
  await run('rm', ['-rf', xc], iosDir)
  await run(
    'xcodebuild',
    [
      '-create-xcframework',
      '-library',
      join(rustDir, 'target', 'aarch64-apple-ios', 'release', 'libdashshielded.a'),
      '-headers',
      deviceHeaders,
      '-library',
      join(
        rustDir,
        'target',
        'aarch64-apple-ios-sim',
        'release',
        'libdashshielded.a'
      ),
      '-headers',
      simHeaders,
      '-output',
      xc
    ],
    iosDir
  )

  await writeFile(
    join(iosDir, '.uniffi-generated'),
    'dash.swift dashFFI.h from rust/src/dash.udl\n'
  )
  console.log(`Wrote ${xc}`)
}

main().catch((error: unknown) => {
  console.error(error)
  process.exit(1)
})
