#!/usr/bin/env python3
"""Compare the resources a selection of resource packs gives the client with 26.3.

Builds test packs in a folder (a folder pack and a zip pack, with pack.mcmeta
overlays for this and an older format, a filter, block textures, language
files, sounds.json with added and replaced events, font providers and atlas
sources) and selects them over the prepared vanilla resources. The Java oracle
(VanillaPackResourcesOracle) opens them with vanilla's FolderRepositorySource
and MultiPackResourceManager and asks what each reload listener reads: which
pack a file comes from, translations (ClientLanguage), sound events
(SoundManager.prepare), font providers (FontManager) and atlas sources
(SpriteSourceList). examples/probe/pack_resources_fixture builds Eclipse's resource
root for the same selection (packs::build_overlay) and reads it with Eclipse's
loaders. Every answer must agree.
"""
import json
import shutil
import zipfile

try:
    from . import lab, oracle_harness as harness
except ImportError:
    import lab, oracle_harness as harness

FORMAT = {"min_format": [97, 0], "max_format": [97, 1]}


def write(root, files):
    for name, data in files.items():
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data if isinstance(data, bytes) else json.dumps(data).encode())


def packs(folder):
    shutil.rmtree(folder, ignore_errors=True)
    folder.mkdir(parents=True)
    write(folder / "Alpha", {
        "pack.mcmeta": {"pack": {"description": "alpha", **FORMAT}, "overlays": {"entries": [
            {"directory": "alpha_now", **FORMAT},
            {"directory": "alpha_old", "min_format": [80, 0], "max_format": [80, 0]}]}},
        "assets/minecraft/textures/block/stone.png": b"alpha stone",
        "assets/minecraft/textures/block/granite.png": b"alpha granite",
        "alpha_now/assets/minecraft/textures/block/dirt.png": b"alpha overlay dirt",
        "alpha_old/assets/minecraft/textures/block/sand.png": b"old overlay sand",
        "assets/minecraft/lang/en_us.json": {"block.minecraft.stone": "Alpha Stone", "eclipse.k1": "A1",
                                             "eclipse.k2": "A2"},
        "assets/minecraft/sounds.json": {
            "eclipse.test": {"subtitle": "subtitles.eclipse.a", "sounds": ["eclipse/a1"]},
            "block.stone.break": {"sounds": [{"name": "eclipse/a2", "weight": 2}]},
            "eclipse.ref": {"sounds": [{"name": "eclipse.test", "type": "event"}]}},
        "assets/minecraft/sounds/eclipse/a1.ogg": b"a1",
        "assets/minecraft/sounds/eclipse/a2.ogg": b"a2",
        "assets/minecraft/font/default.json": {"providers": [{"type": "space", "advances": {" ": 9}}]},
        "assets/minecraft/atlases/blocks.json": {"sources": [{"type": "single", "resource": "eclipse:block/a"}]},
        "assets/eclipse/lang/en_us.json": {"eclipse.ns": "Alpha namespace"},
    })
    beta = folder / "beta-src"
    write(beta, {
        "pack.mcmeta": {"pack": {"description": "beta", **FORMAT},
                        "filter": {"block": [{"namespace": "minecraft", "path": "textures/block/stone\\.png"}]}},
        "assets/minecraft/textures/block/granite.png": b"beta granite",
        "assets/minecraft/lang/en_us.json": {"eclipse.k1": "B1"},
        "assets/minecraft/sounds.json": {
            "eclipse.test": {"replace": True, "subtitle": "subtitles.eclipse.b",
                             "sounds": ["eclipse/b1", {"name": "eclipse/a1", "weight": 3}]},
            "block.stone.break": {"sounds": ["eclipse/missing"]}},
        "assets/minecraft/sounds/eclipse/b1.ogg": b"b1",
        "assets/minecraft/font/default.json": {"providers": [{"type": "space", "advances": {" ": 4}},
                                                             {"type": "space", "advances": {" ": 5}}]},
        "assets/minecraft/atlases/blocks.json": {"sources": [{"type": "single", "resource": "eclipse:block/b"}]},
    })
    # Gamma: a sounds.json Gson rejects (a bad entry) is skipped whole.
    write(folder / "Gamma", {
        "pack.mcmeta": {"pack": {"description": "gamma", **FORMAT}},
        "assets/minecraft/sounds.json": {"eclipse.test": {"sounds": ["eclipse/a1"]}, "bad": 3},
        "assets/minecraft/lang/en_us.json": {"eclipse.k2": "G2"},
    })
    with zipfile.ZipFile(folder / "Beta.zip", "w") as z:
        for path in sorted(beta.rglob("*")):
            if path.is_file():
                z.write(path, path.relative_to(beta).as_posix())
    shutil.rmtree(beta)


QUERIES = [
    "file minecraft:textures/block/stone.png",
    "file minecraft:textures/block/granite.png",
    "file minecraft:textures/block/dirt.png",
    "file minecraft:textures/block/sand.png",
    "file minecraft:textures/block/oak_planks.png",
    "lang block.minecraft.stone",
    "lang block.minecraft.dirt",
    "lang eclipse.k1",
    "lang eclipse.k2",
    "lang eclipse.ns",
    "lang menu.singleplayer",
    "sound eclipse.test",
    "sound eclipse.ref",
    "sound block.stone.break",
    "sound block.stone.place",
    "font minecraft:default",
    "font minecraft:uniform",
    "atlas minecraft:blocks",
    "atlas minecraft:gui",
]
SELECTIONS = [
    ["file/Alpha", "file/Beta.zip", "file/Gamma"],
    ["file/Beta.zip", "file/Alpha"],
    ["file/Alpha"],
    [],
]


def main():
    total = 0
    for index, selection in enumerate(SELECTIONS):
        # One run per selection: the input's first line names the packs, bottom first.
        def make(build, selection=selection):
            folder = build / "resourcepacks"
            packs(folder)
            vanilla = lab.RESOURCES
            path = build / "inputs.txt"
            path.write_text(f"root {vanilla} {folder} {' '.join(selection)}".rstrip() + "\n"
                            + "".join(q + "\n" for q in QUERIES))
            return path, len(QUERIES)
        harness.differential(f"resource-pack-parity-{index}", "VanillaPackResourcesOracle",
                                    "pack_resources_fixture", make,
                                    "Pack discovery, overlays, filters and the resource stacks vanilla merges.",
                                    sources=[lab.ROOT / "crates/eclipse-client-render/src/packs.rs",
                                             lab.ROOT / "crates/eclipse-client-render/src/sound_events.rs",
                                             lab.ROOT / "crates/eclipse-client-core/src/lang.rs"])
        total += len(QUERIES)
    print(f"{total} resource pack queries match the pinned client over {len(SELECTIONS)} selections")


if __name__ == "__main__":
    main()
