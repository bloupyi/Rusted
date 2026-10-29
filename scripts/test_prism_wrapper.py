#!/usr/bin/env python3
"""Regression against Prism 10.0.5's real command splitter (Qt Core, no GUI/account)."""
import argparse
import hashlib
import json
from pathlib import Path
import shlex
import subprocess
import shutil
import tempfile
import urllib.request

from package_horizon_native import PRISM_WRAPPER

REVISION = "16e541f4483241455ffb674bfe975e20158001a2"
SOURCES = {
    "Commandline.cpp": "d4e1792a66c62ce728371f6a991f285290b9a6304ba894efe2843bfc3f9b875f",
    "Commandline.h": "7786e7e7aed5aa8cb32f934eb03575cf2baae420b349bec0bbfd5895be54b7d6",
}
HARNESS = r'''
#include "Commandline.h"
#include <QJsonArray>
#include <QJsonDocument>
#include <QSettings>
#include <iostream>
int main(int argc, char **argv) {
    QString command;
    if (argc == 3 && QString::fromUtf8(argv[1]) == "--ini") {
        QSettings settings(QString::fromUtf8(argv[2]), QSettings::IniFormat);
        settings.setIniCodec("UTF-8");
        if (!settings.value("OverrideCommands").toBool()) return 3;
        command = settings.value("WrapperCommand").toString();
    } else if (argc == 2) command = QString::fromUtf8(argv[1]);
    else return 2;
    QJsonArray output;
    for (const auto &part : Commandline::splitArgs(command)) output.append(part);
    std::cout << QJsonDocument(output).toJson(QJsonDocument::Compact).constData();
}
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prism-source", type=Path, help="Optional local checkout of the pinned Prism version")
    parser.add_argument("--qt-prefix", type=Path, help="Local extracted qtbase5-dev usr directory, when not installed system-wide")
    parser.add_argument("--bridge", type=Path, help="Also configure a private instance with this Windows bridge and verify its INI through Qt")
    parser.add_argument("--installer", type=Path, help="Verify the actual preconfigured instance created by the GUI installer CLI hook")
    parser.add_argument("--wine", type=Path)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="rusted-prism-splitter-") as temp:
        work = Path(temp)
        for name, checksum in SOURCES.items():
            if args.prism_source:
                body = (args.prism_source / "launcher" / name).read_bytes()
            else:
                url = f"https://raw.githubusercontent.com/PrismLauncher/PrismLauncher/{REVISION}/launcher/{name}"
                body = urllib.request.urlopen(url, timeout=30).read()
            if hashlib.sha256(body).hexdigest() != checksum:
                raise ValueError("Unexpected Prism source: " + name)
            (work / name).write_bytes(body)
        (work / "test.cpp").write_text(HARNESS)
        if args.qt_prefix:
            includes = args.qt_prefix / "include/x86_64-linux-gnu/qt5"
            qt = ["-I" + str(includes), "-I" + str(includes / "QtCore"), "-l:libQt5Core.so.5"]
        else:
            qt = shlex.split(subprocess.check_output(["pkg-config", "--cflags", "--libs", "Qt5Core"], text=True))
        binary = work / "splitter"
        subprocess.run(["g++", "-std=c++17", "-fPIC", str(work / "test.cpp"), str(work / "Commandline.cpp"), "-o", str(binary), *qt], check=True)
        def split(command):
            return json.loads(subprocess.check_output([str(binary), command], text=True))
        directory = r"C:\Users\Player\AppData\Roaming\PrismLauncher\instances\Rusted-Native-Prism-0.3.1\minecraft"
        old = '"$INST_MC_DIR/rusted/rusted-launch.exe" --config "$INST_MC_DIR/rusted/rusted-runtime.json" --'
        assert "\\" not in split(old.replace("$INST_MC_DIR", directory))[0], "Old bug no longer reproduced"
        assert split(PRISM_WRAPPER.replace("$INST_MC_DIR", directory)) == [directory + "/rusted/rusted-launch.exe", "--"]
        # The recommended absolute command also handles spaces and apostrophes.
        for path in ["C:/Users/Player/Prism/instance/minecraft", "C:/Users/Jean Dupont/Prism/Rusted Native/minecraft", "C:/Users/O'Neil/Prism/Rusted Native/minecraft"]:
            command = f'"{path}/rusted/rusted-launch.exe" --'
            assert split(command) == [path + "/rusted/rusted-launch.exe", "--"]
        # Do not advertise the variable form as handling paths with spaces.
        assert len(split(PRISM_WRAPPER.replace("$INST_MC_DIR", r"C:\Users\Jean Dupont\Rusted Native\minecraft"))) != 2
        print("Prism 10.0.5 real splitter: old failure reproduced; package and absolute-path corrections passed (6 scenarios).")
        if args.bridge:
            instance = work / "Prism O'Neil #1 with spaces"
            runtime = instance / "minecraft/rusted"
            runtime.mkdir(parents=True)
            bridge = runtime / "rusted-launch.exe"
            shutil.copy2(args.bridge, bridge)
            original = "[General]\nConfigVersion=1.3\nname=Fixture instance\nOverrideCommands=false\nWrapperCommand=old\nJavaPath=C:/Java/javaw.exe\n"
            ini = instance / "instance.cfg"
            ini.write_text(original)
            (instance / "mmc-pack.json").write_text(json.dumps({"components":[{"uid":"net.minecraft","version":"26.3"}]}))
            path = "Z:" + str(bridge).replace("/", "\\") if args.wine else str(bridge)
            process = subprocess.run(([str(args.wine), path] if args.wine else [path]) + ["--setup"], capture_output=True, text=True, timeout=30)
            assert process.returncode == 0, process.stderr
            tokens = json.loads(subprocess.check_output([str(binary), "--ini", str(ini)], text=True))
            expected = "Z:" + str(bridge) if args.wine else str(bridge).replace("\\", "/")
            assert tokens == [expected, "--"], tokens
            assert "name=Fixture instance" in ini.read_text() and "JavaPath=C:/Java/javaw.exe" in ini.read_text()
            backups = list(instance.glob("instance.cfg.rusted-backup-*.ini"))
            assert len(backups) == 1 and backups[0].read_text() == original
            print("Automatic Prism setup: real bridge -> saved INI -> Qt settings -> real Prism splitter passed; original settings backed up.")

        if args.installer:
            instances = work / "Prism instances O'Neil #1 with spaces"
            instances.mkdir()
            def windows(path):
                return "Z:" + str(path.resolve()).replace("/", "\\") if args.wine else str(path)
            command = ([str(args.wine)] if args.wine else []) + [windows(args.installer), "--destination", windows(instances)]
            result = subprocess.run(command, capture_output=True, text=True, timeout=90)
            assert result.returncode == 0, result.stderr
            instance = instances / "Rusted-0.3.4"
            runtime = instance / "minecraft/rusted"
            ini = instance / "instance.cfg"
            tokens = json.loads(subprocess.check_output([str(binary), "--ini", str(ini)], text=True))
            expected = windows(runtime / "rusted-launch.exe").replace("\\", "/")
            assert tokens == [expected, "--"], tokens
            assert json.loads((runtime / "rusted-runtime.json").read_text())["mode"] == "auto"
            marker = runtime / "native-data/player-world-marker.txt"
            marker.parent.mkdir(exist_ok=True)
            marker.write_text("Preserve existing data")
            original = ini.read_bytes()
            result = subprocess.run(command, capture_output=True, text=True, timeout=30)
            assert result.returncode != 0
            assert marker.read_text() == "Preserve existing data" and ini.read_bytes() == original
            print("Windows GUI installer: preconfigured instance -> Qt settings -> actual Prism splitter passed with spaces/apostrophe/#; existing instance untouched.")


if __name__ == "__main__":
    main()
