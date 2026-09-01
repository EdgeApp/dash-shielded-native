import { spawn } from 'child_process'
import { copyFile, mkdir, readdir, readFile, writeFile } from 'fs/promises'
import { join } from 'path'

const rustDir = join(__dirname, '../rust')
const androidJni = join(__dirname, '../android/src/main/jniLibs')
const androidJava = join(__dirname, '../android/src/main/java')

const NDK_VERSION = '27.1.12297006'

const cargoEnv = {
  ...process.env,
  CARGO_NET_GIT_FETCH_WITH_CLI: 'true'
}

function run(
  cmd: string,
  args: string[],
  cwd: string,
  extraEnv?: NodeJS.ProcessEnv
): Promise<void> {
  return new Promise((resolve, reject) => {
    const child = spawn(cmd, args, {
      cwd,
      stdio: 'inherit',
      env: { ...cargoEnv, ...extraEnv }
    })
    child.on('error', reject)
    child.on('exit', code => {
      if (code === 0) resolve()
      else reject(new Error(`${cmd} ${args.join(' ')} exited ${code}`))
    })
  })
}

/**
 * UniFFI names the error payload `message`, which collides with
 * Throwable.message on Kotlin 2.x. Rename the stored field where the
 * generator emitted it. No-op if a future UniFFI stops colliding.
 */
async function patchKotlinErrorField(path: string): Promise<void> {
  const source = await readFile(path, 'utf8')
  const patched = source
    .replace(
      'val `message`: kotlin.String\n        ) : DashException() {\n        override val message\n            get() = "message=${ `message` }"',
      'val errorMessage: kotlin.String\n        ) : DashException() {\n        override val message\n            get() = errorMessage'
    )
    .replace(
      'FfiConverterString.allocationSize(value.`message`)',
      'FfiConverterString.allocationSize(value.errorMessage)'
    )
    .replace(
      'FfiConverterString.write(value.`message`, buf)',
      'FfiConverterString.write(value.errorMessage, buf)'
    )
  if (patched !== source) {
    await writeFile(path, patched)
    console.log('Patched Kotlin error field collision')
  }
}

/**
 * Locates the NDK's host toolchain directory.
 *
 * The NDK ships one `prebuilt/<host-tag>` directory per host OS. On macOS that
 * tag is literally `darwin-x86_64` even on Apple Silicon — Google never renamed
 * it, and the binaries inside are universal, so clang runs natively as arm64 —
 * but hardcoding it is wrong on Linux and misleading everywhere. Read whatever
 * the installed NDK actually ships instead.
 */
async function findToolchainBin(ndk: string): Promise<string> {
  const prebuilt = join(ndk, 'toolchains/llvm/prebuilt')
  const hosts = (await readdir(prebuilt, { withFileTypes: true }))
    .filter(entry => entry.isDirectory())
    .map(entry => entry.name)

  if (hosts.length === 0) {
    throw new Error(`No host toolchain under ${prebuilt}`)
  }
  // A given NDK install ships exactly one host toolchain. If that ever stops
  // being true, prefer the one matching this platform over an arbitrary pick.
  const prefix = { darwin: 'darwin', linux: 'linux', win32: 'windows' }[
    process.platform as 'darwin' | 'linux' | 'win32'
  ]
  const host =
    hosts.find(name => prefix != null && name.startsWith(prefix)) ?? hosts[0]

  return join(prebuilt, host, 'bin')
}

async function main(): Promise<void> {
  const { ANDROID_HOME } = process.env
  if (ANDROID_HOME == null) {
    throw new Error('ANDROID_HOME is not set in the environment.')
  }
  const sdk = ANDROID_HOME
  const ndk = join(sdk, 'ndk', NDK_VERSION)

  // Kotlin bindings for the `dash` namespace in rust/src/dash.udl:
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
      'kotlin',
      '--out-dir',
      androidJava
    ],
    rustDir
  )

  await patchKotlinErrorField(join(androidJava, 'uniffi/dash/dash.kt'))

  // arm64-v8a only. RNDashShieldedModule.kt sets
  // `uniffi.component.dash.libraryOverride` to "dashshielded", so the ABI
  // directory needs exactly libdashshielded.so — no libuniffi_dash.so copy.
  const abi = 'arm64-v8a'
  const target = 'aarch64-linux-android'
  const toolchainBin = await findToolchainBin(ndk)
  await run(
    'cargo',
    [
      'ndk',
      '-t',
      abi,
      'build',
      '--release',
      '--no-default-features',
      '--features',
      'uniffi-backend'
    ],
    rustDir,
    {
      ANDROID_NDK_HOME: ndk,
      // Android 15 ships 16 KB pages; link for it or the loader rejects us.
      CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS:
        '-C link-arg=-Wl,-z,max-page-size=16384 -C link-arg=-Wl,-z,common-page-size=16384',
      // Dependencies that compile C (rs-x11-hash) go through the `cc` crate.
      // cargo-ndk points it at a bare `clang` and expects the NDK toolchain
      // to be first on PATH; without that it resolves to host clang, which
      // has no Android sysroot and dies on a missing `stdlib.h`. Setting
      // CC_<target> here does not help — cargo-ndk overwrites it.
      PATH: `${toolchainBin}:${process.env.PATH ?? ''}`
    }
  )

  const destDir = join(androidJni, abi)
  await mkdir(destDir, { recursive: true })
  await copyFile(
    join(rustDir, 'target', target, 'release', 'libdashshielded.so'),
    join(destDir, 'libdashshielded.so')
  )
  console.log(`Wrote ${join(destDir, 'libdashshielded.so')}`)
}

main().catch((error: unknown) => {
  console.error(error)
  process.exit(1)
})
