#!/usr/bin/env python3
"""Execute unchanged developer Fabric JARs against the native JVM host, with real 26.3 API signatures.

The Minecraft JAR and libraries are local developer inputs and are never copied to public artifacts.
An optional published YOSBR JAR is executed unchanged and its configuration-copy behavior checked.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[1]
SOURCES = {
    'TickMod': '''package fixture;
import net.fabricmc.api.ModInitializer;
import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.network.chat.Component;
import java.nio.charset.StandardCharsets;
public final class TickMod implements ModInitializer, ClientModInitializer {
 private static int initialized;
 private Thread initialThread;
 private int ticks;
 public void onInitialize() { initialized = 7; }
 public void onInitializeClient() {
  if (initialized != 7) throw new AssertionError("Initializer order or shared class state lost");
  try (var input = getClass().getResourceAsStream("/fixture/payload.txt")) {
   if (!new String(input.readAllBytes(), StandardCharsets.UTF_8).equals("unchanged JAR resources")) throw new AssertionError("JAR resource unavailable");
  } catch (java.io.IOException error) { throw new AssertionError(error); }
  initialThread = Thread.currentThread();
  var invoker = ClientTickEvents.END_CLIENT_TICK.invoker();
  if (!invoker.equals(invoker) || invoker.hashCode() != invoker.hashCode()) throw new AssertionError("Invoker object contract");
  ClientTickEvents.END_CLIENT_TICK.register(client -> {
   if (Thread.currentThread() != initialThread || !client.isSameThread()) throw new AssertionError("Client thread identity changed");
   if (client.player != null && ++ticks == 60)
    client.execute(() -> client.player.sendSystemMessage(Component.literal("JAR callback: tick=60 health=" + client.player.getHealth() + " time=" + client.level.getLevelData().getGameTime())));
  });
 }
}''',
    'MissingApi': '''package fixture;
public final class MissingApi implements net.fabricmc.api.ClientModInitializer {
 public void onInitializeClient() {
  try { Class.forName("net.minecraft.client.Options"); }
  catch (Exception error) { throw new RuntimeException(error); }
 }
}''',
    'BadTick': '''package fixture;
public final class BadTick implements net.fabricmc.api.ClientModInitializer {
 public void onInitializeClient() {
  net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents.END_CLIENT_TICK.register(client -> { throw new IllegalStateException("bad tick once"); });
 }
}''',
}


def fixture(root, java_home, minecraft, libraries, api):
    source = root / 'source/fixture'
    source.mkdir(parents=True)
    for name, body in SOURCES.items():
        (source / f'{name}.java').write_text(body)
    classes = root / 'classes'
    compiler = java_home / ('bin/javac.exe' if os.name == 'nt' else 'bin/javac')
    # Minecraft first: bytecode calls are checked against the authentic mapped signatures.
    cp = os.pathsep.join(map(str, [minecraft, api, *libraries]))
    subprocess.run([str(compiler), '--release', '17', '-encoding', 'UTF-8', '-cp', cp,
        '-d', str(classes), *map(str, source.glob('*.java'))], check=True)
    game = root / 'profil cuivre é;测试'
    mods = game / 'mods'
    mods.mkdir(parents=True)
    for name, module, entries in [
        ('TickMod', 'fixture_tick', {'main': ['fixture.TickMod'], 'client': ['fixture.TickMod']}),
        ('MissingApi', 'missing_api', {'client': ['fixture.MissingApi']}),
        ('BadTick', 'bad_tick', {'client': ['fixture.BadTick']}),
    ]:
        with zipfile.ZipFile(mods / (module + '.jar'), 'x') as archive:
            archive.writestr('fabric.mod.json', json.dumps({'schemaVersion': 1, 'id': module,
                'name': module, 'version': '1.0.0', 'environment': 'client', 'entrypoints': entries}))
            for file in (classes / 'fixture').glob(name + '*.class'):
                archive.write(file, 'fixture/' + file.name)
            archive.writestr('fixture/payload.txt', 'unchanged JAR resources')
    with zipfile.ZipFile(mods / 'unsupported-mixin.jar', 'x') as archive:
        archive.writestr('fabric.mod.json', json.dumps({'schemaVersion': 1, 'id': 'mixin_test', 'mixins': ['x.json']}))
    return game


def run(probe, java_home, game, libraries, extra_classpath=()):
    env = os.environ.copy()
    for key in ['HORIZON_HOME', 'HORIZON_LAUNCHER_HOME', 'HORIZON_LAUNCHER_TRASH',
                'HORIZON_LAUNCHER_CLIPBOARD', 'HORIZON_LAUNCHER_DESKTOP', 'XDG_DATA_HOME', 'XDG_CONFIG_HOME']:
        env[key] = str(game / key)
    env.update(HORIZON_NO_AUDIO='1', HORIZON_LAUNCHER_NO_SELF_UPDATE='1')
    result = subprocess.run([str(probe), str(java_home), str(game), str(game / 'cache'),
        *map(str, [*libraries, *extra_classpath])], env=env, capture_output=True, text=True, timeout=60)
    (game / 'probe.log').write_text(result.stdout + result.stderr)
    if result.returncode:
        raise RuntimeError(result.stdout + result.stderr)
    return result.stdout + result.stderr


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--probe', type=Path, required=True)
    parser.add_argument('--java-home', type=Path, default=Path(os.environ.get('JAVA_HOME', '')))
    parser.add_argument('--minecraft-jar', type=Path, help='Optional authentic Minecraft 26.3 client JAR for ABI validation')
    parser.add_argument('--libraries', type=Path)
    parser.add_argument('--yosbr', type=Path)
    args = parser.parse_args()
    if os.name == 'nt' and not args.probe.exists() and not args.probe.suffix:
        args.probe = args.probe.with_suffix('.exe')
    for key in ('probe', 'java_home', 'minecraft_jar', 'libraries'):
        if getattr(args, key) is not None:
            setattr(args, key, getattr(args, key).resolve())
    (ROOT / 'build').mkdir(exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='java-mod-runtime-', dir=ROOT / 'build'))
    api = root / 'native-api.jar'
    subprocess.run([str(args.probe), '--export-api', str(api)], check=True)
    libraries = sorted(args.libraries.rglob('*.jar')) if args.libraries else []
    game = fixture(root, args.java_home, args.minecraft_jar or api, libraries, api)
    output = run(args.probe, args.java_home, game, libraries, [args.minecraft_jar] if args.minecraft_jar else [])
    assert 'CHAT [fixture_tick] JAR callback: tick=60 health=17.5' in output, output
    assert output.count('ERROR bad_tick:') == 1, output
    assert output.count('ERROR missing_api:') == 1, output
    assert 'Rusted native API not implemented: net.minecraft.client.Options' in output, output
    assert 'REJECTED unsupported-mixin.jar' in output, output
    assert 'ERROR fixture_tick' not in output, output
    report = {'fixture_bytecode_compiled_against_minecraft_26_3': args.minecraft_jar is not None,
        'unchanged_jar_code_and_resources_executed': True, 'client_tick_snapshot_and_chat': True,
        'client_thread_identity_preserved': True, 'failed_mod_isolated': True,
        'minecraft_java_class_fallback_blocked': True, 'mixins_rejected': True,
        'unicode_and_semicolon_paths': True, 'yosbr': None}
    if args.yosbr:
        game = root / 'yosbr'
        (game / 'mods').mkdir(parents=True)
        body = args.yosbr.read_bytes()
        assert hashlib.sha512(body).hexdigest() == '254f2225f6446523aa8c8274e83b31e27f3b2afc8fe4f490fb1b169dd518e10c7993acda5db0ab597474ddf9d69506b5e794242fada1e2f89ed7bf9d412a8f78', 'Expected the published YOSBR 0.1.2 fixture'
        (game / 'mods/yosbr.jar').write_bytes(body)
        defaults = game / 'config/yosbr'
        defaults.mkdir(parents=True)
        (defaults / 'options.txt').write_text('native_java_default:true\n')
        output = run(args.probe, args.java_home, game, libraries)
        assert 'ERROR yosbr:' not in output, output
        assert (game / 'options.txt').read_text() == 'native_java_default:true\n'
        (game / 'options.txt').write_text('preserve_player_setting:true\n')
        run(args.probe, args.java_home, game, libraries)
        assert (game / 'options.txt').read_text() == 'preserve_player_setting:true\n'
        report['yosbr'] = {'sha512': hashlib.sha512(body).hexdigest(), 'default_options_copied': True,
                           'existing_options_preserved': True, 'jar_unmodified': True}
    (root / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'scratch': str(root), 'results': report}, indent=2))


if __name__ == '__main__':
    main()
