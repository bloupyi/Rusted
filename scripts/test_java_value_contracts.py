#!/usr/bin/env python3
"""Compare original value facades against local, unmodified Minecraft/Fabric binaries.

No third-party implementation source is inspected. All compiled inputs stay in build/.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
IDENTIFIER = '''
import net.minecraft.resources.Identifier;
public class IdentifierContract {
 public static void main(String[] args) {
  for (String value : new String[]{"", ":stone", "minecraft:stone", "copper:ore/block", "bad:Upper", "a:b:c", "a:../x", "a:../../x", "a:/root", "a:", ""}) {
   try {
    var id = Identifier.parse(value);
    System.out.println(value + "=" + id + "|" + id.getNamespace() + "|" + id.getPath() + "|" + id.hashCode() + "|" + id.toDebugFileName() + "|" + id.toShortString());
    System.out.println(id.equals(Identifier.tryParse(value)) + "|" + id.withPrefix("pre/") + "|" + id.withSuffix(".json"));
    try { System.out.println("resolved=" + id.resolveAgainst(java.nio.file.Path.of("test-root"))); }
    catch (Exception error) { System.out.println(error.getClass().getSimpleName()); }
   } catch (Exception error) { System.out.println(value + "=" + error.getClass().getSimpleName()); }
  }
  for (String namespace : new String[]{"", "minecraft", "copper", "BAD", "a/b"}) {
   System.out.println(namespace + "|" + Identifier.isValidNamespace(namespace) + "|" + Identifier.tryBuild(namespace, "path"));
  }
  System.out.println("order=" + Identifier.parse("z:a").compareTo(Identifier.parse("a:z")));
  for (char c : new char[]{'a', 'Z', '0', '-', '.', '/', ':', '\\u00e9'}) System.out.println((int)c + "|" + Identifier.validPathChar(c) + "|" + Identifier.isAllowedInIdentifier(c));
 }
}
'''
VERSIONS = '''
import net.fabricmc.loader.api.*;
public class VersionContract {
 public static void main(String[] args) throws Exception {
  for (String[] pair : new String[][]{{"1.9","1.10"},{"1.0","1.0.0"},{"1.0-rc.2","1.0-rc.10"},{"1.0+one","1.0+two"},{"1.0-beta","1.0"},{"1.0-1","1.0-alpha"},{"1.0-alpha","1.0-alpha.1"}}) {
   Version a = Version.parse(pair[0]), b = Version.parse(pair[1]);
   System.out.println(pair[0] + "|" + pair[1] + "|" + Integer.signum(a.compareTo(b)) + "|equal=" + a.equals(b) + "|sameHash=" + (a.hashCode() == b.hashCode()));
  }
  for (String text : new String[]{"26.3", "26.3.x", "1.0-rc.1+build", "release-copper", "", "1..0", "1.0-", "*", "1.0.01", "1.0-alpha_foo", "1.0+build_foo", "01.02.003-beta.01+001", "1.0+", "1.0+a..b", "1.0+hello+there", "1.0+hello world"}) {
   try {
    Version v = Version.parse(text);
    System.out.println("repeatEqual=" + v.equals(Version.parse(text)));
    System.out.println("parse=" + text + "|" + v.getFriendlyString() + "|" + (v instanceof SemanticVersion));
    if (v instanceof SemanticVersion s) System.out.println(s.getVersionComponentCount() + "|" + s.getVersionComponent(2) + "|" + s.hasWildcard() + "|" + s.getPrereleaseKey() + "|" + s.getBuildKey());
   } catch (VersionParsingException error) { System.out.println("parse=" + text + "|invalid"); }
   try { SemanticVersion.parse(text); System.out.println("semantic=valid"); }
   catch (VersionParsingException error) { System.out.println("semantic=invalid"); }
  }
 }
}
'''

ADAPTER = '''
import net.fabricmc.loader.api.LanguageAdapter;
public class AdapterContract {
 public interface Echo { String call(String text); }
 public interface Multi { void a(); void b(); }
 public static class Entries {
  public static int calls;
  public static final Runnable FIELD=() -> calls+=1;
  public final Runnable NONSTATIC=() -> {};
  public static final String WRONG="wrong";
  public static final Runnable NULL=null;
  public static void staticCall() {calls+=10;}
  public void instanceCall() {calls+=100;}
  public static String echo(String text) {return "echo="+text;}
  public static void overload(){}
  public static void overload(int x){}
  private static void hidden(){}
  public void badArgument(int x){}
 }
 public static void main(String[] args) throws Exception {
 try {
  Class<?> launcher=Class.forName("net.fabricmc.loader.impl.launch.FabricLauncher");
  Object reference=java.lang.reflect.Proxy.newProxyInstance(launcher.getClassLoader(),new Class<?>[]{launcher},(proxy,method,values)-> {
   if (method.getName().equals("getTargetClassLoader")) return AdapterContract.class.getClassLoader();
   throw new UnsupportedOperationException(method.getName());
  });
  Class.forName("net.fabricmc.loader.impl.launch.FabricLauncherBase").getMethod("setLauncher",launcher).invoke(null,reference);
  } catch (ClassNotFoundException absent) { }
  var adapter=LanguageAdapter.getDefault();
  for (String member:new String[]{"FIELD","NONSTATIC","WRONG","NULL","staticCall","instanceCall","overload","hidden","badArgument","missing","staticCall::extra"}) {
   try {var value=adapter.create(null,"AdapterContract$Entries::"+member,Runnable.class); if(value!=null)value.run();System.out.println(member+"=ok:"+(value==null)+":"+Entries.calls);}
   catch(Throwable error){System.out.println(member+"=error");}
  }
  try {System.out.println(adapter.create(null,"AdapterContract$Entries::echo",Echo.class).call("copper"));}catch(Throwable error){System.out.println("echo=error");}
  try {adapter.create(null,"AdapterContract$Entries::staticCall",Multi.class);System.out.println("multi=ok");}catch(Throwable error){System.out.println("multi=error");}
 }
}
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--probe', type=Path, required=True)
    parser.add_argument('--java-home', type=Path, required=True)
    parser.add_argument('--minecraft-jar', type=Path, required=True)
    parser.add_argument('--libraries', type=Path, required=True)
    parser.add_argument('--fabric-loader', type=Path, required=True)
    args = parser.parse_args()
    (ROOT / 'build').mkdir(exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='java-value-contracts-', dir=ROOT / 'build'))
    api = root / 'rusted-api.jar'
    subprocess.run([str(args.probe.resolve()), '--export-api', str(api)], check=True)
    java = args.java_home.resolve() / ('bin/java.exe' if os.name == 'nt' else 'bin/java')
    javac = args.java_home.resolve() / ('bin/javac.exe' if os.name == 'nt' else 'bin/javac')
    classes = root / 'classes'
    libraries = list(args.libraries.resolve().rglob('*.jar'))
    report = {}
    for name, source, reference in [('IdentifierContract', IDENTIFIER, [args.minecraft_jar.resolve(), *libraries]),
                                     ('VersionContract', VERSIONS, [args.fabric_loader.resolve()]),
                                     ('AdapterContract', ADAPTER, [args.fabric_loader.resolve()])]:
        file = root / (name + '.java')
        file.write_text(source, encoding='utf-8')
        subprocess.run([str(javac), '--release', '17', '-encoding', 'UTF-8', '-cp',
                        os.pathsep.join(map(str, reference)), '-d', str(classes), str(file)], check=True)
        traces = {}
        for kind, classpath in [('reference', reference), ('rusted', [api])]:
            home = root / (name + '-' + kind)
            home.mkdir()
            env = os.environ.copy()
            for key in ['HORIZON_HOME', 'HORIZON_LAUNCHER_HOME', 'XDG_DATA_HOME', 'XDG_CONFIG_HOME',
                        'HORIZON_LAUNCHER_TRASH', 'HORIZON_LAUNCHER_CLIPBOARD', 'HORIZON_LAUNCHER_DESKTOP']:
                env[key] = str(home)
            env.update(HORIZON_NO_AUDIO='1', HORIZON_LAUNCHER_NO_SELF_UPDATE='1')
            result = subprocess.run([str(java), '-Djava.awt.headless=true', '-cp',
                os.pathsep.join(map(str, [classes, *classpath])), name], env=env, cwd=home,
                capture_output=True, text=True, timeout=60, check=True)
            traces[kind] = result.stdout
            (home / 'trace.txt').write_text(result.stdout)
        if traces['reference'] != traces['rusted']:
            import difflib
            raise AssertionError(name + '\n' + ''.join(difflib.unified_diff(
                traces['reference'].splitlines(True), traces['rusted'].splitlines(True), 'reference', 'rusted')))
        report[name] = {'passed': True, 'reference_trace_lines': len(traces['reference'].splitlines())}
    (root / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'scratch': str(root), 'result': report}, indent=2))


if __name__ == '__main__':
    main()
