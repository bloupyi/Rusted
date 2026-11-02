#!/usr/bin/env python3
"""Exercise unchanged Java mod bytecode in a real native world, in both execution paths."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import tempfile
import time
from test_horizon_native import NativeGame, ROOT


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--resources', type=Path, required=True)
    parser.add_argument('--fixture-jar', type=Path, required=True)
    parser.add_argument('--yosbr', type=Path, required=True)
    parser.add_argument('--java-home', type=Path, required=True)
    parser.add_argument('--libraries', type=Path, required=True)
    args = parser.parse_args()
    (ROOT / 'build').mkdir(exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='native-java-game-', dir=ROOT / 'build'))
    keys = ['HORIZON_SERVER_THREAD', 'RUSTED_HOME', 'RUSTED_INSTANCE_DIR', 'RUSTED_JAVA_MODS',
            'RUSTED_JAVA_HOME', 'RUSTED_JAVA_LIBRARIES_FILE', 'HORIZON_SESSION_FILE',
            'HORIZON_LAUNCHER_TRASH', 'HORIZON_LAUNCHER_CLIPBOARD', 'HORIZON_LAUNCHER_DESKTOP']
    original = {key: os.environ.get(key) for key in keys}
    reports = []
    try:
        for threaded in [True, False]:
            scratch = root / ('server-thread' if threaded else 'in-frame')
            home = scratch / 'game'
            (home / 'mods').mkdir(parents=True)
            shutil.copy2(args.fixture_jar, home / 'mods/fixture_tick.jar')
            shutil.copy2(args.yosbr, home / 'mods/yosbr.jar')
            (home / 'config/yosbr').mkdir(parents=True)
            (home / 'config/yosbr/options.txt').write_text('fullscreen:false\n')
            manifest = home / 'java-libraries.json'
            manifest.write_text(json.dumps([str(p.resolve()) for p in sorted(args.libraries.rglob('*.jar'))]))
            os.environ.update(HORIZON_SERVER_THREAD='1' if threaded else '0',
                RUSTED_HOME=str(home), RUSTED_INSTANCE_DIR=str(home), RUSTED_JAVA_MODS='1',
                RUSTED_JAVA_HOME=str(args.java_home.resolve()), RUSTED_JAVA_LIBRARIES_FILE=str(manifest),
                HORIZON_LAUNCHER_TRASH=str(scratch / 'trash'), HORIZON_LAUNCHER_CLIPBOARD=str(scratch / 'clipboard'),
                HORIZON_LAUNCHER_DESKTOP=str(scratch / 'desktop'))
            os.environ.pop('HORIZON_SESSION_FILE', None)
            game = NativeGame(args.binary.resolve(), args.resources.resolve(), scratch, home, True)
            result = {'server_thread': threaded, 'passed': False, 'account_used': False}
            try:
                game.wait(lambda state: state.get('menu') is not None, seconds=180)
                log = (scratch / 'console.log').read_text()
                assert 'Java entrypoints initialized for 2 module' in log, log[-3000:]
                assert 'Applying default options' in log and 'YOSBR' in log, log[-3000:]
                assert (home / 'options.txt').is_file()
                if Path('/proc').exists():
                    result['jvm_in_native_process'] = 'libjvm.so' in Path(f'/proc/{game.process.pid}/maps').read_text()
                    assert result['jvm_in_native_process']
                game.command('open JavaJarQA 1 creative')
                state = game.wait(lambda state: state.get('world') is not None and state.get('player') is not None, seconds=180)
                expected = 'health=' + str(float(state['player']['health']))
                deadline = time.monotonic() + 120
                while time.monotonic() < deadline:
                    chat = game.command('client-chat-log 50')
                    if 'JAR callback: tick=60' in json.dumps(chat):
                        break
                    time.sleep(.2)
                else:
                    raise AssertionError('Java callback did not reach the native client chat')
                assert expected in json.dumps(chat), chat
                assert 'ERROR fixture_tick' not in (scratch / 'console.log').read_text()
                result.update(chat=chat, native_player_health=state['player']['health'],
                    yosbr_original_sha512=hashlib.sha512(args.yosbr.read_bytes()).hexdigest())
                game.command('key t tap')
                result['screenshot'] = game.screenshot('unchanged-java-jar-in-native-world')
                game.command('save')
                game.close()
                result['passed'] = True
            finally:
                if game.process.poll() is None:
                    game.process.kill()
                    game.process.wait()
                reports.append(result)
                (root / 'result.json').write_text(json.dumps(reports, indent=2) + '\n')
            print(json.dumps({'scratch': str(scratch), 'passed': result['passed']}), flush=True)
    finally:
        for key, value in original.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value
    print(root / 'result.json', flush=True)


if __name__ == '__main__':
    main()
