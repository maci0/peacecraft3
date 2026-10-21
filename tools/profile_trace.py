"""Summarize threaded tracing-chrome spans without counting children twice.

Durations use the producer's monotonic wall clock, not thread CPU time.
"""
import math


def summarize(events, *, limit=50, max_events=2_000_000):
    if type(max_events) is not int or max_events < 1:
        raise ValueError('event budget must be a positive integer')
    if not isinstance(events, list) or len(events) > max_events:
        raise ValueError('trace must be an event list within the event budget')
    if not isinstance(limit, int) or not 1 <= limit <= 1000:
        raise ValueError('summary limit must be between 1 and 1000')
    stacks, clocks, totals = {}, {}, {}
    count = 0
    for event in events:
        if not isinstance(event, dict):
            raise ValueError('trace event must be an object')
        phase = event.get('ph')
        if phase in ('M', 'i', 'I', 's', 't', 'f'):
            continue
        if phase not in ('B', 'E'):
            raise ValueError(f'unsupported duration phase {phase!r}; use threaded B/E traces')
        thread = (event.get('pid'), event.get('tid'))
        if any(type(value) is not int for value in thread):
            raise ValueError('duration event needs integer process and thread identities')
        timestamp = event.get('ts')
        if type(timestamp) not in (int, float) or not math.isfinite(timestamp):
            raise ValueError('duration event needs a finite timestamp')
        if timestamp < clocks.get(thread, timestamp):
            raise ValueError('thread timestamps moved backwards')
        clocks[thread] = timestamp
        stack = stacks.setdefault(thread, [])
        if phase == 'B':
            name, category = event.get('name'), event.get('cat', '')
            args = event.get('args', {})
            if not isinstance(name, str) or not name or not isinstance(category, str) or not isinstance(args, dict):
                raise ValueError('span needs a name, category and argument object')
            native = args.get('native', '')
            if not isinstance(native, str):
                raise ValueError('native span argument must be text')
            stack.append({'key': (category, name, native), 'start': timestamp, 'children': 0.})
            continue
        if not stack:
            raise ValueError('span exit has no matching entry')
        span = stack.pop()
        if event.get('name', span['key'][1]) != span['key'][1]:
            raise ValueError('span exit name differs from its entry')
        if event.get('cat', span['key'][0]) != span['key'][0]:
            raise ValueError('span exit category differs from its entry')
        elapsed = timestamp - span['start']
        exclusive = elapsed - span['children']
        # Floating-point subtraction can leave a tiny negative remainder.
        if exclusive < -1e-6:
            raise ValueError('child spans exceed the parent duration')
        if stack:
            stack[-1]['children'] += elapsed
        row = totals.setdefault(span['key'], {'calls': 0, 'elapsed_us': 0., 'self_elapsed_us': 0., 'max_us': 0.})
        row['calls'] += 1
        row['elapsed_us'] += elapsed
        row['self_elapsed_us'] += max(0., exclusive)
        row['max_us'] = max(row['max_us'], elapsed)
        count += 1
    if any(stacks.values()):
        raise ValueError('trace ends inside a span; close the workload normally to flush it')
    if not count:
        raise ValueError('trace contains no completed spans')
    rows = [dict(category=category, name=name, native=native, **values)
            for (category, name, native), values in totals.items()]
    rows.sort(key=lambda row: (-row['self_elapsed_us'], row['category'], row['name'], row['native']))
    return {'clock': 'monotonic_wall', 'timing_is_report_only': True,
            'completed_spans': count, 'distinct_spans': len(rows), 'omitted_rows': max(0, len(rows) - limit),
            'rows': rows[:limit]}
