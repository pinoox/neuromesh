; NeuroMesh extract profile — tree-sitter-java
(method_declaration
  name: (identifier) @function.name) @function

(constructor_declaration
  name: (identifier) @function.name) @function

(class_declaration
  name: (identifier) @class.name) @class

(interface_declaration
  name: (identifier) @class.name) @class

(enum_declaration
  name: (identifier) @class.name) @class

(import_declaration) @import

(method_invocation) @call

(object_creation_expression) @call
