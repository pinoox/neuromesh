; NeuroMesh extract profile — tree-sitter-typescript
(function_declaration
  name: (identifier) @function.name) @function

(method_definition
  name: (_) @function.name) @function

(class_declaration
  name: (_) @class.name) @class

(interface_declaration
  name: (_) @symbol.name) @symbol

(type_alias_declaration
  name: (_) @symbol.name) @symbol

(import_statement) @import

(call_expression) @call
