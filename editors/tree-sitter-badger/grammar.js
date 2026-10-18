/**
 * Tree-sitter grammar for Badger (`.bdg`), for editor highlighting and
 * structural queries. It follows crates/badger_syntax/src/grammar.pest but is
 * deliberately lenient: newlines are whitespace here (the compiler's layout
 * pass, which makes them statement separators, is not reproduced), so a few
 * rare multi-line forms may parse differently than the compiler parses them.
 * The compiler remains the authority on what a program means.
 */

const PREC = {
  pipe: 1,
  range: 2,
  or: 3,
  and: 4,
  compare: 5,
  bitor: 6,
  bitxor: 7,
  bitand: 8,
  shift: 9,
  add: 10,
  mul: 11,
  unary: 12,
  postfix: 13,
};

const sep1 = (rule, s) => seq(rule, repeat(seq(s, rule)));
const commaSep = (rule) => optional(seq(sep1(rule, ','), optional(',')));
const commaSep1 = (rule) => seq(sep1(rule, ','), optional(','));

module.exports = grammar({
  name: 'badger',

  extras: ($) => [/\s/, $.line_comment, $.block_comment],

  // nested block comments need counting
  externals: ($) => [$.block_comment],

  word: ($) => $.identifier,

  conflicts: ($) => [
    [$.module_path],
    [$._literal, $._label],
    [$.record_literal, $._record_entry],
    [$._member, $._primary],
  ],

  rules: {
    source_file: ($) => repeat(choice($._item, ';')),

    // ------------------------------------------------------------ comments
    line_comment: (_) => token(seq('//', /[^\n]*/)),

    // ------------------------------------------------------------ names
    identifier: (_) => /[a-z_][A-Za-z0-9_]*/,
    type_identifier: (_) => /[A-Z][A-Za-z0-9_]*/,
    _name: ($) => choice($.identifier, $.type_identifier),
    _member: ($) => choice($.identifier, $.type_identifier, alias(/[0-9]+/, $.integer)),

    // ------------------------------------------------------------ items
    _item: ($) =>
      choice(
        $.import_declaration,
        $.type_alias,
        $.struct_declaration,
        $.enum_declaration,
        $.protocol_declaration,
        $.impl_declaration,
        $.function_declaration,
        $.provider_declaration,
        $.stage_declaration,
        $.extern_declaration,
        $.extern_type,
        $.global_declaration,
      ),

    visibility: (_) => 'pub',

    import_declaration: ($) =>
      seq(
        'import',
        $.module_path,
        optional(choice(seq('as', $.identifier), seq('.', '{', commaSep($.import_item), '}'))),
      ),
    module_path: ($) => sep1($._name, '.'),
    import_item: ($) => seq($._name, optional(seq('as', $._name))),

    type_alias: ($) =>
      seq(optional($.visibility), 'type', field('name', $.type_identifier), optional($.generic_parameters), '=', $._type),

    struct_declaration: ($) =>
      seq(
        optional($.visibility),
        optional(seq('repr', 'c')),
        'struct',
        field('name', $.type_identifier),
        optional($.generic_parameters),
        '{',
        commaSep($.struct_field),
        '}',
        optional($.derive_clause),
      ),
    struct_field: ($) => seq(optional($.visibility), field('name', $._member), ':', $._type),

    enum_declaration: ($) =>
      seq(
        optional($.visibility),
        'enum',
        field('name', $.type_identifier),
        optional($.generic_parameters),
        '{',
        commaSep($.enum_variant),
        '}',
        optional($.derive_clause),
      ),
    enum_variant: ($) => seq(field('name', $._member), optional(seq('(', commaSep($.variant_field), ')'))),
    variant_field: ($) => seq(optional(seq(field('name', $._member), ':')), $._type),
    derive_clause: ($) => seq('derive', '(', commaSep1($.type_path), ')'),

    protocol_declaration: ($) =>
      seq(
        optional($.visibility),
        'protocol',
        field('name', $.type_identifier),
        optional($.generic_parameters),
        optional(seq(':', sep1($._type, '+'))),
        '{',
        repeat(choice($.function_declaration, $.associated_type, ';')),
        '}',
      ),
    associated_type: ($) => seq('type', field('name', $.type_identifier), optional(seq('=', $._type))),

    impl_declaration: ($) =>
      seq(
        'impl',
        optional($.generic_parameters),
        $._type,
        optional(seq('for', $._type)),
        optional($.where_clause),
        '{',
        repeat(choice($.function_declaration, $.associated_type, ';')),
        '}',
      ),

    contract: ($) => seq(choice('requires', 'ensures'), $._expression, optional(seq(',', $.string))),

    function_declaration: ($) =>
      seq(
        repeat($.contract),
        optional($.visibility),
        'fn',
        field('name', $._member),
        optional($.generic_parameters),
        $.parameters,
        optional(seq('->', field('return_type', $._type))),
        optional($.effect),
        optional($.where_clause),
        optional(choice(field('body', $.block), seq('=>', field('body', $._expression)))),
      ),

    // `provider name [as #sym](scope, config) [-> Result<O, E>] { ... }`
    provider_declaration: ($) =>
      seq(
        repeat($.contract),
        optional($.visibility),
        'provider',
        field('name', $.identifier),
        optional(seq('as', field('label', $.symbol))),
        $.parameters,
        optional(seq('->', field('return_type', $._type))),
        optional($.effect),
        field('body', $.block),
      ),

    // `stage name [as #sym][<G>](params) [-> Result<_, E>] { enter(ctx) { ... } ... }`
    stage_declaration: ($) =>
      seq(
        repeat($.contract),
        optional($.visibility),
        'stage',
        field('name', $.identifier),
        optional(seq('as', field('label', $.symbol))),
        optional($.generic_parameters),
        $.parameters,
        optional(seq('->', field('return_type', $._type))),
        '{',
        repeat(choice($.stage_callback, ';')),
        '}',
      ),

    stage_callback: ($) =>
      seq(
        field('kind', choice('enter', 'leave', 'error')),
        $.parameters,
        optional(seq('->', field('return_type', $._type))),
        choice(field('body', $.block), seq('=>', field('body', $._expression))),
      ),

    extern_declaration: ($) =>
      seq(
        optional($.visibility),
        'extern',
        $.string,
        'fn',
        field('name', $.identifier),
        optional($.generic_parameters),
        $.parameters,
        '->',
        field('return_type', $._type),
        optional($.effect),
        optional($.where_clause),
      ),
    extern_type: ($) =>
      seq(optional($.visibility), 'extern', 'type', field('name', $.type_identifier), optional($.generic_parameters)),

    global_declaration: ($) =>
      seq(optional($.visibility), choice('let', 'var', 'const'), $._pattern, optional(seq(':', $._type)), '=', $._expression),

    parameters: ($) => seq('(', commaSep($.parameter), ')'),
    parameter: ($) => seq($._pattern, optional(seq(':', $._type))),
    generic_parameters: ($) => seq('<', commaSep1($.generic_parameter), '>'),
    generic_parameter: ($) => seq($._name, optional(seq(':', sep1($._type, '+')))),
    where_clause: ($) => prec.right(seq('where', sep1($.where_predicate, ','))),
    where_predicate: ($) => seq($._type, ':', sep1($._type, '+')),

    // ------------------------------------------------------------ types
    _type: ($) =>
      choice(
        alias('_', $.hole),
        $.function_type,
        $.forall_type,
        $.reference_type,
        $.dyn_type,
        $.record_type,
        $.unit,
        $.tuple_type,
        seq('(', $._type, ')'),
        $.type_path,
      ),
    type_path: ($) =>
      prec.right(
        seq(repeat(seq($.identifier, '.')), $.type_identifier, optional(seq('.', $.type_identifier)), optional($.type_arguments)),
      ),
    type_arguments: ($) => seq(token.immediate('<'), commaSep1(choice($.effect, $._type)), '>'),
    tuple_type: ($) => seq('(', $._type, ',', commaSep($._type), ')'),
    function_type: ($) =>
      prec.right(seq(optional('once'), 'fn', '(', commaSep($._type), ')', '->', $._type, optional($.effect))),
    forall_type: ($) => prec.right(seq('for', $.generic_parameters, $._type)),
    reference_type: ($) => prec.right(seq('&', optional('mut'), $._type)),
    dyn_type: ($) => seq('dyn', $.type_path),
    record_type: ($) => seq('{', commaSep(choice($.record_type_field, $.row_rest)), '}'),
    record_type_field: ($) => seq(field('name', $._label), ':', $._type),
    row_rest: ($) => seq('..', $.type_identifier),
    effect: ($) => prec.right(seq('!', choice(seq('{', commaSep($._name), '}'), $.type_identifier))),

    // ------------------------------------------------------------ statements
    block: ($) => seq('{', repeat(choice($._statement, ';')), '}'),
    _statement: ($) => choice($.let_statement, $.use_statement, $.assignment, $._expression),
    let_statement: ($) =>
      seq(
        choice('let', 'var', 'const'),
        $._pattern,
        optional(seq(':', $._type)),
        '=',
        $._expression,
        optional(seq('else', $.block)),
      ),
    use_statement: ($) => seq('use', sep1($._pattern, ','), '<-', $._expression),
    assignment: ($) =>
      prec(
        -1,
        seq(
          field('target', $._place),
          field('operator', choice('=', '+=', '-=', '*=', '/=', '%=')),
          field('value', $._expression),
        ),
      ),
    _place: ($) => choice($.identifier, $.field_expression, $.index_expression),

    // ------------------------------------------------------------ expressions
    _expression: ($) =>
      choice(
        $._primary,
        $.unary_expression,
        $.binary_expression,
        $.call_expression,
        $.field_expression,
        $.index_expression,
        $.try_expression,
      ),

    _primary: ($) =>
      choice(
        $._literal,
        $.lambda,
        $.if_expression,
        $.match_expression,
        $.while_expression,
        $.for_expression,
        $.loop_expression,
        $.system_expression,
        $.pipeline_expression,
        $.map_literal,
        $.set_literal,
        $.ref_expression,
        $.unsafe_block,
        $.return_expression,
        $.break_expression,
        $.continue_expression,
        $.record_literal,
        $.block,
        $.unit,
        $.tuple_expression,
        $.parenthesized_expression,
        $.list_literal,
        $.type_identifier,
        $.identifier,
      ),

    _literal: ($) =>
      choice(
        $.string,
        $.raw_string,
        $.interpolated_string,
        $.char,
        $.symbol,
        $.duration,
        $.float,
        $.integer,
        $.boolean,
      ),

    unary_expression: ($) => prec(PREC.unary, seq(choice('-', '!', '&', seq('&', 'mut')), $._expression)),

    binary_expression: ($) => {
      const table = [
        [PREC.pipe, '|>'],
        [PREC.or, '||'],
        [PREC.and, '&&'],
        [PREC.compare, choice('==', '!=', '<', '<=', '>', '>=')],
        [PREC.range, choice('..', '..=')],
        [PREC.bitor, '|'],
        [PREC.bitxor, '^'],
        [PREC.bitand, '&'],
        [PREC.shift, choice('<<', '>>')],
        [PREC.add, choice('+', '-')],
        [PREC.mul, choice('*', '/', '%')],
      ];
      return choice(
        ...table.map(([p, op]) =>
          prec.left(p, seq(field('left', $._expression), field('operator', op), field('right', $._expression))),
        ),
      );
    },

    call_expression: ($) =>
      prec(
        PREC.postfix,
        seq(field('function', $._expression), optional($.type_arguments), field('arguments', $.arguments)),
      ),
    arguments: ($) => seq(token.immediate('('), commaSep($._expression), ')'),
    field_expression: ($) => prec(PREC.postfix, seq(field('value', $._expression), '.', field('field', $._member))),
    index_expression: ($) => prec(PREC.postfix, seq($._expression, token.immediate('['), $._expression, ']')),
    try_expression: ($) => prec(PREC.postfix, seq($._expression, token.immediate('?'))),

    lambda: ($) =>
      prec.right(
        seq(
          optional('once'),
          'fn',
          $.parameters,
          optional(seq('->', $._type)),
          optional($.effect),
          choice($.block, seq('=>', $._expression)),
        ),
      ),

    if_expression: ($) =>
      prec.right(
        seq('if', choice($.let_condition, $._expression), $.block, optional(seq('else', choice($.if_expression, $.block)))),
      ),
    let_condition: ($) => seq('let', $._pattern, '=', $._expression),
    match_expression: ($) => seq('match', $._expression, '{', commaSep($.match_arm), '}'),
    match_arm: ($) =>
      seq($._pattern, repeat(seq('|', $._pattern)), optional(seq('if', $._expression)), '=>', $._expression),
    while_expression: ($) => seq('while', choice($.let_condition, $._expression), $.block),
    for_expression: ($) => seq('for', $._pattern, 'in', $._expression, $.block),
    loop_expression: ($) => seq('loop', $.block),
    // `system`, `pipeline`, `map`, `set`, and `ref` are keywords only before
    // their bracket (they also name standard modules: `set.from_list(...)`)
    system_expression: ($) => seq(alias(token(seq('system', /\s*/, '{')), 'system'), commaSep($.system_entry), '}'),
    system_entry: ($) => seq(field('name', $._label), ':', $._expression),
    pipeline_expression: ($) => seq(alias(token(seq('pipeline', /\s*/, '{')), 'pipeline'), commaSep($._expression), '}'),
    map_literal: ($) => seq(alias(token(seq('map', /\s*/, '{')), 'map'), commaSep($.map_entry), '}'),
    map_entry: ($) => seq($._expression, '=>', $._expression),
    set_literal: ($) => seq(alias(token(seq('set', /\s*/, '{')), 'set'), commaSep($._expression), '}'),
    ref_expression: ($) => seq(alias(token(seq('ref', '(')), 'ref'), $.symbol, ')'),
    unsafe_block: ($) => seq('unsafe', $.block),
    return_expression: ($) => prec.right(seq('return', optional($._expression))),
    break_expression: ($) => prec.right(seq('break', optional($._expression))),
    continue_expression: (_) => 'continue',

    unit: (_) => seq('(', ')'),
    parenthesized_expression: ($) => seq('(', $._expression, ')'),
    tuple_expression: ($) => seq('(', $._expression, ',', commaSep($._expression), ')'),
    list_literal: ($) => seq('[', commaSep($._expression), ']'),

    record_literal: ($) =>
      seq(
        '{',
        choice(
          seq($._record_entry, ',', commaSep($._record_entry)),
          seq(choice($.spread, $.field_initializer), optional(',')),
        ),
        '}',
      ),
    _record_entry: ($) => choice($.spread, $.field_initializer, $.shorthand_field),
    spread: ($) => seq('..', $._expression),
    field_initializer: ($) => seq(field('name', sep1($._label, '.')), ':', field('value', $._expression)),
    shorthand_field: ($) => $.identifier,
    _label: ($) => choice($._member, seq('[', $.symbol, ']')),

    // ------------------------------------------------------------ patterns
    _pattern: ($) =>
      choice(
        alias('_', $.wildcard),
        $.literal_pattern,
        $.unit_pattern,
        $.tuple_pattern,
        $.parenthesized_pattern,
        $.record_pattern,
        $.variant_pattern,
        $.identifier,
      ),
    literal_pattern: ($) =>
      choice(
        seq(optional('-'), choice($.float, $.integer)),
        $.duration,
        $.string,
        $.raw_string,
        $.char,
        $.symbol,
        $.boolean,
      ),
    unit_pattern: (_) => seq('(', ')'),
    parenthesized_pattern: ($) => seq('(', $._pattern, ')'),
    tuple_pattern: ($) => seq('(', $._pattern, ',', commaSep($._pattern), ')'),
    record_pattern: ($) => seq('{', commaSep(choice($.field_pattern, '..')), '}'),
    field_pattern: ($) => seq($._label, optional(seq(':', $._pattern))),
    variant_pattern: ($) =>
      prec.right(
        seq(
          optional(seq(repeat(seq($.identifier, '.')), $.type_identifier)),
          '.',
          field('variant', $._member),
          optional(seq('(', commaSep($._pattern), ')')),
        ),
      ),

    // ------------------------------------------------------------ literals
    boolean: (_) => choice('true', 'false'),
    integer: (_) =>
      token(
        seq(choice(/0x[0-9A-Fa-f_]+/, /0b[01_]+/, /[0-9][0-9_]*/), optional(/[IUF][0-9]+/)),
      ),
    float: (_) =>
      token(
        choice(
          seq(/[0-9][0-9_]*/, choice(seq('.', /[0-9][0-9_]*/, optional(/[eE][+-]?[0-9]+/)), /[eE][+-]?[0-9]+/), optional(/F[0-9]+/)),
          seq(/[0-9][0-9_]*/, /F[0-9]+/),
        ),
      ),
    duration: (_) => token(seq(/[0-9][0-9_]*/, choice('ns', 'us', 'ms', 'min', 's', 'h'))),
    symbol: (_) => token(seq('#', /[A-Za-z][A-Za-z0-9_]*/, optional(seq('/', /[A-Za-z][A-Za-z0-9_]*/)))),
    char: (_) => token(seq("'", choice(/\\(u\{[0-9A-Fa-f]+\}|[nrt\\"'0])/, /[^'\\]/), "'")),
    escape_sequence: (_) => token.immediate(/\\(u\{[0-9A-Fa-f]+\}|[nrt\\"'0])/),
    string: ($) => seq('"', repeat(choice(token.immediate(prec(1, /[^"\\\n]+/)), $.escape_sequence)), token.immediate('"')),
    raw_string: (_) => token(seq('"""', /([^"]|"[^"]|""[^"])*/, '"""')),
    interpolated_string: ($) =>
      seq(
        '$"',
        repeat(
          choice(
            token.immediate(prec(1, /([^"\\{}\n]|\{\{|\}\})+/)),
            $.escape_sequence,
            $.interpolation,
          ),
        ),
        token.immediate('"'),
      ),
    interpolation: ($) => seq(token.immediate('{'), $._expression, '}'),
  },
});
