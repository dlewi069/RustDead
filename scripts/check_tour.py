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


def door_opens(ctx):
    m = re.search(r"autoshot door open ([\d.]+) tris (\d+)", ctx.log)
    if "autoshot door " not in ctx.log:
        ctx.err("08q_door: no free door found")
    elif not m:
        ctx.err("08q_door: door state not logged")
    else:
        if float(m.group(1)) < 0.5:
            ctx.err(f"08q_door: door only {m.group(1)} open after F")
        if int(m.group(2)) == 0:
            ctx.err("08q_door: the door has no collision")


def keypad_opens_the_door(ctx):
    if "autoshot keypad: no keypad door" in ctx.log:
        ctx.err("08s_keypad: the level has no keypad door")
        return
    code = re.search(r"autoshot keypad code (\d+)", ctx.log)
    m = re.search(r"autoshot keypad open ([\d.]+) locked (\w+)", ctx.log)
    if not code or not 1111 <= int(code.group(1)) <= 9999:
        ctx.err("08r_note: no keypad code rolled")
    if not m:
        ctx.err("08s_keypad: keypad door state not logged (pad never opened?)")
    else:
        if float(m.group(1)) < 0.5:
            ctx.err(f"08s_keypad: door only {m.group(1)} open after the code")
        if m.group(2) != "false":
            ctx.err("08s_keypad: door still locked after the right code")


def clothing_hides_the_body(ctx):
    # 08d_dressed wears the winter coat: BodyPartVisibility hides torso and arms under it.
    shown = re.findall(r"body parts shown: ([\w ]*)", ctx.log)
    if not shown:
        ctx.err("08d_dressed: body parts never updated for the clothing")
    elif any(p in shown[-1].split() for p in ("Torso", "Arms")):
        ctx.err(f"08d_dressed: skin parts still shown under the coat: {shown[-1]}")


def ladder_climbs(ctx):
    if "autoshot ladder: none" in ctx.log:
        ctx.err("08t_ladder: no drawn ladder in the level")
        return
    m = re.search(r"autoshot ladder climbing (\w+) rise ([-\d.]+) loco (\w+)", ctx.log)
    if not m:
        ctx.err("08t_ladder: climb not logged")
    else:
        if m.group(1) != "true" or m.group(3) != "Climb":
            ctx.err(f"08t_ladder: not climbing after W ({m.group(1)}, {m.group(3)})")
        # 1 m/s for ~2 s.
        if not 1.5 < float(m.group(2)) < 2.5:
            ctx.err(f"08t_ladder: rose {m.group(2)} m (MaxFlySpeed is 1 m/s)")
    top = re.search(r"autoshot ladder top climbing (\w+) above ([-\d.]+)", ctx.log)
    if not top:
        ctx.err("08u_ladder_top: top not logged")
    elif top.group(1) != "false" or float(top.group(2)) < -0.3:
        ctx.err(f"08u_ladder_top: didn't get off at the top (climbing {top.group(1)}, {top.group(2)} m from it)")


def lab_generator_lights_the_lab(ctx):
    m = re.search(r"autoshot power on (\w+) lit (\d+) lamps (\d+)", ctx.log)
    if "autoshot power: no generator" in ctx.log:
        ctx.err("08v_lab_power: no lab generator in the level")
    elif not m:
        ctx.err("08v_lab_power: generator state not logged")
    elif m.group(1) != "true" or int(m.group(2)) == 0:
        ctx.err(f"08v_lab_power: F didn't power the lab (on {m.group(1)}, {m.group(2)} lamps lit)")
    elif int(m.group(3)) == 0:
        ctx.err("08v_lab_power: lamps lit but no point light near the generator")
    # The switches' status lamps (OnRep_Warning? -> OnRep_On?): Warning before, On after.
    before = re.search(r"autoshot power status before (.*)", ctx.log)
    after = re.search(r"autoshot power status after (.*)", ctx.log)
    if not before or not after:
        ctx.err("08v_lab_power: status lamps not logged")
    elif "[Switch Some(Warning)]" not in before.group(1):
        ctx.err(f"08v_lab_power: no switch lamp showed Warning before power ({before.group(1)})")
    elif "[Switch Some(On)]" not in after.group(1):
        ctx.err(f"08v_lab_power: no switch lamp turned On with power ({after.group(1)})")


