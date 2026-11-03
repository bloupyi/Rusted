#!/usr/bin/env python3
"""Render original custom heads and animated item textures before/after pack reload.

Uses disposable worlds in both native execution paths. No account, remote skin
service, server pack, or third-party artwork is needed. Requires Pillow and a
display (Xvfb works).
"""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import tempfile
import time

from PIL import Image
from test_horizon_native import NativeGame, ROOT


def fixture(home, resources):
    texture = "rusted_fixture_cyan"
    digest = hashlib.sha1(texture.encode("utf-16-le")).hexdigest()
    cache = home / "assets/skins" / digest[:2] / digest
    cache.parent.mkdir(parents=True)
    Image.new("RGBA", (64, 64), (0, 255, 180, 255)).save(cache, format="PNG")
    profile = base64.b64encode(json.dumps({"textures": {"SKIN": {
        "url": "https://textures.minecraft.net/texture/" + texture}}}).encode()).decode()
    pack = home / "resourcepacks/visual-fixture"
    skin = pack / "assets/fixture/textures/entity/head.png"
    skin.parent.mkdir(parents=True)
    Image.new("RGBA", (64, 64), (255, 0, 170, 255)).save(skin)
    gem = pack / "assets/fixture/textures/custom/gem.png"
    gem.parent.mkdir(parents=True)
    strip = Image.new("RGBA", (16, 32), (255, 0, 0, 255))
    strip.paste((0, 0, 255, 255), (0, 16, 16, 32))
    strip.save(gem)
    gem.with_suffix(".png.mcmeta").write_text(json.dumps({"animation": {"frametime": 4}}))
    items = pack / "assets/fixture/items"
    items.mkdir(parents=True)
    (items / "gem.json").write_text(json.dumps({"model": {"type": "minecraft:model", "model": "fixture:item/gem"}}))
    models = pack / "assets/fixture/models/item"
    models.mkdir(parents=True)
    (models / "gem.json").write_text(json.dumps({"parent": "minecraft:item/generated", "textures": {"layer0": "fixture:custom/gem"}}))
    atlases = pack / "assets/minecraft/atlases"
    atlases.mkdir(parents=True)
    original = resources / "assets/minecraft/atlases/items.json"
    atlas = json.loads(original.read_text()) if original.is_file() else {"sources": []}
    atlas["sources"].append({"type": "minecraft:single", "resource": "fixture:custom/gem"})
    (atlases / "items.json").write_text(json.dumps(atlas))
    (pack / "pack.mcmeta").write_text(json.dumps({"pack": {"pack_format": 97, "description": "Original visual regression fixture"}}))
    return profile, skin


def colors(path):
    image = Image.open(path).convert("RGB")
    # Restrict assertions to the hotbar. World terrain and the player's hand
    # must not make a missing inventory icon look like a successful render.
    width, height = image.size
    pixels = list(image.crop((width // 2 - 185, height - 44, width // 2 + 185, height - 2)).getdata())
    return {
        "cyan": sum(r < 10 and g > 60 and abs(b / g - 180 / 255) < .05 for r, g, b in pixels),
        "magenta": sum(g < 10 and r > 60 and abs(b / r - 170 / 255) < .05 for r, g, b in pixels),
        "yellow": sum(b < 10 and r > 60 and abs(g / r - 1) < .05 for r, g, b in pixels),
        "red": sum(r > 60 and g < 10 and b < 10 for r, g, b in pixels),
        "blue": sum(b > 60 and r < 10 and g < 10 for r, g, b in pixels),
    }


def observe(game, name, required, absent=()):
    deadline = time.monotonic() + 30
    seen = set()
    while time.monotonic() < deadline:
        result = colors(game.screenshot(name)["path"])
        seen.update(color for color in required if result[color] > 15)
        if seen == set(required) and all(result[color] == 0 for color in absent):
            return {"seen": sorted(seen), "last_frame_pixels": result}
        time.sleep(.12)
    raise AssertionError(f"Missing rendered colors in {name}: {required}; observed={seen}, last={result}")


def reload(game):
    deadline = time.monotonic() + 180
    while True:
        try:
            return game.command("resource-pack file/visual-fixture")
        except RuntimeError as error:
            if "Initial resources are still loading" not in str(error) or time.monotonic() > deadline:
                raise
            time.sleep(.2)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--resources", required=True, type=Path)
    args = parser.parse_args()
    args.resources = args.resources.resolve()
    root = Path(tempfile.mkdtemp(prefix="native-resource-visuals-", dir=ROOT / "build"))
    keys = ["HORIZON_SERVER_THREAD", "HORIZON_SESSION_FILE", "RUSTED_HOME", "RUSTED_INSTANCE_DIR"]
    original = {key: os.environ.get(key) for key in keys}
    reports = []
    try:
        for threaded in [True, False]:
            scratch = root / ("server-thread" if threaded else "in-frame")
            home = scratch / "game"
            profile, skin = fixture(home, args.resources)
            os.environ.update(HORIZON_SERVER_THREAD="1" if threaded else "0", RUSTED_HOME=str(home), RUSTED_INSTANCE_DIR=str(home))
            os.environ.pop("HORIZON_SESSION_FILE", None)
            game = NativeGame(args.binary.resolve(), args.resources, scratch, home, True)
            result = {"server_thread": threaded, "account_used": False, "passed": False}
            try:
                game.wait(lambda state: state.get("menu") is not None)
                result["first_reload"] = reload(game)
                game.command("open ResourceVisuals 123 creative")
                game.wait(lambda state: state.get("world") is not None and state.get("player") is not None)
                game.command('command give @s minecraft:player_head[minecraft:profile={properties:[{name:"textures",value:"' + profile + '"}]}]')
                game.command('command give @s minecraft:player_head[minecraft:profile={texture:"fixture:entity/head"}]')
                game.command('command give @s minecraft:diamond[minecraft:item_model="fixture:gem"]')
                result["before"] = observe(game, "before", ["cyan", "magenta", "red", "blue"])
                result["second_reload"] = reload(game)
                result["after"] = observe(game, "after", ["cyan", "magenta", "red", "blue"])
                Image.new("RGBA", (64, 64), (255, 255, 0, 255)).save(skin)
                result["changed_reload"] = reload(game)
                result["changed"] = observe(game, "changed", ["cyan", "yellow", "red", "blue"], ["magenta"])
                result["passed"] = True
            finally:
                game.close()
                reports.append(result)
                (root / "result.json").write_text(json.dumps(reports, indent=2) + "\n")
            print(json.dumps(result), flush=True)
    finally:
        for key, value in original.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value
    print(root / "result.json", flush=True)


if __name__ == "__main__":
    main()
