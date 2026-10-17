; Badger highlighting (capture names follow the common nvim-treesitter /
; Helix conventions).

(line_comment) @comment
(block_comment) @comment

[
  "import" "as" "type" "struct" "enum" "protocol" "impl" "where"
  "let" "var" "const" "use" "extern" "dyn" "derive" "repr" "unsafe"
  "system" "pipeline" "map" "set" "ref"
] @keyword

["fn" "once"] @keyword.function
["if" "else" "match"] @keyword.conditional
["while" "loop" "for" "in" "break"] @keyword.repeat
(continue_expression) @keyword.repeat
["return"] @keyword.return
["requires" "ensures"] @keyword.directive
(visibility) @keyword.modifier
"mut" @keyword.modifier

; types
(type_identifier) @type
(generic_parameter (type_identifier) @type.parameter)
(type_alias name: (type_identifier) @type.definition)
(struct_declaration name: (type_identifier) @type.definition)
(enum_declaration name: (type_identifier) @type.definition)
(protocol_declaration name: (type_identifier) @type.definition)
(effect) @attribute
(hole) @type

; functions
(function_declaration name: (_) @function)
(extern_declaration name: (identifier) @function)
(call_expression function: (identifier) @function.call)
(call_expression function: (field_expression field: (_) @function.method.call))

; values
(parameter (identifier) @variable.parameter)
(struct_field name: (_) @variable.member)
(field_expression field: (_) @variable.member)
(field_initializer name: (_) @variable.member)
(record_type_field name: (_) @variable.member)
(enum_variant name: (_) @constant)
(variant_pattern variant: (_) @constant)
(module_path (identifier) @module)
(wildcard) @variable.builtin

; literals
(string) @string
(raw_string) @string
(interpolated_string) @string
(escape_sequence) @string.escape
(interpolation ["{" "}"] @punctuation.special)
(char) @character
(symbol) @string.special.symbol
(integer) @number
(float) @number.float
(duration) @number
(boolean) @boolean

; operators and punctuation
[
  "+" "-" "*" "/" "%" "==" "!=" "<" "<=" ">" ">=" "&&" "||" "!" "&" "|" "^"
  "<<" ">>" "|>" ".." "..=" "=" "=>" "->" "<-" "?"
] @operator

["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["," "." ":" ";"] @punctuation.delimiter
