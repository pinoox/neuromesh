; NeuroMesh extract profile — tree-sitter-go
(function_declaration
  name: (identifier) @function.name) @function

(method_declaration
  receiver: (parameter_list
    (parameter_declaration
      type: (_) @function.parent))
  name: (field_identifier) @function.name) @function

(type_spec
  name: (type_identifier) @class.name) @class

(import_spec) @import

(call_expression) @call
