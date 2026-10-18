"""Exact, ordered JASS RNG fixtures. No world clock or renderer is driven here."""
from pathlib import Path
import hashlib
import html
import json
import re


def encoded(value):
    return (json.dumps(value, sort_keys=True, separators=(',', ':')) + '\n').encode()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def read(path, maximum):
    with Path(path).open('rb') as source:
        data = source.read(maximum + 1)
    if len(data) > maximum:
        raise ValueError(f'{path}: input exceeds {maximum} bytes')
    return data


def document(path):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f'duplicate field: {key}')
            result[key] = value
        return result
    return json.loads(read(path, 2 << 20), object_pairs_hook=unique)


def validate(plan):
    if not isinstance(plan, dict) or set(plan) != {'version', 'nonce', 'steps'} or type(plan['version']) is not int or plan['version'] != 1:
        raise ValueError('RNG fixture requires version 1, nonce and steps')
    if not isinstance(plan['nonce'], str) or not re.fullmatch(r'[A-Za-z0-9_-]{1,40}', plan['nonce']):
        raise ValueError('RNG nonce must be 1..40 safe ASCII characters')
    steps = plan['steps']
    if not isinstance(steps, list) or not 1 <= len(steps) <= 256:
        raise ValueError('RNG fixture requires 1..256 ordered steps')
    for index, step in enumerate(steps):
        if not isinstance(step, dict):
            raise ValueError(f'RNG step {index}: expected object')
        op = step.get('op')
        if op == 'seed' and set(step) == {'op', 'value'}:
            values = [step['value']]
        elif op in ('integer', 'real') and set(step) == {'op', 'low', 'high'}:
            values = [step['low'], step['high']]
            if op == 'real' and (any(type(v) is not int or v not in (-1, 0, 1) for v in values) or values[0] > values[1]):
                raise ValueError('real fixture bounds must be ordered -1, 0 or 1; bit extraction supports this finite domain')
        else:
            raise ValueError(f'RNG step {index}: unsupported operation or fields')
        if any(type(v) is not int or not -(2**31) <= v < 2**31 for v in values):
            raise ValueError(f'RNG step {index}: expected signed i32 arguments')
    if steps[0]['op'] != 'seed':
        raise ValueError('RNG fixture must begin with an explicit executed seed call')
    if not all(any(s['op'] == op and s['low'] != s['high'] for s in steps) for op in ('integer', 'real')):
        raise ValueError('RNG fixture needs both nondegenerate integer and real draws')
    return plan


def integer(value):
    return '(-2147483647 - 1)' if value == -(2**31) else str(value)


def probe(plan):
    """Only mathematical IEEE binary32 invariants occur in the bit extractor."""
    key = digest(encoded(plan))
    globals_source = '    integer array PCXRngValues\n    boolean PCXRngUnsupported = false\n'
    lines = ['function PCXRngBits takes real value returns integer',
             '    local integer sign = 0', '    local integer exponent = 150',
             '    if value > 1.0 or value < -1.0 then',
             '        set PCXRngUnsupported = true', '        return 0', '    endif',
             '    if value < 0.0 then', '        set sign = -2147483647 - 1',
             '        set value = -value', '    elseif value <= 0.0 then',
             '        if value != 0.0 then', '            return -2147483647 - 1',
             '        endif', '        return 0', '    endif',
             '    loop', '        exitwhen value >= I2R(8388608) or exponent < 1',
             '        set value = value * I2R(2)', '        set exponent = exponent - 1',
             '    endloop', '    if exponent < 1 then',
             '        set PCXRngUnsupported = true', '        return 0', '    endif',
             '    return sign + exponent * 8388608 + R2I(value) - 8388608',
             'endfunction', '', 'function PCXRngFixture takes nothing returns nothing',
             '    set PCXRngUnsupported = false']
    for index, step in enumerate(plan['steps']):
        op = step['op']
        if op == 'seed':
            lines += [f'    call SetRandomSeed({integer(step["value"])})',
                      f'    set PCXRngValues[{index}] = {integer(step["value"])}']
        else:
            a, b = integer(step['low']), integer(step['high'])
            call = f'GetRandomInt({a}, {b})' if op == 'integer' else f'PCXRngBits(GetRandomReal(I2R({a}), I2R({b})))'
            lines.append(f'    set PCXRngValues[{index}] = {call}')
    # Store all results before formatting/export so output work cannot interleave draws.
    lines += ['    call PreloadGenClear()', '    call PreloadGenStart()',
              f'    call Preload("PCX_RNG1 {key} begin {len(plan["steps"])}")',
              '    if PCXRngUnsupported then',
              f'        call Preload("PCX_RNG1 {key} unsupported")', '    endif']
    for index, step in enumerate(plan['steps']):
        a, b = (step['value'], 0) if step['op'] == 'seed' else (step['low'], step['high'])
        prefix = f'PCX_RNG1 {key} {index} {step["op"]} {a} {b} '
        lines.append(f'    call Preload("{prefix}" + I2S(PCXRngValues[{index}]))')
    lines += [f'    call Preload("PCX_RNG1 {key} end {len(plan["steps"])}")',
              f'    call PreloadGenEnd("PCX-RNG-{plan["nonce"]}.txt")', 'endfunction', '']
    functions = '\n'.join(lines)
    return globals_source, functions, 'globals\n' + globals_source + 'endglobals\n\n' + functions