def shove_and_stomp(ctx):
    """08ib_shove: V shoves a roamer (it reels, standing still), looking down V stomps it (20-40)."""
    m = re.search(r"autoshot shove shoved (\w+) loco (\S+) dist ([\d.]+) hp (-?\d+)", ctx.log)
    if not m:
        ctx.err("08ib_shove: shove not logged")
        return
    if m.group(1) != "true" or "Idle" not in m.group(2):
        ctx.err(f"08ib_shove: zombie not shoved (shoved {m.group(1)}, {m.group(2)})")
    s = re.search(r"autoshot stomp hp (-?\d+)/(\d+)", ctx.log)
    if not s:
        ctx.err("08ib_shove: stomp not logged")
    elif not 20 <= int(m.group(4)) - int(s.group(1)) <= 40:
        ctx.err(f"08ib_shove: the stomp took {int(m.group(4)) - int(s.group(1))} hp (20-40 in the game)")


def frag_grenade_explodes(ctx):
    """08ic_throw: G throws one of two frags (85-115 within 10 m) at a roamer 8 m ahead."""
    left = re.search(r"autoshot throw grenades left (\d+)", ctx.log)
    if not left or left.group(1) != "1":
        ctx.err(f"08ic_throw: grenades left {left.group(1) if left else 'not logged'} (want 1)")
    if not re.search(r"DA_FragGrenade exploded at .*: [1-9]\d* zombies", ctx.log):
        ctx.err("08ic_throw: the frag didn't explode near the zombie")
    z = re.search(r"autoshot throw zombie hp (-?\d+) dead (\w+)", ctx.log)
    if not z:
        ctx.err("08ic_throw: zombie state not logged")
    elif z.group(2) != "true" and int(z.group(1)) > 30:
        ctx.err(f"08ic_throw: the zombie still has {z.group(1)} hp")
    # The physics body bounces, then lies still before its 3 s fuse.
    hit = ctx.log.find("DA_FragGrenade hit something")
    rest = re.search(r"DA_FragGrenade came to rest at .* after ([\d.]+) s", ctx.log)
    if hit < 0 or not rest or rest.start() < hit:
        ctx.err("08ic_throw: the frag didn't hit the ground then come to rest")
    elif float(rest.group(1)) >= 3.0:
        ctx.err(f"08ic_throw: the frag was still moving at the fuse ({rest.group(1)} s)")


def molotov_sets_the_zombie_burning(ctx):
    """08id_molotov: the fire lights 0.2 s after the smash; its tick hurts the roamer (10-25) and
    sets it burning (4 per 0.8 s once alight); the survivor catches fire when within its 5 m and
    then takes burn damage (3-5 per 0.8 s, no direct damage)."""
    if not re.search(r"DA_MolotovCocktail caught fire at", ctx.log):
        ctx.err("08id_molotov: the Molotov never caught fire")
    m = re.search(r"autoshot molotov zombie hp (-?\d+) dead (\w+) alight (\w+) survivor burning (\w+) fire dist (-?[\d.]+)", ctx.log)
    if not m:
        ctx.err("08id_molotov: zombie state not logged")
        return
    hp, dead, alight, me, dist = int(m.group(1)), m.group(2), m.group(3), m.group(4), float(m.group(5))
    if dead != "true" and (alight != "true" or hp >= 100):
        ctx.err(f"08id_molotov: the roamer isn't burning (hp {hp}, alight {alight})")
    burns = [float(x) for x in re.findall(r"burning: ([\d.]+) damage", ctx.log)]
    # Within 4.5 m the capsule is surely in the 5 m sphere; past 5.5 m surely not.
    if 0 <= dist < 4.5 and (me != "true" or not burns):
        ctx.err(f"08id_molotov: the survivor {dist} m from the fire didn't burn ({len(burns)} burn ticks)")
    if dist > 5.5 and me != "false":
        ctx.err(f"08id_molotov: the survivor caught fire {dist} m away")
    if burns and max(burns) > 5.0:
        ctx.err(f"08id_molotov: a burn tick did {max(burns)} damage (want 3-5 before armor)")


