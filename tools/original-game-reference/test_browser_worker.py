import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


HERE = Path(__file__).resolve().parent


@unittest.skipUnless(shutil.which('node'), 'requires Node.js')
class BrowserWorkerTests(unittest.TestCase):
    def refusal(self, count, character='x', long_error=False):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            # No browser or source assets: exercise the real worker's process protocol.
            module = root / 'playwright.mjs'
            module.write_text('''
import {writeFile} from 'node:fs/promises';
const options = OPTIONS;
const listeners = {};
const page = {
  setDefaultTimeout() {}, route: async () => {},
  on: (name, callback) => { listeners[name] = callback; },
  goto: async () => {
    for (let i = 0; i < options.count; i++)
      listeners.console({type: () => 'error',
        text: () => `${i}:` + options.character.repeat(3900)});
  },
  waitForFunction: async () => {},
  evaluate: async () => ['tft'],
  locator: selector => ({
    setInputFiles: async () => {}, count: async () => 1,
    check: async () => {}, selectOption: async () => {},
    click: async () => {
      if (selector === '#demo-start') {
        const error = Error(options.long_error ? 'failure'.repeat(50000) : 'fixture entry refused');
        if (options.long_error) error.code = 'code'.repeat(1000);
        throw error;
      }
    }
  })
};
export const chromium = {launch: async () => ({
  newContext: async () => ({routeWebSocket: async () => {}, newPage: async () => page}),
  close: async () => writeFile(options.closed, 'closed', {flag: 'wx'})
})};
'''.replace('OPTIONS', json.dumps({'count': count, 'character': character,
                                 'long_error': long_error, 'closed': str(root / 'closed')})))
            (root / 'manifest.json').write_text(json.dumps({'files': {}}))
            config = root / 'config.json'
            config.write_text(json.dumps({'playwrightModule': str(module),
                'manifest': str(root / 'manifest.json'), 'url': 'http://127.0.0.1/',
                'extent': [80, 60], 'edition': 'tft', 'iso': str(root / 'not-read.iso')}))
            result = subprocess.run(['node', str(HERE / 'browser-worker.mjs'), str(config)],
                input=b'', capture_output=True, timeout=15)
            self.assertTrue((root / 'closed').exists(), 'worker must close its browser')
            self.assertEqual(result.returncode, 2, result.stderr[-2000:].decode())
            self.assertEqual(result.stderr, b'')
            self.assertLessEqual(len(result.stdout), 2 * 1024 * 1024 + 1)
            self.assertEqual(len(result.stdout.splitlines()), 1)
            response = json.loads(result.stdout)
            self.assertFalse(response['ok'])
            return response['error']

    def test_short_refusal_preserves_error_and_cleanup(self):
        error = self.refusal(1)
        self.assertEqual(error['message'], 'fixture entry refused')
        self.assertEqual(error['code'], 'BROWSER_REPLAY_FAILED')
        self.assertEqual(error['stage'], 'entry-click')
        self.assertEqual(len(error['errors']), 1)
        self.assertEqual(error['sockets'], [])
        self.assertEqual(error['omitted'], dict(errors=0, sockets=0, requests=0, messages=0))

    def test_large_logs_still_emit_one_bounded_failure(self):
        for character in ['x', '\u754c']:
            with self.subTest(character=character):
                error = self.refusal(600, character)
                self.assertEqual(error['message'], 'fixture entry refused')
                self.assertEqual(error['stage'], 'entry-click')
                self.assertEqual(len(error['errors']), 8)
                self.assertEqual(len(error['messages']), 8)
                self.assertTrue(error['errors'][-1].startswith('599:'))
                self.assertEqual(error['messages'][-1]['text'], error['errors'][-1])
                self.assertEqual(error['omitted'], dict(errors=592, sockets=0, requests=0, messages=592))

    def test_oversized_primary_error_declares_truncation(self):
        error = self.refusal(1, long_error=True)
        self.assertTrue(error['message_truncated'])
        self.assertTrue(error['code_truncated'])
        self.assertEqual(len(error['message']), 4000)
        self.assertEqual(len(error['code']), 128)


if __name__ == '__main__':
    unittest.main()
