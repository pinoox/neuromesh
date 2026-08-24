; NeuroMesh extract profile — tree-sitter-dart-orchard (ABI 14)
; Signature and body are siblings; the driver extends @function to the
; following function_body so calls inside the body stay in-scope.
(program
  (function_signature
    name: (identifier) @function.name) @function)

(method_signature
  (function_signature
    name: (identifier) @function.name)) @function

(class_definition
  name: (identifier) @class.name) @class

(enum_declaration
  name: (identifier) @class.name) @class

(import_or_export) @import

(method_invocation) @call

(selector
  (argument_part)) @call
