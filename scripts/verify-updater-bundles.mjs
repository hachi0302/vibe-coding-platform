#!/usr/bin/env node

import { createHash, createPublicKey, verify } from 'node:crypto'
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, resolve } from 'node:path'

const UPDATE_BUNDLE_SUFFIXES = ['.app.tar.gz', '-setup.exe', '.AppImage']
const SPKI_ED25519_PREFIX = Buffer.from('302a300506032b6570032100', 'hex')
const TRUSTED_COMMENT_PREFIX = 'trusted comment: '

function option(name) {
  const index = process.argv.indexOf(name)
  if (index === -1 || !process.argv[index + 1]) {
    throw new Error(`Missing required option: ${name}`)
  }
  return resolve(process.argv[index + 1])
}

function walk(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const fullPath = join(directory, entry.name)
    return entry.isDirectory() ? walk(fullPath) : [fullPath]
  })
}

function loadPublicKey(configPath) {
  const config = JSON.parse(readFileSync(configPath, 'utf8'))
  const encoded = config?.plugins?.updater?.pubkey
  if (typeof encoded !== 'string' || !encoded) {
    throw new Error(`Updater public key is missing from ${configPath}`)
  }

  const lines = Buffer.from(encoded, 'base64').toString('utf8').trim().split(/\r?\n/)
  if (lines.length < 2) {
    throw new Error('Updater public key has an invalid Minisign format')
  }

  const raw = Buffer.from(lines[1], 'base64')
  if (raw.length !== 42 || raw.subarray(0, 2).toString('ascii') !== 'Ed') {
    throw new Error('Updater public key is not an Ed25519 Minisign key')
  }

  return {
    keyId: raw.subarray(2, 10),
    key: createPublicKey({
      key: Buffer.concat([SPKI_ED25519_PREFIX, raw.subarray(10)]),
      format: 'der',
      type: 'spki',
    }),
  }
}

function loadSignature(signaturePath) {
  const decoded = Buffer.from(readFileSync(signaturePath, 'utf8').trim(), 'base64').toString('utf8')
  const lines = decoded.trim().split(/\r?\n/)
  if (lines.length !== 4 || !lines[2].startsWith(TRUSTED_COMMENT_PREFIX)) {
    throw new Error(`${signaturePath} is not a supported Tauri/Minisign signature`)
  }

  const primary = Buffer.from(lines[1], 'base64')
  const global = Buffer.from(lines[3], 'base64')
  if (primary.length !== 74 || global.length !== 64) {
    throw new Error(`${signaturePath} has an invalid Minisign signature length`)
  }

  return {
    algorithm: primary.subarray(0, 2).toString('ascii'),
    keyId: primary.subarray(2, 10),
    signature: primary.subarray(10),
    trustedComment: lines[2].slice(TRUSTED_COMMENT_PREFIX.length),
    global,
  }
}

function verifyBundle(bundlePath, publicKey) {
  const signaturePath = `${bundlePath}.sig`
  const signature = loadSignature(signaturePath)
  if (!signature.keyId.equals(publicKey.keyId)) {
    throw new Error(`${bundlePath}: signature key does not match the configured updater public key`)
  }

  const contents = readFileSync(bundlePath)
  const payload = signature.algorithm === 'ED'
    ? createHash('blake2b512').update(contents).digest()
    : signature.algorithm === 'Ed'
      ? contents
      : null
  if (!payload) {
    throw new Error(`${bundlePath}: unsupported Minisign algorithm ${signature.algorithm}`)
  }
  if (!verify(null, payload, publicKey.key, signature.signature)) {
    throw new Error(`${bundlePath}: content signature verification failed`)
  }

  const globalPayload = Buffer.concat([signature.signature, Buffer.from(signature.trustedComment)])
  if (!verify(null, globalPayload, publicKey.key, signature.global)) {
    throw new Error(`${bundlePath}: trusted-comment signature verification failed`)
  }
}

try {
  const root = option('--root')
  const config = option('--config')
  if (!statSync(root).isDirectory()) {
    throw new Error(`Bundle root is not a directory: ${root}`)
  }

  const bundles = walk(root).filter((path) => UPDATE_BUNDLE_SUFFIXES.some((suffix) => path.endsWith(suffix)))
  if (!bundles.length) {
    throw new Error(`No updater bundles found under ${root}`)
  }

  const publicKey = loadPublicKey(config)
  bundles.forEach((bundle) => verifyBundle(bundle, publicKey))
  console.log(`Verified ${bundles.length} updater bundle signature(s).`)
} catch (error) {
  console.error(`Updater bundle verification failed: ${error.message}`)
  process.exitCode = 1
}
