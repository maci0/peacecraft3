#!/usr/bin/env python3
"""Drive a running original Warcraft III reference from a vision decision model.

Jev 1.14 is the default. It is the hosted decision model that returns a box, so
one frame becomes one click. OpenRouter vision decision models such as
Perplexity Decider return a label, so they choose a cell on a fixed grid.
The harness owns the window. The model only answers the questions below.
"""
import argparse
import base64
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time
import urllib.error
import urllib.request
import uuid

HERE = Path(__file__).resolve().parent
_SPEC = importlib.util.spec_from_file_location('reference', HERE / 'reference.py')
ref = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(ref)

JEV_URL = 'https://api.typesafe.ai/v1/systemone'
OPENROUTER_URL = 'https://openrouter.ai/api/alpha/decisions'
JEV_MODEL = 'jev-1.14.0'
OPENROUTER_MODEL = 'perplexity/pplx-decider-v1.2'
FRAME_ASSET = 'frame'
MIN_EDGE = 280
MAX_EDGE = 1568
JPEG_QUALITY = 80
MAX_FRAME_BYTES = 32 * 1024 * 1024
MAX_RESPONSE_BYTES = 2 * 1024 * 1024
DECISION_TIMEOUT_S = 60
MIN_ACT_CONFIDENCE = 0.22
MIN_PRESENT = 0.50
MIN_POINT_CONFIDENCE = 0.08
MIN_CELL_CONFIDENCE = 0.12
PLAYFIELD_Y_MAX = 0.78
MINIMAP_X_MAX = 0.22
MINIMAP_Y_MIN = 0.62
MIN_ELEMENT_CONFIDENCE = 0.30
MAX_BOX_AREA = 0.35
MAX_OCR_ELEMENTS = 40
OCR_MIN_CONFIDENCE = 55.0
MIN_TERMINAL = 0.80
GRID_ROWS = 6
GRID_COLS = 8
RECENT_LIMIT = 8
MAX_STEPS = 20000
MODEL_RE = re.compile(r'^[A-Za-z0-9_.:/+-]{1,160}$')

CAMPAIGNS = {
    'roc-prologue': 'Reign of Chaos prologue',
    'roc-human': 'Reign of Chaos human campaign',
    'roc-undead': 'Reign of Chaos undead campaign',
    'roc-orc': 'Reign of Chaos orc campaign',
    'roc-nightelf': 'Reign of Chaos night elf campaign',
    'tft-nightelf': 'The Frozen Throne night elf campaign',
    'tft-human': 'The Frozen Throne human campaign',
    'tft-undead': 'The Frozen Throne undead campaign',
}
SCENES = {
    'main_menu': 'the title or main menu',
    'campaign_select': 'a campaign or race selection screen',
    'mission_select': 'a mission or chapter selection screen',
    'briefing': 'a mission briefing',
    'loading': 'a loading screen',
    'cinematic': 'a cinematic or conversation without a command card',
    'gameplay': 'an active mission with a command card or minimap',
    'dialog': 'a modal dialog',
    'single_player': 'the single player profile screen, with campaign buttons on the right',
    'victory': 'a mission victory screen',
    'defeat': 'a defeat or game-over screen',
    'other': 'none of the listed scenes',
}
ACTS = {
    'advance': 'advance the idle army along an unexplored path in the chosen direction',
    'left_click': 'left click the pointer target',
    'right_click': 'right click the pointer target',
    'drag': 'drag from the pointer target to the drag end',
    'hotkey': 'press the chosen hotkey',
    'type': 'click the pointer target and type the chosen text',
    'wait': 'wait during loading, conversation, movement or combat; choose advance if the army is idle',
    'stop': 'stop the session without quitting the game',
}
DIRECTIONS = {
    'north': (0.5, 0.15), 'northeast': (0.8, 0.2), 'east': (0.85, 0.4),
    'southeast': (0.8, 0.6), 'south': (0.5, 0.65),
    'southwest': (0.25, 0.6), 'west': (0.15, 0.4), 'northwest': (0.2, 0.2),
}
TEXTS = {
    'Jev': 'the profile name Jev',
}
FORBIDDEN_CONTROLS = (
    'quit', 'exit', 'delete', 'battle.net', 'battlenet', 'options', 'credits',
    'local area', 'view replay', 'custom', 'load saved', 'cancel', 'video',
)
HOTKEYS = {
    'Escape': 'cancel the current order or close a menu',
    'Return': 'confirm the focused button',
    'space': 'center the camera on the latest alert',
    'Left': 'pan the camera left',
    'Right': 'pan the camera right',
    'Up': 'pan the camera up',
    'Down': 'pan the camera down',
    'F1': 'select the primary hero',
    'F10': 'open the in-game menu',
    'Page_Up': 'scroll a list upward',
    'Page_Down': 'scroll a list downward',
    'a': 'attack',
    'm': 'move',
    's': 'stop',
    'h': 'hold position',
    'b': 'open the build menu for a selected worker',
    'p': 'patrol',
}


