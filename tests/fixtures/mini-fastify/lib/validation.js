'use strict'

const Ajv = require('ajv')

function validate (schema, data) {
  const ajv = new Ajv()
  return ajv.validate(schema, data)
}

module.exports = { validate }
