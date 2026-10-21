import unittest

from profile_trace import summarize


def event(phase, name, timestamp, thread=1, **args):
    return dict(ph=phase, name=name, ts=timestamp, pid=1, tid=thread, cat='pcx_cpu', args=args)


class TraceSummaryTests(unittest.TestCase):
    def test_nested_work_is_not_double_counted_and_threads_are_independent(self):
        result = summarize([
            event('B', 'parent', 0), event('B', 'other-thread', 1, thread=2),
            event('B', 'child', 2), event('E', 'child', 5),
            event('E', 'other-thread', 7, thread=2), event('E', 'parent', 10),
        ])
        rows = {row['name']: row for row in result['rows']}
        self.assertEqual(rows['parent']['elapsed_us'], 10)
        self.assertEqual(rows['parent']['self_elapsed_us'], 7)
        self.assertEqual(rows['other-thread']['self_elapsed_us'], 6)
        self.assertEqual(sum(row['self_elapsed_us'] for row in rows.values()), 16)

    def test_native_calls_keep_their_identity_and_accumulate(self):
        result = summarize([
            event('B', 'native', 0, native='CreateUnit'), event('E', 'native', 3),
            event('B', 'native', 4, native='CreateUnit'), event('E', 'native', 9),
            event('B', 'native', 10, native='GetUnitX'), event('E', 'native', 11),
        ], limit=1)
        self.assertEqual(result['rows'][0]['native'], 'CreateUnit')
        self.assertEqual(result['rows'][0]['calls'], 2)
        self.assertEqual(result['rows'][0]['elapsed_us'], 8)
        self.assertEqual(result['rows'][0]['max_us'], 5)
        self.assertEqual(result['omitted_rows'], 1)

    def test_missing_mismatched_or_unclosed_spans_refuse(self):
        for events in [
            [event('E', 'absent', 1)], [event('B', 'open', 1)],
            [event('B', 'first', 0), event('E', 'second', 1)],
            [event('B', 'first', 3), event('E', 'first', 2)],
            [event('B', 'first', float('nan'))],
            [event('B', 'first', True)],
            [event('B', 'first', 0), dict(event('E', 'first', 1), cat='other')],
            [event('X', 'complete', 0)], [],
        ]:
            with self.subTest(events=events), self.assertRaises(ValueError):
                summarize(events)

    def test_collection_and_output_bounds_are_explicit(self):
        events = [event('B', 'span', 0), event('E', 'span', 1)]
        for kwargs in [dict(limit=0), dict(limit=1001), dict(max_events=1), dict(max_events=float('inf'))]:
            with self.subTest(kwargs=kwargs), self.assertRaises(ValueError):
                summarize(events, **kwargs)
        self.assertTrue(summarize(events)['timing_is_report_only'])


if __name__ == '__main__':
    unittest.main()
