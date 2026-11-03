#!/usr/bin/env python3
"""Measure complete local-pack reloads in native Rusted and an installed Java 26.3.

Developer tool only. Uses disposable profiles, no Microsoft account, and the same
ZIP in both clients. Requires a graphical display (Xvfb works) and a JDK 25.
Minecraft files stay in the user's existing local installation.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import statistics
import subprocess
import tempfile
import time
from zipfile import ZipFile

from test_horizon_native import NativeGame, ROOT


# Original instrumentation against public Minecraft methods. No loader or mod
# implementation is inspected or included. Classfile is the standard JDK API.
AGENT = r'''
import java.lang.instrument.*;
import java.lang.classfile.*;
import java.lang.constant.*;
import static java.lang.constant.ConstantDescs.*;
import java.security.ProtectionDomain;
import java.util.*;
import java.util.concurrent.CompletableFuture;
public class RustedReloadAgent {
 static volatile boolean busy;
 static volatile int completed;
 static volatile long after;
 static String pack;
 static int repetitions;
 public static void premain(String arg, Instrumentation instrumentation) {
  String[] parts=arg.split(","); pack=parts[0]; repetitions=Integer.parseInt(parts[1]);
  instrumentation.addTransformer(new ClassFileTransformer() {
   public byte[] transform(ClassLoader loader,String name,Class<?> type,ProtectionDomain domain,byte[] bytes) {
    if (!name.equals("net/minecraft/client/Minecraft")) return null;
    return ClassFile.of().transformClass(ClassFile.of().parse(bytes),ClassTransform.transformingMethodBodies(m -> m.methodName().equalsString("tick") && m.methodType().equalsString("()V"),new CodeTransform() {
     public void atStart(CodeBuilder builder) { builder.aload(0).invokestatic(ClassDesc.of("RustedReloadAgent"),"tick",MethodTypeDesc.of(CD_void,CD_Object)); }
     public void accept(CodeBuilder builder, CodeElement element) { builder.with(element); }
    }));
   }
  });
 }
 public static void tick(Object game) {
  if (busy || System.nanoTime()<after) return;
  try {
   Class<?> type=game.getClass();
   if (!(Boolean)type.getMethod("isGameLoadFinished").invoke(game)) return;
   if (completed>=repetitions) { type.getMethod("stop").invoke(game); return; }
   Object repository=type.getMethod("getResourcePackRepository").invoke(game);
   if (completed==0) {
    repository.getClass().getMethod("reload").invoke(repository);
    Collection<?> available=(Collection<?>)repository.getClass().getMethod("getAvailableIds").invoke(repository);
    if (!available.contains(pack)) throw new IllegalArgumentException("Pack absent: "+pack+" available="+available);
    repository.getClass().getMethod("setSelected",Collection.class).invoke(repository,List.of("vanilla",pack));
   }
   busy=true;
   long start=System.nanoTime();
   CompletableFuture<?> reload=(CompletableFuture<?>)type.getMethod("reloadResourcePacks").invoke(game);
   reload.whenComplete((value,error) -> {
    if (error!=null) { error.printStackTrace(); System.exit(3); }
    System.out.println("RUSTED_RELOAD_BENCH "+completed+" "+((System.nanoTime()-start)/1e6));
    completed++;after=System.nanoTime()+500_000_000L;busy=false;
   });
  } catch (Throwable failure) { failure.printStackTrace(); System.exit(2); }
 }
}
'''


def java_reload(args, scratch):
    agent_dir = scratch / "agent"
    agent_dir.mkdir()
    (agent_dir / "RustedReloadAgent.java").write_text(AGENT)
    (agent_dir / "MANIFEST.MF").write_text("Premain-Class: RustedReloadAgent\n\n")
    executable = ".exe" if os.name == "nt" else ""
    subprocess.run([str(args.java_home / "bin" / ("javac" + executable)),
                    "RustedReloadAgent.java"], cwd=agent_dir, check=True)
    jar = agent_dir / "reload-agent.jar"
    subprocess.run([str(args.java_home / "bin" / ("jar" + executable)), "cfm", str(jar),
                    "MANIFEST.MF", *[p.name for p in sorted(agent_dir.glob("*.class"))]],
                   cwd=agent_dir, check=True)
    home = scratch / "java"
    (home / "resourcepacks").mkdir(parents=True)
    shutil.copyfile(args.pack, home / "resourcepacks/benchmark.zip")
    (home / "options.txt").write_text(
        "enableVsync:false\nmaxFps:60\nrenderDistance:2\nonboardAccessibility:false\n")
    jars = [args.minecraft_home / "versions/26.3/26.3.jar",
            *sorted((args.minecraft_home / "libraries").rglob("*.jar"))]
    if not jars[0].is_file():
        raise RuntimeError("A local Minecraft 26.3 client is required")
    command = [str(args.java_home / "bin" / ("java" + executable)), "-Xmx2G",
               "--enable-native-access=ALL-UNNAMED", f"-javaagent:{jar}=file/benchmark.zip,{args.repetitions}",
               "-cp", os.pathsep.join(map(str, jars)), "net.minecraft.client.main.Main",
               "--username", "ReloadFixture", "--version", "26.3", "--gameDir", str(home),
               "--assetsDir", str(args.minecraft_home / "assets"), "--assetIndex", args.asset_index,
               "--accessToken", "0", "--width", "960", "--height", "540"]
    log = home / "console.log"
    with log.open("w") as output:
        process = subprocess.run(command, cwd=home, stdout=output,
                                 stderr=subprocess.STDOUT, timeout=240)
    text = log.read_text(errors="replace")
    samples = [(int(i), float(ms)) for i, ms in re.findall(r"RUSTED_RELOAD_BENCH (\d+) ([\d.]+)", text)]
    if process.returncode or [i for i, _ in samples] != list(range(args.repetitions)):
        raise RuntimeError(f"Java reference did not complete every reload; inspect {log}")
    backend = next((line.split("Using graphics backend ", 1)[1] for line in text.splitlines()
                    if "Using graphics backend " in line), "unknown")
    return {"milliseconds": [ms for _, ms in samples], "backend": backend}


def native_reload(args, scratch):
    work = scratch / "native"
    home = work / "game"
    (home / "resourcepacks").mkdir(parents=True)
    shutil.copyfile(args.pack, home / "resourcepacks/benchmark.zip")
    game = NativeGame(args.binary, args.resources, work, home, True)
    samples = []
    try:
        game.wait(lambda state: state.get("menu") is not None)
        deadline = time.monotonic() + 180
        while not samples:
            try:
                result = game.command("resource-pack file/benchmark.zip")
                samples.append(result["total_ms"])
            except RuntimeError as error:
                if "Initial resources are still loading" not in str(error) or time.monotonic() > deadline:
                    raise
                time.sleep(0.25)
        for _ in range(args.repetitions - 1):
            time.sleep(0.5)
            samples.append(game.command("resource-pack file/benchmark.zip")["total_ms"])
    finally:
        game.close()
    return {"milliseconds": samples}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["pack", "java-home", "minecraft-home", "resources", "binary"]:
        parser.add_argument("--" + name, required=True, type=Path)
    parser.add_argument("--asset-index", default="34")
    parser.add_argument("--repetitions", type=int, default=3)
    parser.add_argument("--report", type=Path, default=ROOT / "build/resource-reload-comparison.json")
    args = parser.parse_args()
    if args.repetitions < 2:
        parser.error("At least two repetitions separate first activation from warm reloads")
    for name in ["pack", "java_home", "minecraft_home", "resources", "binary"]:
        setattr(args, name, getattr(args, name).resolve())
    scratch = Path(tempfile.mkdtemp(prefix="reload-comparison-", dir=ROOT / "build"))
    (scratch / "runtime").mkdir(mode=0o700)
    os.environ["XDG_RUNTIME_DIR"] = str(scratch / "runtime")
    with ZipFile(args.pack) as pack:
        files = len(pack.infolist())
    report = {"schema": 1, "kind": "complete-local-pack-reload", "account_used": False,
              "minecraft": "26.3", "pack_sha256": hashlib.sha256(args.pack.read_bytes()).hexdigest(),
              "zip_bytes": args.pack.stat().st_size, "zip_entries": files,
              "includes": "local selection, reload listeners, CPU resources and GPU application",
              "excludes": "download and initial game startup", "scratch": str(scratch)}
    print("Java reference", flush=True)
    report["java"] = java_reload(args, scratch)
    print("Native Rusted", flush=True)
    report["rusted"] = native_reload(args, scratch)
    for client in ["java", "rusted"]:
        values = report[client]["milliseconds"]
        report[client]["first_activation_ms"] = values[0]
        report[client]["warm_median_ms"] = statistics.median(values[1:])
    report["warm_rusted_over_java"] = report["rusted"]["warm_median_ms"] / report["java"]["warm_median_ms"]
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
