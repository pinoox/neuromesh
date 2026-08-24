; NeuroMesh extract profile — tree-sitter-ruby
(method
  name: (_) @function.name) @function

(singleton_method
  name: (_) @function.name) @function

(class
  name: [
    (constant) @class.name
    (scope_resolution
      name: (_) @class.name)
  ]) @class

(module
  name: [
    (constant) @class.name
    (scope_resolution
      name: (_) @class.name)
  ]) @class

(call) @call