class Failure(Exception):
    pass


class Parser(argparse.ArgumentParser):
    def error(self, message):
        raise Failure(message)


def goal_for(campaign):
    try:
        label = CAMPAIGNS[campaign]
    except KeyError:
        raise Failure(f'unknown campaign {campaign}') from None
    return (
        f'Play the original Warcraft III {label} through every mission to victory. '
        'The human campaign is titled The Scourge of Lordaeron. '
        'If the profile name field is empty, type Jev and then click Create. '
        'If a profile named Jev already exists, click Campaign, scroll until '
        'The Scourge of Lordaeron is visible, and open that campaign. '
        'If Sentinels, Terror of the Tides, Alliance, Scourge, or The Founding of Durotar '
        'is open, press F10, quit that mission, and return to the campaign list. '
        'Do not start those campaigns. '
        'Read the visible quest objectives and follow them. '
        'Right-click enemies to attack and ground to move the selected army. '
        'World orders recenter on the hero and include nearby soldiers. '
        'Explore unexplored paths toward the objective, choosing ground ahead of the army '
        'instead of repeatedly clicking its current position. Pan with arrow keys when needed. '
        'Wait while the army is moving or fighting. Do not attack friendly units. '
        'Click Continue on briefings and the victory screen. '
        'Do not click Quit, Exit, Cancel, Delete, Battle.net, Options, Credits, Load, Replay, or Custom.'
    )


def forbidden_control(text):
    lowered = text.lower()
    return any(needle in lowered for needle in FORBIDDEN_CONTROLS)


def cell_name(row, col):
    return f'r{row}c{col}'


def cell_criteria():
    criteria = {}
    for row in range(GRID_ROWS):
        for col in range(GRID_COLS):
            criteria[cell_name(row, col)] = (
                f'row {row} of {GRID_ROWS}, column {col} of {GRID_COLS}, origin at the top left'
            )
    return criteria


CELL_CRITERIA = cell_criteria()


def element_criteria(elements):
    criteria = {'none': 'no listed text element is the right target'}
    for element in elements or []:
        criteria[element['id']] = f"{element['text']} at x={element['x']} y={element['y']}"
    return criteria


def questions(spatial, elements=None):
    if spatial not in ('box', 'grid'):
        raise Failure(f'unknown spatial mode {spatial}')
    shared = {
        'scene': {
            'type': 'choice',
            'instructions': 'What kind of Warcraft III screen is visible?',
            'criteria': SCENES,
        },
        'act': {
            'type': 'choice',
            'instructions': 'Which single next action best advances the campaign goal?',
            'criteria': ACTS,
        },
        'direction': {
            'type': 'choice',
            'instructions': 'Looking at `frame`, which direction follows an open road away from the idle army into unexplored terrain? Used only for advance.',
            'criteria': {name: name for name in DIRECTIONS},
        },
        'present': {
            'type': 'noul',
            'instructions': 'A clickable control, unit, or terrain target for the campaign goal is visible.',
            'criteria': {'true': 'A target is visible', 'false': 'No target is visible'},
        },
        'hotkey': {
            'type': 'choice',
            'instructions': 'Which hotkey to press when the next action is hotkey. Otherwise pick Escape.',
            'criteria': HOTKEYS,
        },
        'text': {
            'type': 'choice',
            'instructions': 'Which text to type when the next action is type. Otherwise pick Jev.',
            'criteria': TEXTS,
        },
        'element': {
            'type': 'choice',
            'instructions': (
                'Which visible text element should receive the click. '
                'Pick the control that advances the campaign. Pick none when the target is a unit or the ground.'
            ),
            'criteria': element_criteria(elements),
        },
        'victory': {
            'type': 'noul',
            'instructions': 'The mission victory screen is visible.',
            'criteria': {'true': 'Victory is visible', 'false': 'Victory is not visible'},
        },
        'defeat': {
            'type': 'noul',
            'instructions': 'The mission defeat or game-over screen is visible.',
            'criteria': {'true': 'Defeat is visible', 'false': 'Defeat is not visible'},
        },
    }
    if spatial == 'box':
        shared['target'] = {
            'type': 'box',
            'instructions': f'the single best next pointer target in `{FRAME_ASSET}`',
        }
        shared['drag_end'] = {
            'type': 'box',
            'instructions': f'where a drag that starts at the pointer target should end in `{FRAME_ASSET}`',
        }
        shared['cell'] = {
            'type': 'choice',
            'instructions': (
                f'The frame is split into {GRID_ROWS} equal rows and {GRID_COLS} equal columns. '
                'Row 0 is the top and column 0 is the left. '
                'Choose the cell that contains the next world target, such as an enemy unit or ground to move onto. '
                'Do not choose the bottom row or the minimap.'
            ),
            'criteria': CELL_CRITERIA,
        }
    else:
        shared['cell'] = {
            'type': 'choice',
            'instructions': (
                f'The frame is split into {GRID_ROWS} equal rows and {GRID_COLS} equal columns. '
                'Row 0 is the top and column 0 is the left. Choose the cell that contains the next pointer target.'
            ),
            'criteria': CELL_CRITERIA,
        }
        shared['drag_cell'] = {
            'type': 'choice',
            'instructions': 'Choose the cell where a drag should end. Use the same cell as the pointer target when not dragging.',
            'criteria': CELL_CRITERIA,
        }
    return shared


