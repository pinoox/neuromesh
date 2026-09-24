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

; `pub const DEFAULT_PORT: u16 = 8765` — a public SCREAMING_SNAKE constant
; is a symbol a question names by name. `pub` (any visibility modifier)
; only occurs at module level, so a `const` inside a function body can
; never match here. Private/lowercase consts stay out, like before.
(const_item
  (visibility_modifier)
  name: (identifier) @symbol.name
  (#match? @symbol.name "^[A-Z][A-Z0-9]*(_[A-Z0-9]+)*$")) @symbol

; `pub static EARLY: …` — same rule for statics.
(static_item
  (visibility_modifier)
  name: (identifier) @symbol.name
  (#match? @symbol.name "^[A-Z][A-Z0-9]*(_[A-Z0-9]+)*$")) @symbol
