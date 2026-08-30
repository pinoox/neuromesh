'use strict'

function encapsulate (name, fn) {
  return async function plugin (fastify, opts) {
    return fn(fastify, opts)
  }
}

module.exports = { encapsulate }