def fit_size(width, height):
    if width < 1 or height < 1:
        raise Failure('frame has no pixels')
    scale = 1.0
    if max(width, height) > MAX_EDGE:
        scale = MAX_EDGE / max(width, height)
    if min(width, height) * scale < MIN_EDGE:
        scale = MIN_EDGE / min(width, height)
    fitted = (max(1, round(width * scale)), max(1, round(height * scale)))
    if any(edge < MIN_EDGE or edge > MAX_EDGE for edge in fitted):
        raise Failure(f'frame {width}x{height} cannot fit the {MIN_EDGE}..{MAX_EDGE} image limit')
    return fitted


def prepare_frame(data):
    if len(data) > MAX_FRAME_BYTES:
        raise Failure('frame exceeds 32 MiB')
    from PIL import Image
    from io import BytesIO
    with Image.open(BytesIO(data)) as image:
        width, height = image.size
        fitted = fit_size(width, height)
        rgb = image.convert('RGB')
        if rgb.size != fitted:
            rgb = rgb.resize(fitted, Image.Resampling.LANCZOS)
        buffer = BytesIO()
        rgb.save(buffer, format='JPEG', quality=JPEG_QUALITY)
    return buffer.getvalue(), width, height


def default_model(provider):
    if provider == 'jev':
        return JEV_MODEL
    if provider == 'openrouter':
        return OPENROUTER_MODEL
    raise Failure(f'unknown provider {provider}')


def spatial_for(provider):
    if provider == 'jev':
        return 'box'
    if provider == 'openrouter':
        return 'grid'
    raise Failure(f'unknown provider {provider}')


def check_model(model):
    if not MODEL_RE.fullmatch(model):
        raise Failure('model id contains unsupported characters')
    return model


def state_text(campaign, step, recent):
    history = ', '.join(recent) if recent else 'none'
    return (
        f'goal: {goal_for(campaign)}\n'
        f'campaign: {campaign}\n'
        f'step: {step}\n'
        f'recent actions: {history}'
    )


def jev_request(key, model, jpeg, campaign, step, recent, elements=None):
    encoded = base64.standard_b64encode(jpeg).decode('ascii')
    asset_ref = uuid.uuid4().hex
    body = {
        'model': check_model(model),
        'state': {
            'goal': goal_for(campaign),
            'campaign': campaign,
            'step': step,
            'recent_actions': list(recent),
            'visible_text': [f"{item['id']} {item['text']}" for item in (elements or [])],
            FRAME_ASSET: asset_ref,
        },
        'assets': {asset_ref: {'type': 'image', 'format': 'jpeg', 'data_base64': encoded}},
        'questions': questions('box', elements),
    }
    headers = {
        'Authorization': f'Bearer {key}',
        'Content-Type': 'application/json',
        'TypeSafe-Danger-Zone-Images': 'true',
    }
    return body, headers


def openrouter_request(key, model, jpeg, campaign, step, recent, elements=None):
    encoded = base64.standard_b64encode(jpeg).decode('ascii')
    visible = ', '.join(f"{item['id']} {item['text']}" for item in (elements or [])) or 'none'
    body = {
        'model': check_model(model),
        'state': [
            state_text(campaign, step, recent) + f'\nvisible text: {visible}',
            {'type': 'image_url', 'image_url': {'url': f'data:image/jpeg;base64,{encoded}'}},
        ],
        'questions': questions('grid', elements),
    }
    headers = {'Authorization': f'Bearer {key}', 'Content-Type': 'application/json'}
    return body, headers