def output_directory(value, ref):
    path = Path(value).resolve()
    roots = [ref.HERE.parents[1] / '.scratch', Path('/tmp')]
    if not any(path.is_relative_to(base.resolve()) and path != base.resolve() for base in roots):
        raise ValueError('RNG artifacts must stay in local .scratch or /tmp')
    if path.exists():
        raise ValueError('RNG output exists; choose a new directory to preserve evidence')
    return path


def fixture_manifest(plan, files):
    return {'version': 1, 'scope': 'serial_jass_rng', 'scenario_sha256': digest(encoded(plan)),
              'files': {name: digest(data) for name, data in files.items()},
              'entrypoint': 'PCXRngFixture', 'output_name': f'PCX-RNG-{plan["nonce"]}.txt',
              'clock': 'one serial callback; no world tick advancement',
              'deterministic_parity_verified': False}


def prepare(args, ref):
    plan = validate(document(args.scenario))
    target = output_directory(args.output, ref)
    global_source, functions, script = probe(plan)
    target.mkdir(parents=True, mode=0o700)
    files = {'scenario.json': encoded(plan), 'probe-globals.j': global_source.encode(),
             'probe-functions.j': functions.encode(), 'probe.j': script.encode()}
    for name, data in files.items():
        (target / name).write_bytes(data)
    result = fixture_manifest(plan, files)
    ref.write_json(target / 'fixture.json', result)
    return {'fixture': str(target), **result}


def records(raw, plan):
    text = raw.decode('utf-8')
    rows = [match.group(1).split() for match in re.finditer(r'PCX_RNG1 ([^"\r\n]*)', text)]
    key, count = digest(encoded(plan)), len(plan['steps'])
    if [key, 'unsupported'] in rows:
        raise ValueError('RNG fixture encountered an unsupported real number class')
    if len(rows) != count + 2 or rows[0] != [key, 'begin', str(count)] or rows[-1] != [key, 'end', str(count)]:
        raise ValueError('missing, extra, foreign or incomplete RNG records')
    values = []
    for index, (row, step) in enumerate(zip(rows[1:-1], plan['steps'], strict=True)):
        a, b = (step['value'], 0) if step['op'] == 'seed' else (step['low'], step['high'])
        if len(row) != 6 or row[:5] != [key, str(index), step['op'], str(a), str(b)]:
            raise ValueError(f'RNG record {index}: wrong operation, arguments or order')
        value = row[-1]
        if not re.fullmatch(r'0|-?[1-9][0-9]{0,9}', value) or not -(2**31) <= int(value) < 2**31:
            raise ValueError(f'RNG record {index}: noncanonical i32 result')
        if step['op'] == 'seed' and int(value) != a:
            raise ValueError(f'RNG record {index}: seed was not acknowledged')
        values.append(int(value))
    return values


