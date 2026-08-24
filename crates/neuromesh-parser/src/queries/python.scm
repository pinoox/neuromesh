; NeuroMesh extract profile — tree-sitter-python
(function_definition
  name: (identifier) @function.name) @function

(class_definition
  name: (identifier) @class.name) @class

(import_statement) @import

(import_from_statement) @import

(call) @call