def post_json(url, headers, body, timeout=DECISION_TIMEOUT_S):
    request = urllib.request.Request(
        url, data=json.dumps(body).encode(), headers=headers, method='POST')
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            raw = response.read(MAX_RESPONSE_BYTES + 1)
    except urllib.error.HTTPError as error:
        detail = error.read(4000).decode(errors='replace')
        raise Failure(f'decision request failed with HTTP {error.code}: {detail[:500]}') from None
    except urllib.error.URLError as error:
        raise Failure(f'decision request failed: {error.reason}') from None
    if len(raw) > MAX_RESPONSE_BYTES:
        raise Failure('decision response exceeds 2 MiB')
    try:
        return json.loads(raw)
    except json.JSONDecodeError as error:
        raise Failure(f'decision response is not JSON: {error}') from None


def answers_of(payload):
    if not isinstance(payload, dict):
        raise Failure('decision response is not an object')
    if isinstance(payload.get('answers'), dict):
        return payload['answers']
    data = payload.get('data')
    if isinstance(data, dict):
        result = data.get('result') if isinstance(data.get('result'), dict) else data
        if isinstance(result.get('answers'), dict):
            return result['answers']
    raise Failure('decision response has no answers')


def _choice(answer, allowed):
    if not isinstance(answer, dict):
        raise Failure('choice answer missing')
    choice = answer.get('choice')
    if choice not in allowed:
        raise Failure(f'choice {choice!r} is outside the allowed set')
    confidence = answer.get('confidence')
    if confidence is None:
        probabilities = answer.get('probabilities')
        if isinstance(probabilities, dict) and choice in probabilities:
            confidence = probabilities[choice]
        else:
            confidence = 0
    return choice, float(confidence)


def _noul(answer):
    if not isinstance(answer, dict):
        raise Failure('noul answer missing')
    value = answer.get('noul', answer.get('probability'))
    if isinstance(value, bool):
        return 1.0 if value else 0.0
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        return float(value)
    raise Failure('noul answer has no probability')


def _box_point(answer, src_w, src_h):
    if not isinstance(answer, dict) or not isinstance(answer.get('box'), dict):
        raise Failure('box answer missing')
    box = answer['box']
    try:
        edges = {name: float(box[name]) for name in ('top', 'bottom', 'left', 'right')}
    except (KeyError, TypeError, ValueError) as error:
        raise Failure(f'box edges are incomplete: {error}') from None
    if any(not 0 <= value <= 1 for value in edges.values()):
        raise Failure('box edge is outside 0..1')
    if edges['right'] <= edges['left'] or edges['bottom'] <= edges['top']:
        raise Failure('degenerate box')
    confidence = float(answer.get('confidence') or 0)
    area = (edges['right'] - edges['left']) * (edges['bottom'] - edges['top'])
    x = min(src_w - 1, max(0, int((edges['left'] + edges['right']) / 2 * src_w)))
    y = min(src_h - 1, max(0, int((edges['top'] + edges['bottom']) / 2 * src_h)))
    return x, y, confidence, area


def playfield_point(name, src_w, src_h):
    x, y = cell_center(name, src_w, src_h)
    if y > src_h * PLAYFIELD_Y_MAX:
        return None
    if y > src_h * MINIMAP_Y_MIN and x < src_w * MINIMAP_X_MAX:
        return None
    return x, y


def cell_center(name, src_w, src_h):
    match = re.fullmatch(r'r(\d+)c(\d+)', name or '')
    if not match:
        raise Failure('cell name is not rNcM')
    row, col = int(match.group(1)), int(match.group(2))
    if not (0 <= row < GRID_ROWS and 0 <= col < GRID_COLS):
        raise Failure('cell is outside the grid')
    x = min(src_w - 1, max(0, int((col + 0.5) * src_w / GRID_COLS)))
    y = min(src_h - 1, max(0, int((row + 0.5) * src_h / GRID_ROWS)))
    return x, y


def _blank(scene, act_name, act_confidence, present, victory, defeat):
    return {
        'scene': scene,
        'proposed_act': act_name,
        'act': 'wait',
        'x': None,
        'y': None,
        'x2': None,
        'y2': None,
        'hotkey': None,
        'confidence': {
            'act': act_confidence,
            'point': 0.0,
            'present': present,
            'victory': victory,
            'defeat': defeat,
        },
        'status': 'continue',
        'gate': None,
    }