def damage_numbers_float(ctx):
    """FloatingDamageNumbersUI: shots, swings and blasts that hurt a zombie show their damage."""
    nums = re.findall(r"damage number (-?\d+)", ctx.log)
    if not nums:
        ctx.err("no floating damage numbers were shown")
    elif any(int(n) == 0 for n in nums):
        ctx.err("a damage number of 0 was shown (SpawnFloatingDamageText skips those)")


def recoil_kicks(ctx):
    """StartRecoil: every shot rolls a kick (guns kick up: negative pitch input), and the
    sideways part turns the camera (the tour pins the pitch, so only the yaw shows)."""
    kicks = [(float(p), float(y)) for p, y in re.findall(r"recoil kick (-?[\d.]+) (-?[\d.]+)", ctx.log)]
    shots = len(re.findall(r"autoshot shot \d+:", ctx.log))
    if len(kicks) < shots or not kicks:
        ctx.err(f"08g_armed: {len(kicks)} recoil kicks for {shots} shots")
        return
    if any(p > 0 for p, _ in kicks):
        ctx.err(f"08g_armed: a gun kicked down ({kicks})")
    yaws = [float(y) for y in re.findall(r"autoshot shot \d+:.*camera yaw (-?[\d.]+)", ctx.log)]
    if any(abs(y) > 0.01 for _, y in kicks) and len(set(yaws)) < 2:
        ctx.err(f"08g_armed: sideways recoil didn't turn the camera ({yaws})")


def passive_skills_gain_xp(ctx):
    """PassiveSkillsComponent XP from the tour's actions: searching a container (Scavenging 4-6,
    08c_loot), hitting the zombie with the rifle (Marksmanship 1-6 a hit, 08g_armed), crouched steps (Sneaking, 08w_crouch) and a hit the survivor
    lives through (Toughness 2-4, 08x_fall)."""
    xp = {}
    for step, rest in re.findall(r"autoshot passive (\S+) (.*)", ctx.log):
        it = rest.split()
        xp[step] = {it[k]: float(it[k + 1]) for k in range(0, len(it) - 1, 2)}
    steps = list(xp)
    if not xp:
        ctx.err("passive skill XP not logged")
        return
    def gained(step, skill, at_least):
        if step not in xp:
            ctx.err(f"{step}: passive XP not logged")
            return
        i = steps.index(step)
        before = xp[steps[i - 1]].get(skill, 0.0) if i > 0 else 0.0
        if xp[step].get(skill, 0.0) < before + at_least:
            ctx.err(f"{step}: no {skill} XP ({before} -> {xp[step].get(skill)})")
    gained("08c_loot", "Scavenging", 4.0)
    gained("08g_armed", "Marksmanship", 1.0)
    gained("08w_crouch", "Sneaking", 0.1)
    gained("08x_fall", "Toughness", 2.0)

def journal_unlocks_skills(ctx):
    """08za_journal: a level-up gives a skill point; the journal's tree spends it on Health Regen
    (cost 1) and refuses Lockpick+ (its parent, Distraction, is locked)."""
    lv = re.search(r"autoshot journal level (\d+) points (\d+)", ctx.log)
    btn = re.search(r"autoshot journal buttons (\d+)", ctx.log)
    end = re.search(r"autoshot journal points (\d+) unlocked (.*)", ctx.log)
    if not lv or not end:
        ctx.err("08za_journal: not logged")
        return
    if int(lv.group(2)) < 1:
        ctx.err(f"08za_journal: levelling up gave no skill point ({lv.group(0)})")
    if not btn or int(btn.group(1)) != 11:
        ctx.err(f"08za_journal: the tree shows {btn.group(1) if btn else 0} skills (want 11)")
    if "Skill_HealthRegen" not in end.group(2) or "Skill_Lockpick" in end.group(2):
        ctx.err(f"08za_journal: unlocked {end.group(2)} (want Health Regen only)")
    if int(end.group(1)) != int(lv.group(2)) - 1:
        ctx.err(f"08za_journal: {lv.group(2)} points -> {end.group(1)} (Health Regen costs 1)")
    if "Skill_Lockpick: Cannot unlock - Parent skill is locked!" not in ctx.log:
        ctx.err("08za_journal: Lockpick+ wasn't refused for its locked parent")


