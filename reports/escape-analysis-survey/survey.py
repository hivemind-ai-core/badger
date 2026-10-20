"""Classify the allocation sites left in optimised MIR by where their value flows.

Input: files holding `badger run --release --dump-mir` output. Each `Alloc` is
followed through its plain copies (`Use(Local)`, including copies into locals
assigned more than once); a use that is not a field load, a field store into
the object, or a tag read decides its class. A site escapes if any use is a
return, a store into another object, a global, a branch on the pointer, a word
conversion, an unknown closure call, or a runtime function whose name says it
keeps its argument. What is left is the upper bound a stack-allocation pass
could take, split by what it would need to prove.

Usage: python3 survey.py DUMP...
"""
import collections
import re
import sys

RETAINS = ('cleanup_push', 'iter_on_close', 'scope_defer', 'map_insert', 'atom', 'native_call',
           'err_origin', 'list_push', 'append', 'spawn', 'send', 'set', 'insert', 'push', 'builder')
ESCAPE = {'return', 'store', 'global', 'retains', 'other', 'toword', 'closure-call'}


def classify(lines, root):
    group = {root}
    grew = True
    while grew:
        grew = False
        for x in lines:
            m = re.match(r'Assign\((\d+), Use\(Local\((\d+)\)\)\)', x)
            if m and m.group(2) in group and m.group(1) not in group:
                group.add(m.group(1))
                grew = True
    kinds = set()
    for x in lines:
        for g in group:
            if not re.search(r'Local\(%s\)' % g, x):
                continue
            if (re.match(r'Assign\(\d+, Alloc', x) or re.match(r'Store \{ base: Local\(%s\)' % g, x)
                    or re.match(r'Assign\(\d+, (Load \{ base: Local\(%s\)|Tag\(Local\(%s\)\))' % (g, g), x)
                    or re.match(r'Assign\(\d+, Use\(Local\(%s\)\)\)' % g, x)):
                continue
            if x.startswith('Some(Return'):
                kinds.add('return')
            elif x.startswith(('Some(Branch', 'Some(Switch')):
                kinds.add('other')
            elif x.startswith('Store'):
                kinds.add('store')
            elif x.startswith('GlobalStore'):
                kinds.add('global')
            elif m := re.search(r'Runtime\("(\w+)"\)', x) or re.search(r'Intrinsic \{ name: "(\w+)"', x):
                name = m.group(1)
                kinds.add('retains' if any(r in name for r in RETAINS)
                          else 'runtime:' + name.removeprefix('badger_rt_i_').removeprefix('badger_rt_'))
            elif 'Direct(' in x:
                kinds.add('direct-call')
            elif 'Closure(Const' in x:
                kinds.add('static-closure-call')
            elif 'Closure(' in x or 'Lambda(' in x:
                kinds.add('closure-call')
            elif 'Concat' in x:
                kinds.add('concat')
            elif 'Cmp' in x:
                kinds.add('cmp')
            elif 'ToWord' in x:
                kinds.add('toword')
            else:
                kinds.add('other')
    if kinds & ESCAPE:
        return 'escapes'
    return ' + '.join(sorted(kinds)) or 'local only, through a merge'


def main():
    tally = collections.Counter()
    for path in sys.argv[1:]:
        text = open(path, errors='replace').read()
        for func in re.split(r'\n(?=fn \d+ )', text):
            if not func.startswith('fn '):
                continue
            lines = [x.strip() for x in func.split('\n')]
            for root in set(re.findall(r'Assign\((\d+), Alloc \{', func)):
                tally[classify(lines, root)] += 1
    print('allocation sites', sum(tally.values()))
    for kind, n in tally.most_common():
        print(f'{n:6} {kind}')


if __name__ == '__main__':
    main()