def group_ocr_tsv(tsv, limit=MAX_OCR_ELEMENTS):
    words = []
    for line in tsv.splitlines()[1:]:
        parts = line.split('\t')
        if len(parts) < 12:
            continue
        try:
            confidence = float(parts[10])
            left, top, width, height = (int(parts[index]) for index in (6, 7, 8, 9))
        except ValueError:
            continue
        word = parts[11].strip()
        if confidence < OCR_MIN_CONFIDENCE or not word or width < 2 or height < 2:
            continue
        words.append((top, left, width, height, word))
    words.sort()
    lines = []
    for top, left, width, height, word in words:
        if lines and abs(top - lines[-1]['top']) <= 8:
            line = lines[-1]
            line['text'] += ' ' + word
            line['right'] = max(line['right'], left + width)
            line['bottom'] = max(line['bottom'], top + height)
            line['left'] = min(line['left'], left)
        else:
            lines.append({
                'top': top, 'left': left, 'right': left + width,
                'bottom': top + height, 'text': word,
            })
    elements = []
    for index, line in enumerate(lines[:limit]):
        elements.append({
            'id': f'e{index}',
            'text': line['text'][:80],
            'x': (line['left'] + line['right']) // 2,
            'y': (line['top'] + line['bottom']) // 2,
        })
    return elements


def read_elements(image_path, limit=MAX_OCR_ELEMENTS):
    # tesseract is an external binary. The Python stdlib has no OCR API.
    result = subprocess.run(
        ['tesseract', str(image_path), 'stdout', '--psm', '11', '-l', 'eng', 'tsv'],
        capture_output=True, timeout=20, check=False)
    if result.returncode != 0:
        return []
    return group_ocr_tsv(result.stdout.decode(errors='replace'), limit=limit)


def interpret(answers, src_w, src_h, spatial, elements=None):
    if src_w < 1 or src_h < 1:
        raise Failure('frame has no pixels')
    required = ('scene', 'act', 'present', 'hotkey', 'victory', 'defeat')
    pointing = ('target', 'drag_end') if spatial == 'box' else ('cell', 'drag_cell')
    missing = [name for name in (*required, *pointing) if name not in answers]
    if missing:
        raise Failure(f'decision is missing {", ".join(missing)}')
    scene, _scene_confidence = _choice(answers['scene'], SCENES)
    victory = _noul(answers['victory'])
    defeat = _noul(answers['defeat'])
    present = _noul(answers['present'])
    act_name, act_confidence = _choice(answers['act'], ACTS)
    hotkey, hotkey_confidence = _choice(answers['hotkey'], HOTKEYS)
    catalog = {element['id']: element for element in (elements or [])}
    if 'element' in answers:
        element_id, element_confidence = _choice(answers['element'], {'none', *catalog})
    else:
        element_id, element_confidence = 'none', 0.0
    text_value, text_confidence = (None, 0.0)
    if 'text' in answers:
        text_value, text_confidence = _choice(answers['text'], TEXTS)
    decision = _blank(scene, act_name, act_confidence, present, victory, defeat)
    decision['width'] = src_w
    decision['height'] = src_h
    decision['hotkey'] = hotkey
    decision['text'] = text_value
    decision['target_text'] = None
    if victory >= MIN_TERMINAL and victory >= defeat:
        decision.update(act='wait', status='victory', gate='victory')
        return decision
    if defeat >= MIN_TERMINAL:
        decision.update(act='wait', status='defeat', gate='defeat')
        return decision
    if act_name == 'stop' and act_confidence >= MIN_ACT_CONFIDENCE:
        decision.update(act='stop', status='stopped', gate=None)
        return decision
    if act_confidence < MIN_ACT_CONFIDENCE:
        decision['gate'] = 'low_act_confidence'
        return decision
    if act_name == 'advance':
        if scene != 'gameplay':
            decision['gate'] = 'not_gameplay'
            return decision
        direction, confidence = _choice(answers.get('direction'), DIRECTIONS)
        if confidence < MIN_ACT_CONFIDENCE:
            decision['gate'] = 'low_direction_confidence'
            return decision
        x, y = DIRECTIONS[direction]
        decision.update(act='right_click', x=int(x * src_w), y=int(y * src_h),
                        gate='navigation_order', direction=direction)
        decision['confidence']['point'] = confidence
        return decision
    if act_name == 'wait':
        decision['act'] = 'wait'
        return decision
    if act_name == 'hotkey':
        if hotkey_confidence < MIN_ACT_CONFIDENCE:
            decision['gate'] = 'low_hotkey_confidence'
            return decision
        decision['act'] = 'hotkey'
        return decision
    if act_name == 'type' and (text_value is None or text_confidence < MIN_ACT_CONFIDENCE):
        decision['gate'] = 'low_text_confidence'
        return decision
    chosen = catalog.get(element_id)
    if act_name == 'left_click' and chosen is not None and element_confidence >= MIN_ELEMENT_CONFIDENCE:
        decision['target_text'] = chosen['text']
        decision['x'], decision['y'] = chosen['x'], chosen['y']
        decision['confidence']['point'] = element_confidence
        if forbidden_control(chosen['text']):
            decision['gate'] = 'forbidden_control'
            return decision
        decision['act'] = 'left_click'
        return decision
    if present < MIN_PRESENT:
        decision['gate'] = 'target_not_present'
        return decision
    try:
        if spatial == 'box':
            x, y, point_confidence, area = _box_point(answers['target'], src_w, src_h)
            x2, y2, end_confidence, _end_area = _box_point(answers['drag_end'], src_w, src_h)
        elif spatial == 'grid':
            cell, point_confidence = _choice(answers['cell'], CELL_CRITERIA)
            end_name, end_confidence = _choice(answers['drag_cell'], CELL_CRITERIA)
            x, y = cell_center(cell, src_w, src_h)
            x2, y2 = cell_center(end_name, src_w, src_h)
        else:
            raise Failure(f'unknown spatial mode {spatial}')
    except Failure as error:
        decision['gate'] = 'bad_point'
        decision['point_error'] = str(error)
        return decision
    decision['x'], decision['y'] = x, y
    decision['x2'], decision['y2'] = x2, y2
    decision['confidence']['point'] = point_confidence
    in_playfield = y <= src_h * PLAYFIELD_Y_MAX and not (
        y > src_h * MINIMAP_Y_MIN and x < src_w * MINIMAP_X_MAX)
    weak_box = point_confidence < MIN_POINT_CONFIDENCE or not in_playfield or (
        spatial == 'box' and area > MAX_BOX_AREA)
    if weak_box and 'cell' in answers and act_name in ('left_click', 'right_click'):
        cell_name, cell_confidence = _choice(answers['cell'], CELL_CRITERIA)
        point = playfield_point(cell_name, src_w, src_h) if cell_confidence >= MIN_CELL_CONFIDENCE else None
        if point is not None:
            decision['x'], decision['y'] = point
            decision['act'] = act_name
            decision['confidence']['point'] = cell_confidence
            decision['gate'] = None
            return decision
    if spatial == 'box' and area > MAX_BOX_AREA:
        decision['gate'] = 'oversized_box'
        return decision
    if point_confidence < MIN_POINT_CONFIDENCE or not in_playfield:
        decision['gate'] = 'low_point_confidence'
        return decision
    if act_name == 'drag' and end_confidence < MIN_POINT_CONFIDENCE:
        decision['gate'] = 'low_drag_confidence'
        return decision
    decision['act'] = act_name
    return decision