def body_parts_move_in_step(ctx):
    """Each survivor body part has its own AnimationPlayer; parts that join late (new clothes)
    copy the others' clip times, so the head and hands stay with the body."""
    spreads = [(step, float(x)) for step, x in re.findall(r"autoshot parts (\S+) spread ([\d.]+)", ctx.log)]
    if not spreads:
        ctx.err("body part phase not logged")
    bad = [f"{step} {x:.2f}s" for step, x in spreads if x > 0.05]
    if bad:
        ctx.err(f"body parts out of step: {', '.join(bad)}")


def police_garage_opens(ctx):
    """08va/08vb: the briefcase holds the key; gas + key start the generator; the lever opens the gate."""
    if "autoshot police: no" in ctx.log:
        ctx.err("08va/08vb: " + re.search(r"autoshot police: (no \w+)", ctx.log).group(1))
        return
    m = re.search(r'autoshot police briefcase "([^"]*)" holds .* key (\w+)', ctx.log)
    if not m:
        ctx.err("08va_briefcase: briefcase never opened")
    elif m.group(1) != "Police Briefcase" or m.group(2) != "true":
        ctx.err(f"08va_briefcase: {m.group(1)!r} opened, key inside: {m.group(2)}")
    g = re.search(r"autoshot police generator gas (\w+) on (\w+) keys left (\d+)", ctx.log)
    if not g:
        ctx.err("08vb_garage: generator state not logged")
    elif g.groups() != ("true", "true", "0"):
        ctx.err(f"08vb_garage: generator gas {g.group(1)} on {g.group(2)}, keys left {g.group(3)} (want fuelled, on, key used up)")
    if not re.search(r"autoshot police lever pulled gate moving true", ctx.log):
        ctx.err("08vb_garage: the lever didn't start the gate")
    # BP_Lever's SetPower_TurnedOn: its base shows green (and its red lamp goes grey).
    lamps = re.search(r"autoshot police lever lamps (.*)", ctx.log)
    if not lamps:
        ctx.err("08vb_garage: lever lamps not logged")
    elif "[LeverGreen Some(On)]" not in lamps.group(1) or "[LeverRed Some(On)]" not in lamps.group(1):
        ctx.err(f"08vb_garage: the powered lever's lamps aren't green ({lamps.group(1)})")
    o = re.search(r"autoshot police gate open (\w+) blocks (\w+) g1 moved ([\d.]+)", ctx.log)
    if not o:
        ctx.err("08vb_garage: gate state not logged")
    elif o.group(1) != "true" or o.group(2) != "false" or float(o.group(3)) < 4.0:
        ctx.err(f"08vb_garage: gate open {o.group(1)}, blocks {o.group(2)}, g1 moved {o.group(3)} m (want open, clear, ~4.7 m)")


def crouch_is_slow_and_quiet(ctx):
    m = re.search(r"autoshot crouch loco (\w+) height ([\d.]+) speed ([\d.]+) footsteps (\d+) range (\S+)", ctx.log)
    if not m:
        ctx.err("08w_crouch: crouch walk not logged")
        return
    loco, height, speed, steps, rng = m.groups()
    if loco != "CrouchWalk":
        ctx.err(f"08w_crouch: {loco} while holding Ctrl and W")
    # CrouchedHalfHeight 50 cm; MaxWalkSpeedCrouched 120 cm/s.
    if abs(float(height) - 1.0) > 0.01:
        ctx.err(f"08w_crouch: capsule {height} m tall crouched")
    if not 0.9 < float(speed) < 1.3:
        ctx.err(f"08w_crouch: moved {speed} m in a second (1.2 m/s crouched)")
    # A_Crouch_Walk at RateScale 0.8: two steps per 1.21 s.
    if int(steps) == 0 or rng != "Some(0.5)":
        ctx.err(f"08w_crouch: footsteps {steps}, last range {rng} (0.5 m crouched)")


