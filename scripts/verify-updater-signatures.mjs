import crypto from 'node:crypto'
import fs from 'node:fs'
import path from 'node:path'
import process from 'node:process'
import { fileURLToPath } from 'node:url'

const [, , configPath, manifestPath, assetsRoot] = process.argv

if (!configPath || !manifestPath || !assetsRoot) {
  console.error(
    'Usage: node scripts/verify-updater-signatures.mjs <tauri.conf.json> <latest.json> <assets-directory>',
  )
  process.exit(2)
}

const config = JSON.parse(fs.readFileSync(configPath, 'utf8'))
const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'))
const encodedPublicKey = config?.plugins?.updater?.pubkey

if (typeof encodedPublicKey !== 'string' || !encodedPublicKey) {
  throw new Error('Tauri updater public key is missing')
}

const publicKeyText = Buffer.from(encodedPublicKey, 'base64').toString('utf8')
const publicKeyLine = publicKeyText.trim().split(/\r?\n/).at(-1)
const publicKeyRecord = Buffer.from(publicKeyLine, 'base64')

if (
  publicKeyRecord.length !== 42 ||
  !['Ed', 'ED'].includes(publicKeyRecord.subarray(0, 2).toString('ascii'))
) {
  throw new Error('Tauri updater public key is not a valid Minisign Ed25519 public key')
}

const publicKeyId = publicKeyRecord.subarray(2, 10)
const publicKey = crypto.createPublicKey({
  key: Buffer.concat([
    Buffer.from('302a300506032b6570032100', 'hex'),
    publicKeyRecord.subarray(10),
  ]),
  format: 'der',
  type: 'spki',
})

function findAsset(fileName) {
  const matches = []

  function walk(directory) {
    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
      const fullPath = path.join(directory, entry.name)
      if (entry.isDirectory()) walk(fullPath)
      else if (entry.name === fileName) matches.push(fullPath)
    }
  }

  walk(assetsRoot)
  if (matches.length !== 1) {
    throw new Error(`Expected one staged updater asset named ${fileName}, found ${matches.length}`)
  }
  return matches[0]
}

function decodeSignature(encodedSignature) {
  if (typeof encodedSignature !== 'string' || !encodedSignature) {
    throw new Error('Updater signature is missing')
  }

  const text = Buffer.from(encodedSignature, 'base64').toString('utf8')
  const lines = text.trim().split(/\r?\n/)
  if (lines.length !== 4 || !lines[2].startsWith('trusted comment: ')) {
    throw new Error('Updater signature is not a valid Minisign signature')
  }

  const signatureRecord = Buffer.from(lines[1], 'base64')
  const globalSignature = Buffer.from(lines[3], 'base64')
  if (signatureRecord.length !== 74 || globalSignature.length !== 64) {
    throw new Error('Updater signature has invalid record lengths')
  }

  const algorithm = signatureRecord.subarray(0, 2).toString('ascii')
  if (!['Ed', 'ED'].includes(algorithm)) {
    throw new Error(`Unsupported Minisign signature algorithm: ${algorithm}`)
  }

  return {
    algorithm,
    keyId: signatureRecord.subarray(2, 10),
    signature: signatureRecord.subarray(10),
    trustedComment: lines[2].slice('trusted comment: '.length),
    globalSignature,
  }
}

function verifyAsset(assetPath, encodedSignature) {
  const signature = decodeSignature(encodedSignature)
  if (!crypto.timingSafeEqual(publicKeyId, signature.keyId)) {
    throw new Error(`${path.basename(assetPath)}: signature key ID differs from updater public key`)
  }

  const asset = fs.readFileSync(assetPath)
  const signedPayload =
    signature.algorithm === 'ED' ? crypto.createHash('blake2b512').update(asset).digest() : asset
  if (!crypto.verify(null, signedPayload, publicKey, signature.signature)) {
    throw new Error(`${path.basename(assetPath)}: signature verification failed`)
  }

  const globalPayload = Buffer.concat([
    signature.signature,
    Buffer.from(signature.trustedComment, 'utf8'),
  ])
  if (!crypto.verify(null, globalPayload, publicKey, signature.globalSignature)) {
    throw new Error(`${path.basename(assetPath)}: trusted comment verification failed`)
  }
}

const platforms = Object.entries(manifest.platforms ?? {})
if (!platforms.length) throw new Error('Updater manifest contains no platforms')

const verified = new Set()
for (const [platform, update] of platforms) {
  const fileName = path.basename(new URL(update.url).pathname)
  const assetPath = findAsset(fileName)
  const verificationKey = `${assetPath}\0${update.signature}`
  if (!verified.has(verificationKey)) {
    verifyAsset(assetPath, update.signature)
    verified.add(verificationKey)
  }
  console.log(`${platform}: Minisign verification passed (${fileName})`)
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  console.log(`Verified ${verified.size} signed updater asset(s) with the configured public key`)
}
