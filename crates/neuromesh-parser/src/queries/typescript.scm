; NeuroMesh extract profile — tree-sitter-typescript
(function_declaration
  name: (identifier) @function.name) @function

(method_definition
  name: (_) @function.name) @function

(variable_declarator
  name: (identifier) @function.name
  value: [(arrow_function) (function_expression)]) @function

(class_declaration
  name: (_) @class.name) @class

(interface_declaration
  name: (_) @symbol.name) @symbol

(type_alias_declaration
  name: (_) @symbol.name) @symbol

(enum_declaration
  name: (_) @symbol.name) @symbol

(import_statement) @import

(call_expression) @call

; `export const MAX_ATTEMPTS = 5`, `export const MAINTENANCE_SECTION_RULES = {...}`:
; a module-level SCREAMING_SNAKE constant is a symbol a question points at by
; name. Only exported and only that spelling — a local `const x` is not,
; and an arrow function keeps its @function capture above.
(export_statement
  (lexical_declaration
    (variable_declarator
      name: (identifier) @symbol.name
      (#match? @symbol.name "^[A-Z][A-Z0-9]*(_[A-Z0-9]+)+$")))) @symbol

; `export const userNameSchema = z.object({...})`, `export const db = new
; PrismaClient()`: an exported module-level object built by a call is what
; `userNameSchema.parse(` and `db.user.update(` are member calls on. Without
; a node the file has nothing to bind those calls to. Literals
; (`export const revalidate = 60`) stay out.
(export_statement
  (lexical_declaration
    (variable_declarator
      name: (identifier) @symbol.name
      value: [(call_expression) (new_expression) (object) (binary_expression) (await_expression)]))) @symbol

; `fastify.decorate('knex', knex(opts))`, `app.decorateRequest('user', null)`:
; a decoration puts a name on the instance that other files use bare
; (`const knex = fastify.knex; knex('users')`) with nothing in scope to
; resolve it. The decorated name is a symbol of the file that decorates,
; so the bare call and the member read bind there.
(call_expression
  function: (member_expression
    property: (property_identifier) @_decorate)
  arguments: (arguments . (string (string_fragment) @symbol.name))
  (#match? @_decorate "^decorate(Request|Reply)?$")) @symbol
