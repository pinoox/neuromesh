; NeuroMesh extract profile — tree-sitter-kotlin
(function_declaration
  (simple_identifier) @function.name) @function

(class_declaration
  (type_identifier) @class.name) @class

(object_declaration
  (type_identifier) @class.name) @class

(import_header) @import

(call_expression) @call

(constructor_invocation) @call