def compare(args, ref):
    fixture = Path(args.fixture).resolve(strict=True)
    plan = validate(document(fixture / 'scenario.json'))
    manifest = document(fixture / 'fixture.json')
    global_source, functions, script = probe(plan)
    expected = {'scenario.json': encoded(plan), 'probe-globals.j': global_source.encode(),
                'probe-functions.j': functions.encode(), 'probe.j': script.encode()}
    if manifest != fixture_manifest(plan, expected):
        raise ValueError('RNG fixture manifest changed')
    for name, data in expected.items():
        if read(fixture / name, 256 << 10) != data:
            raise ValueError(f'RNG fixture changed: {name}')
    target = output_directory(args.output, ref)
    runs, identities, run_ids, observations = [], set(), set(), set()
    for kind, paths in [('original', args.original), ('recreation', args.recreation)]:
        if not 2 <= len(paths) <= 8:
            raise ValueError(f'RNG comparison needs 2..8 independent {kind} run receipts')
        for path in paths:
            receipt = document(path)
            if not isinstance(receipt, dict) or set(receipt) != {'version', 'target', 'run_id', 'runtime', 'files'} or type(receipt['version']) is not int or receipt['version'] != 1 or receipt['target'] != kind:
                raise ValueError('invalid RNG run receipt')
            if not isinstance(receipt['run_id'], str) or not 1 <= len(receipt['run_id']) <= 256 or receipt['run_id'] in run_ids:
                raise ValueError('RNG run ids must be bounded and independent')
            if not isinstance(receipt['runtime'], str) or not 1 <= len(receipt['runtime']) <= 128:
                raise ValueError('RNG runtime identity is required')
            run_ids.add(receipt['run_id'])
            files = receipt['files']
            if not isinstance(files, dict) or set(files) != {'image', 'common', 'probe', 'observation', 'execution'}:
                raise ValueError('RNG receipt needs image/common/probe/observation/execution fingerprints')
            hashes, data = {}, {}
            for name, maximum in [('image', 64 << 20), ('common', 4 << 20), ('probe', 256 << 10), ('observation', 2 << 20), ('execution', 2 << 20)]:
                item = files[name]
                if not isinstance(item, dict) or set(item) != {'path', 'sha256'}:
                    raise ValueError('invalid RNG fingerprint')
                if not isinstance(item['path'], str) or not isinstance(item['sha256'], str) or not re.fullmatch(r'[0-9a-f]{64}', item['sha256']):
                    raise ValueError('invalid RNG fingerprint path or digest')
                raw = read(item['path'], maximum)
                hashes[name] = digest(raw)
                if hashes[name] != item['sha256']:
                    raise ValueError(f'RNG {kind} {name} changed')
                if name != 'image':
                    data[name] = raw
            if data['probe'] != script.encode():
                raise ValueError('RNG run did not select this probe')
            execution = json.loads(data['execution'])
            if not isinstance(execution, dict):
                raise ValueError('RNG execution receipt must be an object')
            if kind == 'original':
                if execution.get('complete') is not True or execution.get('records') != len(plan['steps']) or execution.get('observation_sha256') != hashes['observation']:
                    raise ValueError('original RNG collection was not complete for this observation')
            elif type(execution.get('exit_code')) is not int or execution['exit_code'] != 0:
                raise ValueError('recreation RNG execution did not succeed')
            observation = Path(files['observation']['path']).resolve()
            inode = (observation.stat().st_dev, observation.stat().st_ino)
            if inode in observations:
                raise ValueError('each RNG run must retain its own observation file')
            observations.add(inode)
            identities.add((hashes['image'], hashes['common'], hashes['probe']))
            runs.append({'target': kind, 'run_id': receipt['run_id'], 'runtime': receipt['runtime'],
                         'receipt': str(Path(path).resolve()), 'receipt_sha256': digest(read(path, 2 << 20)),
                         'files': files, 'values': records(data['observation'], plan)})
    if len(identities) != 1:
        raise ValueError('RNG runs do not share the selected image, common declarations and probe')
    reference = runs[0]['values']
    differences = [{'run_id': run['run_id'], 'step': index, 'operation': plan['steps'][index],
                    'original': left, 'observed': right}
                   for run in runs[1:] for index, (left, right) in enumerate(zip(reference, run['values'], strict=True)) if left != right]
    first = min(differences, key=lambda row: row['step'], default=None)
    result = {'version': 1, 'scope': 'serial_jass_rng', 'scenario_sha256': digest(encoded(plan)),
              'clock': 'one serial callback; no world tick advancement',
              'fixture_manifest_sha256': digest(read(fixture / 'fixture.json', 2 << 20)),
              'harness_sha256': {name: digest(read(ref.HERE / name, 2 << 20)) for name in ['reference.py', 'rng_fixture.py', 'schema.json']},
              'runs': runs, 'steps': len(reference), 'rng_results_match': not differences,
              'first_divergence': first,
              'deterministic_parity_verified': False,
              'limitation': 'Exact serial RNG results only; engine startup, other consumers, world clocks and rendering are not synchronized.'}
    target.mkdir(parents=True, mode=0o700)
    ref.write_json(target / 'comparison.json', result)
    header = ''.join('<th>' + html.escape(run['run_id']) + '</th>' for run in runs)
    body = ''.join('<tr><td>' + str(index) + '</td><td>' + html.escape(json.dumps(step, sort_keys=True)) + '</td>' +
                   ''.join('<td>' + (f'0x{run["values"][index] & 0xffffffff:08x}' if step['op'] == 'real' else str(run['values'][index])) + '</td>' for run in runs) + '</tr>'
                   for index, step in enumerate(plan['steps']))
    (target / 'index.html').write_text('<!doctype html><meta charset="utf-8"><title>RNG comparison</title>'
        '<h1>Serial RNG comparison</h1><p>' + html.escape(result['limitation']) + '</p><p>' +
        ('All ordered results match.' if not differences else html.escape(json.dumps(first, sort_keys=True))) +
        '</p><p>Real values are exact binary32 bits in hexadecimal.</p><table border="1"><tr><th>Step</th><th>Operation</th>' + header + '</tr>' + body + '</table>')
    return {'receipt': str(target / 'comparison.json'), 'report': str(target / 'index.html'),
            'steps': len(reference), 'runs': len(runs), 'rng_results_match': not differences,
            'first_divergence': result['first_divergence'], 'deterministic_parity_verified': False}
