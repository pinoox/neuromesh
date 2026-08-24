; NeuroMesh extract profile — tree-sitter-c-sharp
(method_declaration
  name: (identifier) @function.name) @function

(constructor_declaration
  name: (identifier) @function.name) @function

(class_declaration
  name: (identifier) @class.name) @class

(interface_declaration
  name: (identifier) @class.name) @class

(struct_declaration
  name: (identifier) @class.name) @class

(using_directive) @import

(invocation_expression) @call

(object_creation_expression) @call