def hard_landing_hurts(ctx):
    m = re.search(r"autoshot fall grounded (\w+) broken bone (\w+) stunned (\w+)", ctx.log)
    # The step's own landing: the last one before its log line (another step that puts the
    # survivor somewhere high, e.g. beside a container upstairs, can land hard too).
    before = ctx.log[: m.start()] if m else ctx.log
    lands = list(re.finditer(r"hard landing at ([\d.]+) m/s: ([\d.]+) damage \(([\d.]+) after armor\)", before))
    land = lands[-1] if lands else None
    if not land:
        ctx.err("08x_fall: no hard landing from 8 m")
    elif float(land.group(2)) != 30:
        # 8 m at 17.15 m/s^2: 16.6 m/s, the 15-18 m/s band.
        ctx.err(f"08x_fall: {land.group(2)} damage at {land.group(1)} m/s (30 expected)")
    elif abs(float(land.group(3)) - 30 / 1.3) > 0.1:
        # Heavy armor at 30 protection: CalculatePlayerDamage divides by 1.3.
        ctx.err(f"08x_fall: {land.group(3)} after armor (23.1 expected)")
    if not m:
        ctx.err("08x_fall: landing state not logged")
    elif m.groups() != ("true", "true", "true"):
        ctx.err(f"08x_fall: grounded/broken bone/stunned = {m.groups()}")


def flashlight_lights_up(ctx):
    m = re.search(r"autoshot flashlight on (\w+) beam (\S+) battery Some\(([\d.]+)\) hud (\w+)", ctx.log)
    if not m:
        ctx.err("08y_flashlight: flashlight not logged")
        return
    on, beam, charge = m.group(1), m.group(2), float(m.group(3))
    if m.group(4) != "true":
        ctx.err("08y_flashlight: the HUD battery box (FlashlightDur) isn't shown while on")
    if on != "true" or not beam.startswith("Some("):
        ctx.err(f"08y_flashlight: L didn't light the beam (on {on}, beam {beam})")
    # 0.0125 per 0.1 s for ~1.4 s: about 99.83 left.
    if not 99.7 < charge < 99.95:
        ctx.err(f"08y_flashlight: battery at {charge} after ~1.4 s on")


def sitting_holds_still(ctx):
    sit = re.search(r"autoshot sit loco (\w+) moved ([\d.]+)", ctx.log)
    stood = re.search(r"autoshot stood loco (\w+) grounded (\w+)", ctx.log)
    if not sit or not stood:
        ctx.err("08z_sit: sitting not logged")
        return
    # DisableMovement while seated; Jump only stands up.
    if sit.group(1) != "Sit" or float(sit.group(2)) > 0.05:
        ctx.err(f"08z_sit: {sit.group(1)}, moved {sit.group(2)} m with W while seated")
    if stood.group(1) != "Idle" or stood.group(2) != "true":
        ctx.err(f"08z_sit: after Space {stood.group(1)}, grounded {stood.group(2)} (stand up, no jump)")


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
        why = re.search(r"autoshot corpse not opened: (.*)", ctx.log)
        ctx.err(f"08n_corpse: the dead zombie didn't open as a loot container ({why.group(1) if why else 'no corpse?'})")
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


CHECKS = [ran_cleanly, every_step_has_a_screenshot, shots_kill_the_zombie, lockpick_opened, door_opens, keypad_opens_the_door, clothing_hides_the_body, ladder_climbs, lab_generator_lights_the_lab, shove_and_stomp, frag_grenade_explodes, molotov_sets_the_zombie_burning, damage_numbers_float, recoil_kicks, passive_skills_gain_xp, journal_unlocks_skills, body_parts_move_in_step, police_garage_opens, crouch_is_slow_and_quiet, hard_landing_hurts, flashlight_lights_up, sitting_holds_still, night_is_darker_than_day, frame_rate, level_spawners_wake, hound_has_its_health, corpse_opens_as_loot, medical_icons_show, weather_changes_the_scene]


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
