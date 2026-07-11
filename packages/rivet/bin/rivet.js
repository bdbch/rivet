#!/usr/bin/env node
// rivet CLI — delegates to the native NAPI-RS addon

const native = require('../index.js')
const args = process.argv.slice(2)

try {
  native.runCli(args)
} catch (e) {
  if (e.message) {
    console.error('Error:', e.message)
  }
  process.exit(1)
}