def summary(decision):
    if decision['act'] == 'hotkey':
        return f"hotkey {decision['hotkey']}"
    if decision['act'] in ('left_click', 'right_click'):
        return f"{decision['act']} {decision['x']} {decision['y']}"
    if decision['act'] == 'drag':
        return f"drag {decision['x']} {decision['y']} {decision['x2']} {decision['y2']}"
    if decision['act'] == 'type':
        return f"type {decision['text']} {decision['x']} {decision['y']}"
    if decision.get('target_text'):
        return f"{decision['act']} {decision['target_text']} {decision['x']} {decision['y']}"
    return decision['act']


def perform(path, decision):
    act = decision['act']
    if act == 'wait':
        return ref.mark(path, 'campaign-wait', {'step_act': 'wait'})
    if act == 'left_click':
        return ref.action(argparse.Namespace(
            command='click', run_dir=str(path), x=decision['x'], y=decision['y']))
    if act == 'right_click':
        width, height = int(decision['width']), int(decision['height'])
        command = [
            'xdotool', 'key', 'F1',
            'keydown', 'Shift',
            'mousemove', str(int(width * 0.22)), str(int(height * 0.20)),
            'mousedown', '1',
            'mousemove', str(int(width * 0.78)), str(int(height * 0.62)),
            'mouseup', '1',
            'keyup', 'Shift',
            'mousemove', str(decision['x']), str(decision['y']),
            'sleep', '0.12', 'mousedown', '3', 'sleep', '0.12', 'mouseup', '3']
        ref.execute(path, command)
        return ref.mark(path, 'campaign-right-click', {'x': decision['x'], 'y': decision['y']})
    if act == 'drag':
        ref.execute(path, ['xdotool', 'mousemove', str(decision['x']), str(decision['y']),
                           'sleep', '0.2', 'mousedown', '1', 'mousemove',
                           str(decision['x2']), str(decision['y2']), 'sleep', '0.2', 'mouseup', '1'])
        return ref.mark(path, 'campaign-drag', {
            'x': decision['x'], 'y': decision['y'], 'x2': decision['x2'], 'y2': decision['y2']})
    if act == 'hotkey':
        return ref.action(argparse.Namespace(
            command='key', run_dir=str(path), key=decision['hotkey']))
    if act == 'type':
        if decision.get('text') not in TEXTS:
            raise Failure('typed text is outside the allowed set')
        ref.action(argparse.Namespace(
            command='click', run_dir=str(path), x=decision['x'], y=decision['y']))
        # xdotool is the input path the reference container already uses.
        ref.execute(path, ['xdotool', 'type', '--delay', '40', decision['text']])
        return ref.mark(path, 'campaign-type', {
            'x': decision['x'], 'y': decision['y'], 'text': decision['text']})
    raise Failure(f'no performer for {act}')


