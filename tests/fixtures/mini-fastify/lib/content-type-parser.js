'use strict'

function addContentTypeParser (fastify, contentType, parser) {
  fastify.addContentTypeParser(contentType, parser)
}

module.exports = { addContentTypeParser }
