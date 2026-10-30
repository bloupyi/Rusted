#!/usr/bin/env python3
"""Play an original passive module in both native execution paths, with private test worlds."""
import argparse
import json
import os
from pathlib import Path
import tempfile
import time
import zipfile
from test_horizon_native import NativeGame, ROOT


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=ROOT / 'build/rusted-linux-target/fast/rusted')
    parser.add_argument('--resources', type=Path, required=True)
    args = parser.parse_args()
    root = Path(tempfile.mkdtemp(prefix='rusted-native-modules-', dir=ROOT / 'build'))
    reports = []
    owned_keys = ['HORIZON_SERVER_THREAD', 'RUSTED_HOME', 'RUSTED_INSTANCE_DIR']
    originals = {key: os.environ.get(key) for key in owned_keys}
    try:
        for threaded in (True, False):
            scratch = root / ('server-thread' if threaded else 'in-frame')
            home = scratch / 'game'
            (home / 'mods').mkdir(parents=True)
            with zipfile.ZipFile(home / 'mods/own_resources.jar', 'w') as archive:
                archive.writestr('fabric.mod.json', json.dumps({
                    'schemaVersion': 1, 'id': 'own_resources', 'depends': {'minecraft': '26.3'}}))
                archive.writestr('assets/minecraft/lang/en_us.json', json.dumps({
                    'menu.singleplayer': 'Rusted module QA'}))
                archive.writestr('data/own_resources/function/load.mcfunction',
                    'scoreboard objectives add rusted_module dummy\n'
                    'scoreboard players set ModuleMarker rusted_module 42\n')
                archive.writestr('data/minecraft/tags/function/load.json', json.dumps({
                    'values': ['own_resources:load']}))
            os.environ.update(HORIZON_SERVER_THREAD='1' if threaded else '0',
                              RUSTED_HOME=str(home), RUSTED_INSTANCE_DIR=str(home))
            report = {'server_thread': threaded, 'passed': False, 'account_tested': False}
            game = NativeGame(args.binary.resolve(), args.resources.resolve(), scratch, home, True)
            try:
                game.wait(lambda state: state.get('menu') is not None, seconds=180)
                report['title_ui'] = game.command('ui')
                assert 'Rusted module QA' in json.dumps(report['title_ui'])
                game.command('open RustedModuleQA 1 creative')
                state = game.wait(lambda state: state.get('world') is not None and
                                  state.get('player') is not None, seconds=180)
                x, y, z = state['player']['block_pos']
                marker = [x + 2, y, z + 2]
                game.wait_block(marker)
                game.command('command execute if score ModuleMarker rusted_module matches 42 '
                             'run setblock ' + ' '.join(map(str, marker)) + ' minecraft:copper_block')
                report['module_function_marker'] = game.wait_block(marker, 'minecraft:copper_block')
                game.command('command fill ~-3 ~ ~-3 ~3 ~5 ~3 minecraft:air')
                game.command('command fill ~-3 ~-1 ~-3 ~3 ~-1 ~3 minecraft:stone')
                game.command('command give @s minecraft:stick['
                             'minecraft:custom_name={text:"Copper QA",italic:false,color:"gold"}]')
                deadline = time.monotonic() + 10
                while time.monotonic() < deadline:
                    inventory = game.command('inventory')
                    if 'italic:0b' in json.dumps(inventory):
                        break
                    time.sleep(.1)
                assert 'italic:0b' in json.dumps(inventory), inventory
                report['inventory'] = inventory
                report['screenshot'] = game.screenshot('native-module-world')
                game.command('save')
                game.close()
                report['passed'] = True
            except BaseException as error:
                report['error'] = str(error)
                if game.process.poll() is None:
                    game.process.kill()
                    game.process.wait()
                raise
            finally:
                reports.append(report)
                (root / 'validation.json').write_text(json.dumps(reports, indent=2) + '\n')
                print('Native module QA:', scratch, report['passed'], flush=True)
    finally:
        for key, original in originals.items():
            if original is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = original
    print(root / 'validation.json', flush=True)


if __name__ == '__main__':
    main()
