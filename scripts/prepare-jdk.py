#!/usr/bin/env python3
"""Prepare javac locally on this Debian/JRE cloud image; no system files modified."""
import os
from pathlib import Path
import shutil
import subprocess

ROOT = Path("/workspace/.rusted-tools")

def main():
    if shutil.which("javac"):
        help_text = subprocess.check_output(["javac", "--help"], text=True)
        if "17" in help_text:
            print("javac supports --release 17")
            return
    apt = ROOT / "apt"
    for name in ("lists/partial", "archives/partial", "etc/parts", "etc/sources.list.d"):
        (apt / name).mkdir(parents=True, exist_ok=True)
    (apt / "etc/sources.list").write_text(
        "deb [signed-by=/usr/share/keyrings/debian-archive-keyring.gpg] https://deb.debian.org/debian trixie main\n")
    config = apt / "config"
    config.write_text(f'''Dir::Etc::main "-";
Dir::Etc::parts "{apt}/etc/parts";
Dir::Etc::sourcelist "{apt}/etc/sources.list";
Dir::Etc::sourceparts "{apt}/etc/sources.list.d";
Dir::State::lists "{apt}/lists";
Dir::State::status "/var/lib/dpkg/status";
Dir::Cache::archives "{apt}/archives";
APT::Sandbox::User "agent";
''')
    environment = {**os.environ, "APT_CONFIG": str(config)}
    subprocess.run(["apt-get", "update"], env=environment, check=True)
    # Disable only dpkg locking for a download-only operation in our private cache.
    # APT's Release signatures and package checksums remain mandatory.
    subprocess.run(["apt-get", "-o", "Debug::NoLocking=1", "--download-only", "--no-install-recommends",
                    "-y", "install", "openjdk-21-jdk-headless"], env=environment, check=True)
    packages = sorted((apt / "archives").glob("openjdk-21-jdk-headless_*.deb"))
    if not packages:
        raise RuntimeError("No verified JDK package downloaded")
    local = ROOT / "jdk"
    subprocess.run(["dpkg-deb", "-x", str(packages[-1]), str(local)], check=True)
    base = Path("/usr/lib/jvm/java-21-openjdk-amd64")
    target = local / "usr/lib/jvm/java-21-openjdk-amd64"
    if not (base / "lib/modules").is_file():
        raise RuntimeError("This setup expects the image's OpenJDK 21 runtime")
    for item in base.rglob("*"):
        destination = target / item.relative_to(base)
        if not destination.exists() and not destination.is_symlink():
            if item.is_dir():
                destination.mkdir(parents=True, exist_ok=True)
            else:
                destination.symlink_to(item)
    binary = ROOT / "bin"
    binary.mkdir(exist_ok=True)
    wrapper = binary / "javac"
    wrapper.write_text(f'#!/bin/sh\nexec /usr/bin/java -Djava.home={target} com.sun.tools.javac.Main "$@"\n')
    wrapper.chmod(0o755)
    subprocess.run([str(wrapper), "-version"], check=True)

if __name__ == "__main__":
    main()
