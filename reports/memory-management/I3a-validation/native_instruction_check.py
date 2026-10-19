"""Compare runtime instruction text after symbolizing relocation operands."""
from pathlib import Path
import hashlib
import json
import re
import subprocess

BIN = Path('/home/dan/orca/projects/language/target/memory-binaries-2026-10-18')
ARTIFACTS = Path(__file__).resolve().parent
binaries = {'baseline': BIN / 'map-helpers-candidate', 'candidate': BIN / 'stored-sequence-candidate'}
report = {}
relocations = {}
for label, binary in binaries.items():
    symbols = subprocess.check_output(['nm', '-S', '-C', str(binary)], text=True)
    names = {int(parts[0], 16): parts[3] for line in symbols.splitlines() if len(parts := line.split(maxsplit=3)) == 4}
    entries = {}
    for line in subprocess.check_output(['readelf', '-rW', str(binary)], text=True).splitlines():
        parts = line.split()
        if len(parts) == 4 and parts[2] == 'R_X86_64_RELATIVE':
            target = int(parts[3], 16)
            entries[int(parts[0], 16)] = names.get(target, f'unsymbolized-relative:{target:x}')
        elif len(parts) >= 5 and parts[2] == 'R_X86_64_GLOB_DAT':
            entries[int(parts[0], 16)] = parts[4]
    relocations[label] = entries
for name in ['badger_runtime::buffer::push', 'badger_runtime::intrinsics_std::buffer_push', 'badger_runtime::gc::alloc']:
    report[name] = {}
    for label, binary in binaries.items():
        symbols = subprocess.check_output(['nm', '-S', '-C', str(binary)], text=True)
        match = next(line.split(maxsplit=3) for line in symbols.splitlines() if len(line.split(maxsplit=3)) == 4 and line.split(maxsplit=3)[3] == name)
        address, size = int(match[0], 16), int(match[1], 16)
        assembly = subprocess.check_output(['objdump', '-d', '--no-show-raw-insn', f'--start-address={address}', f'--stop-address={address+size}', str(binary)], text=True)
        normalized = []
        for line in assembly.splitlines():
            instruction = re.match(r'^\s*[0-9a-f]+:\s*(.*)', line)
            if instruction:
                text = instruction[1]
                got = re.search(r'#\s*([0-9a-f]+)\s+<_DYNAMIC[^>]*>', text)
                if got:
                    target = relocations[label].get(int(got[1], 16))
                    assert target is not None, text
                    text = text[:got.start()] + '# <GOT:' + target + '>'
                text = re.sub(r'\b[0-9a-f]+\s+(?=<)', '', text)
                text = re.sub(r'(?<![\w$])-?0x[0-9a-f]+(?=\(%rip\))', 'RIP', text)
                text = re.sub(r'#\s*[0-9a-f]+', '#ADDR', text)
                normalized.append(text)
        report[name][label] = dict(address=address, alignment_mod64=address % 64, size=size,
                                   normalized=normalized, normalized_sha256=hashlib.sha256('\n'.join(normalized).encode()).hexdigest())
    report[name]['matching_normalized_instructions'] = report[name]['baseline']['normalized'] == report[name]['candidate']['normalized']
    print(name, report[name]['matching_normalized_instructions'])
(ARTIFACTS / 'runtime-instructions.json').write_text(json.dumps(report, indent=2)+'\n')
