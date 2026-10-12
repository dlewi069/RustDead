#!/usr/bin/env python3
"""Assertions on a screenshot tour run (`SD_MENU_SHOTS=<dir> cargo run`, see scripts/check.sh).

usage: check_tour.py <run.log> <shots dir> <exit code>

Every step in src/menus/autoshot.rs's TOUR must have produced a non-blank PNG, the run must not
panic, and the behaviour each step exercises must show up (shots hit, lockpick opened, night is
darker than day...). Add a check: a function taking `ctx`, listed in CHECKS.
"""
import re
import sys
from pathlib import Path

from PIL import Image, ImageStat

ROOT = Path(__file__).resolve().parent.parent


class Ctx:
    def __init__(self, log, shots, code):
        self.log = Path(log).read_text(errors="replace")
        self.shots = Path(shots)
        self.code = int(code)
        self.errors, self.warnings = [], []
        src = (ROOT / "src/menus/autoshot.rs").read_text()
        tour = src[src.index("const TOUR"):src.index("];", src.index("const TOUR"))]
        self.steps = re.findall(r'MenuScreen::\w+, "([^"]+)"\)', tour)

    def err(self, msg):
        self.errors.append(msg)

    def warn(self, msg):
        self.warnings.append(msg)

    def brightness(self, step):
        im = Image.open(self.shots / f"{step}.png").convert("L")
        return ImageStat.Stat(im).mean[0]


def ran_cleanly(ctx):
    if ctx.code != 0:
        ctx.err(f"tour exited with {ctx.code}")
    for line in ctx.log.splitlines():
        if "panicked" in line or " ERROR " in line:
            ctx.err(f"log: {line.strip()[:200]}")
            break


def every_step_has_a_screenshot(ctx):
    if not ctx.steps:
        ctx.err("couldn't read TOUR from autoshot.rs")
    for step in ctx.steps:
        p = ctx.shots / f"{step}.png"
        if not p.exists():
            ctx.err(f"missing screenshot {step}.png")
        elif ctx.brightness(step) < 3.0:
            ctx.err(f"{step}.png is blank")


def shots_kill_the_zombie(ctx):
    hp = [int(m) for m in re.findall(r"autoshot shot \d+: target hp (-?\d+)/", ctx.log)]
    if not hp:
        ctx.err("08g_armed: no shots logged")
    elif min(hp) > 0:
        ctx.err(f"08g_armed: the zombie survived ({hp})")
    elif hp[0] >= int(re.search(r"target hp -?\d+/(\d+)", ctx.log).group(1)):
        ctx.err("08g_armed: the first shot did no damage")


def lockpick_opened(ctx):
    if "autoshot lockpick" not in ctx.log:
        ctx.err("08l_lockpick: no picklock container found")


def night_is_darker_than_day(ctx):
    if all((ctx.shots / f"{s}.png").exists() for s in ("08_in_game", "08k_night")):
        day, night = ctx.brightness("08_in_game"), ctx.brightness("08k_night")
        if not night < day * 0.8:
            ctx.err(f"08k_night ({night:.0f}) not darker than day ({day:.0f})")


def frame_rate(ctx):
    fps = [float(f) for f in re.findall(r"autoshot fps ([\d.]+)", ctx.log)]
    if fps and sum(fps) / len(fps) < 10:
        ctx.warn(f"average in-game fps {sum(fps) / len(fps):.1f}")


def hound_has_its_health(ctx):
    m = re.search(r"autoshot hound hp (\d+)/(\d+)", ctx.log)
    if not m:
        ctx.err("08m_hound: the hound never got Health")
    elif not 100 <= int(m.group(2)) <= 150:
        ctx.err(f"08m_hound: hound max hp {m.group(2)} outside BP_Zombie_Dog's 125 +- 25")


def corpse_opens_as_loot(ctx):
    m = re.search(r"autoshot corpse loot '([^']+)': (\d+) stacks \(key (\d+)\)", ctx.log)
    if not m:
        ctx.err("08n_corpse: the dead zombie didn't open as a loot container")
    elif m.group(1) != "Dead Zombie" or int(m.group(3)) < 2**32:
        ctx.err(f"08n_corpse: opened '{m.group(1)}' (key {m.group(3)}), not a corpse")


def medical_icons_show(ctx):
    m = re.search(r"autoshot medical icons: (\d+) shown", ctx.log)
    if not m or int(m.group(1)) != 4:
        ctx.err(f"08m_hound: expected 4 medical effect icons, got {m.group(1) if m else 'no log'}")


def weather_changes_the_scene(ctx):
    rain = re.search(r"autoshot weather Rain_Thunderstorm: rain ([\d.]+)", ctx.log)
    if not rain or float(rain.group(1)) < 9.9:
        ctx.err("08o_rain: the thunderstorm preset wasn't applied")
    dust = re.search(r"autoshot weather Sand_Dust_Storm: rain [\d.]+ dust ([\d.]+)", ctx.log)
    if not dust or float(dust.group(1)) < 8.9:
        ctx.err("08p_dust: the dust storm preset wasn't applied")
    # The storm's fog and dust close the view in: the dust shot is flatter than the clear one.
    if (ctx.shots / "08p_dust.png").exists() and (ctx.shots / "08_in_game.png").exists():
        def spread(step):
            im = Image.open(ctx.shots / f"{step}.png").convert("L")
            return ImageStat.Stat(im).stddev[0]
        if spread("08p_dust") >= spread("08_in_game"):
            ctx.err(f"08p_dust: no haze (contrast {spread('08p_dust'):.1f} vs clear {spread('08_in_game'):.1f})")


def level_spawners_wake(ctx):
    # The tour spends well over InitialSpawnDelay (9 s) beside spawners in the valley.
    woke = re.findall(r"zombie spawner (\d+) woke: group of (\d+)", ctx.log)
    if not woke:
        ctx.err("no BP_AISpawner_Zombies woke during the tour")
    elif not any(int(n) > 0 for _, n in woke):
        ctx.err("zombie spawners woke with empty groups (spawn multiplier/count?)")


CHECKS = [ran_cleanly, every_step_has_a_screenshot, shots_kill_the_zombie, lockpick_opened, night_is_darker_than_day, frame_rate, level_spawners_wake, hound_has_its_health, corpse_opens_as_loot, medical_icons_show, weather_changes_the_scene]


def main():
    ctx = Ctx(*sys.argv[1:4])
    for check in CHECKS:
        try:
            check(ctx)
        except Exception as e:
            ctx.err(f"{check.__name__} crashed: {e!r}")
    for w in ctx.warnings:
        print(f"  warn: {w}")
    for e in ctx.errors:
        print(f"  ERROR: {e}")
    print(f"tour checks: {len(ctx.steps)} steps, {len(CHECKS)} checks, {len(ctx.errors)} errors")
    sys.exit(1 if ctx.errors else 0)


if __name__ == "__main__":
    main()