def next_label(path):
    captures = path / 'captures'
    number = 0
    while (captures / f'campaign-{number:04d}.png').exists():
        number += 1
        if number > MAX_STEPS:
            raise Failure('too many campaign captures')
    return f'campaign-{number:04d}', number


def prior_actions(journal):
    if not journal.is_file():
        return []
    recent = []
    for line in journal.read_text().splitlines():
        if not line.strip():
            continue
        row = json.loads(line)
        if row.get('act'):
            recent.append(summary(row))
    return recent[-RECENT_LIMIT:]


def append_jsonl(path, value):
    line = json.dumps(value, sort_keys=True)
    if len(line) > MAX_RESPONSE_BYTES:
        raise Failure('campaign journal line exceeds 2 MiB')
    with path.open('a') as output:
        output.write(line + '\n')


class DecisionClient:
    def __init__(self, provider, key, model, post=post_json):
        self.provider = provider
        self.key = key
        self.model = check_model(model)
        self.post = post

    def decide(self, jpeg, campaign, step, recent, elements=None):
        if self.provider == 'jev':
            body, headers = jev_request(
                self.key, self.model, jpeg, campaign, step, recent, elements)
            url = JEV_URL
        elif self.provider == 'openrouter':
            body, headers = openrouter_request(
                self.key, self.model, jpeg, campaign, step, recent, elements)
            url = OPENROUTER_URL
        else:
            raise Failure(f'unknown provider {self.provider}')
        try:
            return self.post(url, headers, body)
        except Failure as error:
            raise Failure(str(error).replace(self.key, '[redacted]')) from None


def play_steps(path, *, steps, campaign, provider, model, pause_s, client):
    if not 1 <= steps <= MAX_STEPS:
        raise Failure(f'steps must be 1..{MAX_STEPS}')
    if not 0 <= pause_s <= 30:
        raise Failure('pause must be 0..30 seconds')
    goal_for(campaign)
    spatial = spatial_for(provider)
    root = ref.local_run(path)
    journal = root / 'campaign' / 'steps.ndjson'
    journal.parent.mkdir(exist_ok=True)
    recent = prior_actions(journal)
    last = None
    status = 'step_limit'
    completed = 0
    for _ in range(steps):
        completed += 1
        if last is not None and last['scene'] == 'gameplay':
            ref.execute(root, ['xdotool', 'key', 'F1', 'sleep', '0.1', 'key', 'F1'])
        label, index = next_label(root)
        shot = ref.capture(argparse.Namespace(run_dir=str(root), label=label, command='capture'))
        jpeg, src_w, src_h = prepare_frame(Path(shot['capture']).read_bytes())
        elements = read_elements(shot['capture'])
        payload = client.decide(jpeg, campaign, index, recent, elements)
        decision = interpret(answers_of(payload), src_w, src_h, spatial, elements)
        record = {
            'step': index,
            'label': label,
            'capture': shot['capture'],
            'capture_sha256': shot.get('sha256'),
            'provider': provider,
            'model': payload.get('model', model),
            'usage': payload.get('usage'),
            'answers': answers_of(payload),
            **decision,
        }
        if decision['status'] == 'continue':
            record['input'] = perform(root, decision)
            recent.append(summary(decision))
            del recent[:-RECENT_LIMIT]
        else:
            record['input'] = None
            status = decision['status']
            append_jsonl(journal, record)
            last = record
            break
        append_jsonl(journal, record)
        last = record
        if pause_s:
            time.sleep(pause_s)
    return {
        'status': status,
        'steps_completed': completed,
        'campaign': campaign,
        'provider': provider,
        'model': model,
        'spatial': spatial,
        'journal': str(journal),
        'last_act': None if last is None else last['act'],
        'last_gate': None if last is None else last['gate'],
    }


