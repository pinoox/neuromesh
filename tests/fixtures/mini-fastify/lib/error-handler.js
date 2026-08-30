'use strict'

function setErrorHandler (app, handler) {
  app.setErrorHandler(handler)
}

function serializeError (err) {
  return {
    statusCode: err.statusCode || 500,
    message: err.message
  }
}

module.exports = { setErrorHandler, serializeError }
