import argparse
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

from PIL import Image

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location('campaign_agent', HERE / 'campaign_agent.py')
agent = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(agent)


def choice(name, confidence=0.9):
    return {'type': 'choice', 'choice': name, 'confidence': confidence,
            'probabilities': {name: confidence}}


def noul(value):
    return {'type': 'noul', 'noul': value}


def box(confidence=0.9):
    return {'type': 'box', 'confidence': confidence,
            'box': {'top': 0.25, 'bottom': 0.75, 'left': 0.25, 'right': 0.75}}


def jev_answers(**overrides):
    answers = {
        'scene': choice('gameplay'),
        'act': choice('left_click'),
        'present': noul(0.95),
        'target': box(),
        'drag_end': box(),
        'hotkey': choice('Escape'),
        'victory': noul(0.01),
        'defeat': noul(0.01),
    }
    answers.update(overrides)
    return answers


class CampaignAgentTests(unittest.TestCase):
    def invoke(self, *args):
        return subprocess.run(
            [sys.executable, str(HERE / 'campaign_agent.py'), *args],
            capture_output=True, text=True, timeout=30)

    def test_schema_matches_pinned_clispec(self):
        import jsonschema
        result = self.invoke('schema')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(result.stderr)
        schema = json.loads(result.stdout)
        published = json.loads((HERE / 'schema/clispec-v0.3.json').read_text())
        jsonschema.Draft202012Validator(published).validate(schema)
        self.assertEqual([command['name'] for command in schema['commands']],
                         ['schema', 'decide', 'play'])
        self.assertEqual(schema['errors'][0]['kind'], 'campaign_failed')

    def test_usage_failure_is_json_stderr(self):
        result = self.invoke('play')
        self.assertEqual(result.returncode, 2)
        self.assertFalse(result.stdout)
        self.assertEqual(json.loads(result.stderr)['error']['kind'], 'campaign_failed')

    def test_fit_size_scales_into_the_vision_limit(self):
        self.assertEqual(agent.fit_size(1920, 1080), (1568, 882))
        self.assertEqual(agent.fit_size(100, 100), (280, 280))
        self.assertEqual(agent.fit_size(320, 280), (320, 280))
        with self.assertRaises(agent.Failure):
            agent.fit_size(10, 4000)

    def test_prepare_frame_writes_a_jpeg_inside_the_limit(self):
        with tempfile.TemporaryDirectory() as tmp:
            image = Path(tmp) / 'frame.png'
            Image.new('RGB', (1920, 1080), (20, 40, 60)).save(image)
            jpeg, width, height = agent.prepare_frame(image.read_bytes())
            self.assertEqual((width, height), (1920, 1080))
            self.assertTrue(jpeg.startswith(b'\xff\xd8'))
            from io import BytesIO
            with Image.open(BytesIO(jpeg)) as decoded:
                self.assertEqual(decoded.size, (1568, 882))

    def test_jev_request_keeps_the_key_out_of_the_body(self):
        secret = 'test-key-not-real'
        body, headers = agent.jev_request(secret, agent.JEV_MODEL, b'\xff\xd8\xff', 'roc-human', 3, ['wait'])
        encoded = json.dumps(body)
        self.assertNotIn(secret, encoded)
        self.assertEqual(headers['Authorization'], f'Bearer {secret}')
        self.assertEqual(headers['TypeSafe-Danger-Zone-Images'], 'true')
        self.assertEqual(body['questions']['target']['type'], 'box')
        self.assertEqual(body['questions']['cell']['type'], 'choice')
        asset_ref = body['state']['frame']
        self.assertIn(asset_ref, body['assets'])
        self.assertNotEqual(asset_ref, 'frame')
        self.assertEqual(len(asset_ref), 32)

    def test_openrouter_request_uses_a_grid_and_omits_the_key(self):
        secret = 'test-key-not-real'
        body, headers = agent.openrouter_request(
            secret, agent.OPENROUTER_MODEL, b'\xff\xd8\xff', 'roc-undead', 1, [])
        self.assertNotIn(secret, json.dumps(body))
        self.assertEqual(body['questions']['cell']['type'], 'choice')
        self.assertNotIn('target', body['questions'])
        self.assertEqual(len(body['questions']['cell']['criteria']), 48)
        self.assertEqual(body['state'][1]['type'], 'image_url')
        self.assertIn('Reign of Chaos undead campaign', body['state'][0])

    def test_interpret_clicks_the_box_center(self):
        decision = agent.interpret(jev_answers(), 320, 280, 'box')
        self.assertEqual(decision['act'], 'left_click')
        self.assertEqual(decision['status'], 'continue')
        self.assertEqual((decision['x'], decision['y']), (160, 140))
        self.assertIsNone(decision['gate'])

    def test_interpret_waits_when_the_point_is_uncertain_or_the_mission_ended(self):
        unsure = agent.interpret(jev_answers(target=box(0.02)), 320, 280, 'box')
        self.assertEqual(unsure['act'], 'wait')
        self.assertEqual(unsure['gate'], 'low_point_confidence')
        won = agent.interpret(jev_answers(victory=noul(0.91)), 320, 280, 'box')
        self.assertEqual(won['status'], 'victory')
        self.assertEqual(won['act'], 'wait')
        lost = agent.interpret(jev_answers(defeat=noul(0.9), victory=noul(0.2)), 320, 280, 'box')
        self.assertEqual(lost['status'], 'defeat')
        wide = box(0.9)
        wide['box'] = {'top': 0.0, 'bottom': 1.0, 'left': 0.0, 'right': 0.8}
        oversized = agent.interpret(jev_answers(target=wide), 320, 280, 'box')
        self.assertEqual(oversized['gate'], 'oversized_box')
        self.assertEqual(oversized['act'], 'wait')

    def test_text_element_click_overrides_a_weak_box_and_refuses_quit(self):
        elements = [
            {'id': 'e0', 'text': 'Campaign', 'x': 875, 'y': 236},
            {'id': 'e1', 'text': 'Cancel', 'x': 880, 'y': 664},
        ]
        clicked = agent.interpret(
            jev_answers(element=choice('e0', 0.8), target=box(0.02)), 1024, 768, 'box', elements)
        self.assertEqual(clicked['act'], 'left_click')
        self.assertEqual((clicked['x'], clicked['y']), (875, 236))
        self.assertEqual(clicked['target_text'], 'Campaign')
        refused = agent.interpret(
            jev_answers(element=choice('e1', 0.9)), 1024, 768, 'box', elements)
        self.assertEqual(refused['act'], 'wait')
        self.assertEqual(refused['gate'], 'forbidden_control')

    def test_group_ocr_tsv_merges_a_line_and_reports_its_center(self):
        header = 'level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext'
        rows = '\n'.join([
            header,
            '5\t1\t1\t1\t1\t1\t800\t220\t80\t16\t90\tCampaign',
            '5\t1\t1\t1\t1\t2\t890\t220\t40\t16\t90\t',
            '5\t1\t1\t1\t2\t1\t40\t500\t50\t12\t40\tnoise',
            '5\t1\t1\t1\t3\t1\t820\t650\t60\t14\t91\tCancel',
        ])
        elements = agent.group_ocr_tsv(rows)
        self.assertEqual(elements[0]['text'], 'Campaign')
        self.assertEqual((elements[0]['x'], elements[0]['y']), (840, 228))
        self.assertEqual(elements[1]['text'], 'Cancel')
        self.assertTrue(agent.forbidden_control(elements[1]['text']))

    def test_type_action_clicks_then_types_only_the_allowed_name(self):
        decision = agent.interpret(jev_answers(
            act=choice('type'), text=choice('Jev'), target=box(0.8)), 320, 280, 'box')
        self.assertEqual(decision['act'], 'type')
        self.assertEqual(decision['text'], 'Jev')
        calls = []

        def fake_action(args):
            calls.append(('click', args.x, args.y))
            return {'action': 'click'}

        def fake_execute(path, argv):
            calls.append(('exec', tuple(argv)))
            return ''

        with patch.object(agent.ref, 'action', side_effect=fake_action), \
                patch.object(agent.ref, 'execute', side_effect=fake_execute), \
                patch.object(agent.ref, 'mark', return_value={'action': 'campaign-type'}):
            agent.perform(Path('/run'), decision)
        self.assertEqual(calls[0][0], 'click')
        self.assertEqual(calls[1][1], ('xdotool', 'type', '--delay', '40', 'Jev'))

    def test_world_cell_replaces_a_command_card_box(self):
        aimed = agent.interpret(jev_answers(
            act=choice('right_click'), target=box(0.02), cell=choice('r1c3', 0.4)), 1024, 768, 'box')
        self.assertEqual(aimed['act'], 'right_click')
        self.assertEqual((aimed['x'], aimed['y']), (448, 192))
        waiting = agent.interpret(jev_answers(
            scene=choice('gameplay'), act=choice('wait'), cell=choice('r2c4', 0.2)), 1024, 768, 'box')
        self.assertEqual(waiting['act'], 'wait')
        self.assertIsNone(waiting['gate'])
        rejected = agent.interpret(jev_answers(
            act=choice('right_click'), target=box(0.02), cell=choice('r5c3', 0.9)), 1024, 768, 'box')
        self.assertEqual(rejected['act'], 'wait')
        self.assertEqual(rejected['gate'], 'low_point_confidence')

    def test_advance_uses_the_selected_direction_only_during_gameplay(self):
        answers = jev_answers(act=choice('advance'), direction=choice('north'))
        moved = agent.interpret(answers, 1000, 800, 'box')
        self.assertEqual((moved['act'], moved['x'], moved['y']), ('right_click', 500, 120))
        self.assertEqual(moved['gate'], 'navigation_order')
        answers['direction'] = choice('north', 0.01)
        self.assertEqual(agent.interpret(answers, 1000, 800, 'box')['act'], 'wait')
        answers['scene'] = choice('cinematic')
        self.assertEqual(agent.interpret(answers, 1000, 800, 'box')['gate'], 'not_gameplay')

    def test_interpret_grid_cell_and_rejects_an_unknown_choice(self):
        answers = jev_answers()
        del answers['target']
        del answers['drag_end']
        answers['cell'] = choice('r0c0', 0.8)
        answers['drag_cell'] = choice('r1c1', 0.8)
        answers['act'] = choice('right_click')
        decision = agent.interpret(answers, 80, 60, 'grid')
        self.assertEqual(decision['act'], 'right_click')
        self.assertEqual((decision['x'], decision['y']), (5, 5))
        answers['act'] = choice('not-an-action')
        with self.assertRaises(agent.Failure):
            agent.interpret(answers, 80, 60, 'grid')

    def test_secret_file_shapes_and_redacted_transport_errors(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'typesafe.ai').write_text('TYPESAFE_API_KEY="file-key"\n')
            self.assertEqual(agent.load_key('jev', environ={}, secrets_dir=root), 'file-key')
            (root / 'openrouter').write_text(json.dumps({'api_key': 'router-key'}))
            self.assertEqual(
                agent.load_key('openrouter', environ={'OPENROUTER_API_KEY': ' env-key '}, secrets_dir=root),
                'env-key')
            with self.assertRaises(agent.Failure):
                agent.load_key('jev', environ={}, secrets_dir=root / 'missing')
        secret = 'super-secret-token'

        def explode(url, headers, body):
            raise agent.Failure(f'upstream said {secret}')

        client = agent.DecisionClient('jev', secret, agent.JEV_MODEL, post=explode)
        with self.assertRaises(agent.Failure) as caught:
            client.decide(b'\xff', 'roc-human', 0, [])
        self.assertNotIn(secret, str(caught.exception))
        self.assertIn('[redacted]', str(caught.exception))

    def test_play_sends_one_click_then_stops_on_victory(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'source.json').write_text('{}\n')
            clicks = []

            def fake_capture(args):
                dest = root / 'captures'
                dest.mkdir(exist_ok=True)
                target = dest / f'{args.label}.png'
                Image.new('RGB', (320, 280), (8, 8, 8)).save(target)
                return {'capture': str(target), 'sha256': 'abc'}

            def fake_action(args):
                clicks.append((args.command, args.x, args.y))
                return {'action': args.command, 'x': args.x, 'y': args.y}

            payloads = [
                {'model': 'jev-1.14.0', 'usage': {'input_tokens': 12, 'output_tokens': 0},
                 'answers': jev_answers()},
                {'model': 'jev-1.14.0', 'usage': {'input_tokens': 12, 'output_tokens': 0},
                 'answers': jev_answers(victory=noul(0.95))},
            ]

            class Script:
                def decide(self, jpeg, campaign, step, recent, elements=None):
                    self.last = (campaign, step, list(recent))
                    return payloads.pop(0)

            script = Script()
            with patch.object(agent.ref, 'capture', side_effect=fake_capture), \
                    patch.object(agent.ref, 'action', side_effect=fake_action), \
                    patch.object(agent.ref, 'execute') as execute:
                result = agent.play_steps(
                    root, steps=5, campaign='roc-human', provider='jev', model='jev-1.14.0',
                    pause_s=0, client=script)
            self.assertEqual(result['status'], 'victory')
            self.assertEqual(result['steps_completed'], 2)
            self.assertEqual(clicks, [('click', 160, 140)])
            self.assertEqual(execute.call_args.args[1],
                             ['xdotool', 'key', 'F1', 'sleep', '0.1', 'key', 'F1'])
            journal = (root / 'campaign' / 'steps.ndjson').read_text()
            self.assertNotIn('Bearer', journal)
            self.assertIn('left_click', journal)
            self.assertEqual(script.last[2], ['left_click 160 140'])
            first = json.loads(journal.splitlines()[0])
            self.assertEqual(first['label'], 'campaign-0000')
            self.assertEqual(first['step'], 0)

    def test_play_right_click_uses_button_three(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'source.json').write_text('{}\n')

            def fake_capture(args):
                dest = root / 'captures'
                dest.mkdir(exist_ok=True)
                target = dest / f'{args.label}.png'
                Image.new('RGB', (80, 60), (1, 2, 3)).save(target)
                return {'capture': str(target), 'sha256': 'def'}

            answers = jev_answers(act=choice('right_click'))
            del answers['target']
            del answers['drag_end']
            answers['cell'] = choice('r0c0', 0.8)
            answers['drag_cell'] = choice('r0c0', 0.8)

            class Script:
                def decide(self, jpeg, campaign, step, recent, elements=None):
                    return {'answers': answers}

            with patch.object(agent.ref, 'capture', side_effect=fake_capture), \
                    patch.object(agent.ref, 'execute', return_value='') as execute:
                result = agent.play_steps(
                    root, steps=1, campaign='tft-human', provider='openrouter',
                    model=agent.OPENROUTER_MODEL, pause_s=0, client=Script())
            self.assertEqual(result['status'], 'step_limit')
            self.assertEqual(result['last_act'], 'right_click')
            argv = execute.call_args.args[1]
            self.assertEqual(argv[0], 'xdotool')
            self.assertEqual(argv[1:3], ['key', 'F1'])
            self.assertEqual(argv.count('F1'), 1)
            self.assertIn('Shift', argv)
            self.assertIn('3', argv)
