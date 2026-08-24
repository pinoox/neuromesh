; NeuroMesh extract profile — tree-sitter-rust
(function_item
  name: (identifier) @function.name) @function

(impl_item
  type: (_) @impl.type) @impl

(struct_item
  name: (type_identifier) @class.name) @class

(enum_item
  name: (type_identifier) @class.name) @class

(trait_item
  name: (type_identifier) @symbol.name) @symbol

(type_item
  name: (type_identifier) @symbol.name) @symbol

(use_declaration) @import

(call_expression) @call