def read_secret_file(path):
    if not path.is_file():
        raise Failure(f'missing secret file {path.name}')
    text = path.read_text().strip()
    if not text:
        raise Failure(f'empty secret file {path.name}')
    if text.startswith('{'):
        try:
            obj = json.loads(text)
        except json.JSONDecodeError as error:
            raise Failure(f'{path.name} is not valid JSON: {error}') from None
        for name in ('api_key', 'key', 'token', 'TYPESAFE_API_KEY', 'OPENROUTER_API_KEY'):
            value = obj.get(name) if isinstance(obj, dict) else None
            if isinstance(value, str) and value.strip():
                return value.strip()
        raise Failure(f'{path.name} JSON has no key field')
    line = text.splitlines()[0].strip()
    if '=' in line:
        line = line.split('=', 1)[1].strip().strip('"').strip("'")
    if not line:
        raise Failure(f'empty secret file {path.name}')
    return line


def load_key(provider, environ=None, secrets_dir=None):
    environ = os.environ if environ is None else environ
    secrets = Path.home() / '.secrets' if secrets_dir is None else Path(secrets_dir)
    if provider == 'jev':
        for name in ('TYPESAFE_API_KEY', 'JEV_API_KEY'):
            if environ.get(name, '').strip():
                return environ[name].strip()
        return read_secret_file(secrets / 'typesafe.ai')
    if provider == 'openrouter':
        if environ.get('OPENROUTER_API_KEY', '').strip():
            return environ['OPENROUTER_API_KEY'].strip()
        return read_secret_file(secrets / 'openrouter')
    raise Failure(f'unknown provider {provider}')


def decide_image(image, *, campaign, provider, model, client):
    jpeg, src_w, src_h = prepare_frame(Path(image).read_bytes())
    elements = read_elements(image)
    payload = client.decide(jpeg, campaign, 0, [], elements)
    decision = interpret(answers_of(payload), src_w, src_h, spatial_for(provider), elements)
    return {
        'provider': provider,
        'model': payload.get('model', model),
        'campaign': campaign,
        'usage': payload.get('usage'),
        **decision,
    }


def parser():
    cli = Parser(description=__doc__)
    cli.add_argument('--format', choices=['json', 'text'], default='json')
    commands = cli.add_subparsers(dest='command', required=True)
    commands.add_parser('schema', help='offline command contract')
    decide = commands.add_parser('decide')
    decide.add_argument('--image', required=True)
    play = commands.add_parser('play')
    play.add_argument('--run-dir', required=True)
    play.add_argument('--steps', type=step_count, required=True)
    play.add_argument('--pause', type=pause_value, default=1.0)
    for command in (decide, play):
        command.add_argument('--provider', choices=['jev', 'openrouter'], default='jev')
        command.add_argument('--model', default=None)
        command.add_argument('--campaign', choices=sorted(CAMPAIGNS), default='roc-human')
    return cli


def step_count(value):
    number = int(value)
    if not 1 <= number <= MAX_STEPS:
        raise argparse.ArgumentTypeError(f'must be 1..{MAX_STEPS}')
    return number


def pause_value(value):
    number = float(value)
    if not 0 <= number <= 30:
        raise argparse.ArgumentTypeError('must be 0..30 seconds')
    return number


def selected_model(args):
    return check_model(args.model) if args.model else default_model(args.provider)


def main(argv=None):
    try:
        args = parser().parse_args(argv)
        if args.command == 'schema':
            result = json.loads((HERE / 'campaign-schema.json').read_text())
        elif args.command == 'decide':
            image = Path(args.image)
            if not image.is_file():
                raise Failure(f'image not found: {image}')
            model = selected_model(args)
            client = DecisionClient(args.provider, load_key(args.provider), model)
            result = decide_image(
                image, campaign=args.campaign, provider=args.provider, model=model, client=client)
        else:
            model = selected_model(args)
            client = DecisionClient(args.provider, load_key(args.provider), model)
            result = play_steps(
                args.run_dir, steps=args.steps, campaign=args.campaign, provider=args.provider,
                model=model, pause_s=args.pause, client=client)
        print(json.dumps(result) if args.format == 'json' else json.dumps(result, indent=2))
        return 0
    except KeyboardInterrupt:
        print(json.dumps({'error': {
            'kind': 'campaign_interrupted',
            'message': 'Interrupted. Already sent input stays sent. Inspect the campaign journal before another run.',
            'retryable': False,
        }}), file=sys.stderr)
        return 130
    except (Failure, ref.Failure, OSError, ValueError) as error:
        print(json.dumps({'error': {
            'kind': 'campaign_failed', 'message': str(error), 'retryable': False,
        }}), file=sys.stderr)
        return 2


if __name__ == '__main__':
    sys.exit(main())
