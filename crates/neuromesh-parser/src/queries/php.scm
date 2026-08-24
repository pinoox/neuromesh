; NeuroMesh extract profile — tree-sitter-php
(function_definition
  name: (name) @function.name) @function

(method_declaration
  name: (name) @function.name) @function

(class_declaration
  name: (name) @class.name) @class

(interface_declaration
  name: (name) @class.name) @class

(namespace_use_declaration) @import

(function_call_expression) @call

(member_call_expression) @call

(scoped_call_expression) @call

(object_creation_expression) @call
