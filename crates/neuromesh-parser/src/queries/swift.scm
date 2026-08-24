; NeuroMesh extract profile — tree-sitter-swift
(function_declaration
  name: (simple_identifier) @function.name) @function

(init_declaration
  "init" @function.name) @function

(class_declaration
  name: (type_identifier) @class.name) @class

(protocol_declaration
  name: (type_identifier) @class.name) @class

(import_declaration) @import

(call_expression) @call
