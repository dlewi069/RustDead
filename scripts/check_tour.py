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
    # The state when its screenshot was taken (a death later in the tour drops the clothes).
    cut = ctx.log.find("08d_dressed.png")
    shown = re.findall(r"body parts shown: ([\w ]*)", ctx.log[: cut if cut >= 0 else len(ctx.log)])
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


def journal_info_shows_the_stats(ctx):
    """08za_journal_info: J opens the journal on its INFO page (JournalUI's OnInitialized) with
    "SKULL TRACKER", the survivor's license and "LVL: N"; the earlier steps' zombie kills are
    counted, days survived starts at 1 and the distance travelled has grown (CheckDistance)."""
    m = re.search(r"autoshot journal info page (\w+) title (\w+) license (\w+) level_text (\w+) zombies (\d+) animals (\d+) days (\d+) distance ([\d.]+) distance_text (\w+)", ctx.log)
    if not m:
        ctx.err("08za_journal_info: not logged")
        return
    if m[1] != "Info" or "false" in (m[2], m[3], m[4], m[9]):
        ctx.err(f"08za_journal_info: page {m[1]}, title/license/level/distance texts {m[2]}/{m[3]}/{m[4]}/{m[9]}")
    if int(m[5]) < 1:
        ctx.err("08za_journal_info: no zombie kills counted (the 08g steps kill one)")
    if int(m[7]) < 1 or float(m[8]) <= 0:
        ctx.err(f"08za_journal_info: days survived {m[7]}, distance {m[8]} cm")
    # stats.rs: DaysSurvivedWidget types out its line 4 s into the game (Client_AddUI).
    if "days survived popup: Days Survived: " not in ctx.log:
        ctx.err("the Days Survived popup never finished typing")


def map_shows_discovered_poi(ctx):
    """08zb_map: standing in the nearest POI's radius discovers it within its 1-4 s DistanceCheck
    (XP = level x 50 or 250 x 1.05, the POI DISCOVERED banner); M opens the map, the wheel zooms
    to step -1, and the GPS arrow and the explored POIs' markers show. A double click places a
    map marker with its compass icon."""
    poi = re.search(r"autoshot map poi (.+) at", ctx.log)
    disc = re.search(r"autoshot map discovered (\w+) xp (\d+) banner (\d+) level (\d+)", ctx.log)
    view = re.search(r"autoshot map step (-?\d+) scale ([\d.]+) arrow (\w+) markers (\d+)", ctx.log)
    if not poi or not disc or not view:
        ctx.err("08zb_map: not logged")
        return
    if disc.group(1) != "true" or int(disc.group(3)) < 1:
        ctx.err(f"08zb_map: {poi.group(1)} wasn't discovered with its banner ({disc.group(0)})")
    level, xp = int(disc.group(4)), int(disc.group(2))
    if xp not in (round(level * 50 * 1.05), round(level * 250 * 1.05)) and xp not in (round((level - 1) * 50 * 1.05), round((level - 1) * 250 * 1.05)):
        ctx.err(f"08zb_map: discovery gave {xp} XP at level {level} (want level x 50/250 x 1.05)")
    if int(view.group(1)) != -1 or abs(float(view.group(2)) - 0.7975) > 1e-3:
        ctx.err(f"08zb_map: zoom step {view.group(1)} scale {view.group(2)} (want -1 at 0.7975)")
    if view.group(3) != "true":
        ctx.err("08zb_map: the GPS arrow isn't shown")
    if int(view.group(4)) < 1:
        ctx.err("08zb_map: no POI markers on the map")
    # markers.rs: the double click 150 m ahead places a marker (Compass Marker? is on by default),
    # drawn on the map beside the GPS picker, and its icon is near the compass's middle (350 + 700 tan).
    placed = re.search(r"autoshot map markers placed (\d+) widgets (\d+) picker (\d+) compass (Some\(\((\w+), ([-\d.]+|NaN)\)\)|None)", ctx.log)
    if not placed:
        ctx.err("08zb_map: the map marker wasn't logged")
    elif placed.group(1) != "1" or placed.group(2) != "1" or placed.group(3) != "1":
        ctx.err(f"08zb_map: double click placed {placed.group(1)} markers, {placed.group(2)} widgets, {placed.group(3)} pickers (want 1 each)")
    elif placed.group(5) != "true" or abs(float(placed.group(6)) - 350.0) > 120.0:
        ctx.err(f"08zb_map: the marker's compass icon {placed.group(4)} (want shown near x 350)")


def music_plays(ctx):
    """audio/music.rs: MainMenuMusic under the menus; in game the spawn is in the Main audio area,
    whose first track waits 40-120 s (skipped by 08zc_music), then Main_ExplorationMusic plays (or
    CombatMusic_Cue while zombies chase) and the menu music is gone."""
    menu = re.search(r"autoshot music menu \[(.*)\]", ctx.log)
    area = re.search(r"autoshot music area (\S+) waiting (\S+) playing (.*)", ctx.log)
    voices = re.search(r"autoshot music voices \[(.*)\] playing (.*) phase (\w+) combat (\w+) menu \[(.*)\]", ctx.log)
    if not menu or not area or not voices:
        ctx.err("music: not logged (01_main / 08zc_music)")
        return
    if "MainMenuMusic" not in menu.group(1):
        ctx.err(f"music: the main menu has no MainMenuMusic ({menu.group(1)!r})")
    if area.group(1) != "Main":
        ctx.err(f"music: the spawn is in audio area {area.group(1)} (want Main)")
    wait = re.match(r"Some\(([\d.]+)\)", area.group(2))
    if wait and not 0.0 <= float(wait.group(1)) <= 120.0:
        ctx.err(f"music: Main's wait is {wait.group(1)} s (DelayBetweenMusic 40-120)")
    # Roamers chasing the survivor at the spawn turn it into the combat track instead, and
    # between tracks the director fades one out (switching) or, its fight track over, sits idle.
    phase, combat = voices.group(3), voices.group(4) == "true"
    if phase == "playing":
        want = "CombatMusic_Cue" if '", true)' in voices.group(2) else "Main_ExplorationMusic"
        if want not in voices.group(1):
            ctx.err(f"music: {want} isn't playing ({voices.group(0)})")
    elif phase == "waiting" or (phase == "idle" and not combat):
        ctx.err(f"music: the director is {phase} after the wait was skipped ({voices.group(0)})")
    if voices.group(5).strip():
        ctx.err(f"music: the menu music still plays in game ({voices.group(5)})")


def combat_roll_rolls(ctx):
    """08zd_roll: with Combat Roll unlocked, D + C turns the survivor right and the Roll's root
    motion carries them ~5 m (walls may cut it short), for 8 stamina, putting their fire out."""
    m = re.search(r"autoshot roll moved ([\d.]+) along_right (-?[\d.]+) facing_right (-?[\d.]+) stamina_spent (-?[\d.]+) burning (\w+)", ctx.log)
    if not m:
        ctx.err("08zd_roll: not logged")
        return
    moved, along, facing, spent, burning = float(m[1]), float(m[2]), float(m[3]), float(m[4]), m[5]
    # D walks ~1.2 m outside the roll (0.1 s before, 0.2 s after); the roll adds its 5.4 m.
    if moved < 4.5:
        ctx.err(f"08zd_roll: moved {moved} m (want the roll's ~5.4 m plus a little walking)")
    if along < 0.8 or facing < 0.8:
        ctx.err(f"08zd_roll: went {along:.2} / faced {facing:.2} along the camera's right (want ~1)")
    if not 7.5 <= spent <= 8.5:
        ctx.err(f"08zd_roll: the roll took {spent} stamina (it costs 8)")
    if burning != "false":
        ctx.err("08zd_roll: the roll didn't put the fire out")


def fence_is_climbed_over(ctx):
    """08zf_vault: Space facing the 2.1 m board fence climbs it (MQ_Climb_RM1, 1 s) and jumps down
    the far side (MQ_JumpingDownFromWall_RM1, 1.2 s, then the clip's end), ending ~1.5 m on at the
    same ground height, for no stamina and with no jump (the vault's Flying mode blocks it)."""
    if "autoshot vault no fence found" in ctx.log:
        ctx.err("08zf_vault: no spot by the fence plans a climb")
        return
    m = re.search(r"autoshot vault kind (.+?) over (-?[\d.]+) rise (-?[\d.]+) stamina_spent (-?[\d.]+) up_speed (-?[\d.]+) after ([\d.]+)", ctx.log)
    if not m:
        ctx.err("08zf_vault: not logged")
        return
    kind, over, rise, spent, up, after = m[1], float(m[2]), float(m[3]), float(m[4]), float(m[5]), float(m[6])
    if kind != "Some(Climb { thick: false })":
        ctx.err(f"08zf_vault: Space at the fence did {kind} (want a climb over it)")
        return
    if not 1.0 <= over <= 2.5 or abs(rise) > 0.3:
        ctx.err(f"08zf_vault: ended {over:.2} m on and {rise:.2} m up (want the far side, ~1.5 m on, at the same height)")
    if spent > 0.5:
        ctx.err(f"08zf_vault: the climb took {spent} stamina (it only checks for 5)")
    if up > 1.0:
        ctx.err(f"08zf_vault: a jump came of the press too (up at {up} m/s)")
    if not 1.8 <= after <= 3.2:
        ctx.err(f"08zf_vault: took {after} s (want 1 s up, 1.2 s down and the clip's end)")


def animals_flee_bite_and_die(ctx):
    """08zg_deer / 08zh_wolf: a deer that sees the survivor runs (Run Away at 8 m/s); killed, it
    lies on its side as a "Dead Deer" container (loot table Deer), toasts "Deer Killed, XP Given"
    and, with Huntsman, leaves the Animal Meat bag. A wolf that sees them snarls, runs in and bites
    (20 per AttackMontage)."""
    # It must have fled at some point and got away; by 1.5 s it may be roaming again (the game's
    # Can See Player decorator aborts the flight once the survivor is behind it).
    m = re.search(r"autoshot deer brain (\w+) loco (\w+) moved ([\d.]+) from_player [\d.]+ seen \w+ fled (\w+)", ctx.log)
    if not m:
        ctx.err("08zg_deer: not logged")
    elif m[4] != "true" or float(m[3]) < 3.0:
        ctx.err(f"08zg_deer: the deer is {m[1]} / {m[2]}, fled {m[4]}, moved {m[3]} m (want it to have run away)")
    m = re.search(r"autoshot deer dead (\w+) loco (\w+) up_dot (-?[\d.]+) corpse (\w+) container (.+?) meat (\w+) toast \"(.*?)\"", ctx.log)
    if not m:
        ctx.err("08zg_deer: the kill wasn't logged")
    else:
        if m[1] != "true" or abs(float(m[3])) > 0.2:
            ctx.err(f"08zg_deer: dead {m[1]}, up·Y {m[3]} (want it lying on its side)")
        if m[4] != "true" or m[5] != 'Some(("Dead Deer", Some("Deer")))':
            ctx.err(f"08zg_deer: corpse container {m[4]} {m[5]}")
        if m[6] != "true":
            ctx.err("08zg_deer: no Animal Meat bag with Huntsman")
        if not m[7].startswith("Deer Killed, XP Given: "):
            ctx.err(f"08zg_deer: toast {m[7]!r}")
    m = re.search(r"autoshot wolf brain (\w+) loco (\w+) moved ([\d.]+) from_player ([\d.]+)", ctx.log)
    if not m or m[1] != "Chase" or m[2] != "Attack" or float(m[4]) > 3.0:
        ctx.err(f"08zh_wolf: wolf {m.groups() if m else 'not logged'} (want it at the survivor, attacking)")
    # DamageToDo 20, less whatever armour the survivor wears.
    bite = re.search(r"autoshot wolf bit for (\d+) at", ctx.log)
    if not bite or not 5 <= int(bite[1]) <= 20:
        ctx.err(f"08zh_wolf: bite {bite[1] if bite else 'none'} (want DamageToDo 20 less armour)")
    sounds = re.search(r"autoshot sounds 08zh_wolf (.*)", ctx.log)
    if not sounds or "WolfSnarl_Cue=" not in sounds[1]:
        ctx.err("08zh_wolf: no snarl (WolfSnarl_Cue) on seeing the survivor")


def swimming_floats_dives_and_surfaces(ctx):
    """08ze_swim: dropped into a 2.5 m lake the survivor swims afloat (feet ~1.12 m under the
    surface, the swim clips); W looking down takes them under (oxygen draining, the underwater
    clip); Space brings them back to the float."""
    rows = {m[0]: m for m in re.findall(r"autoshot swim (\w+) swimming (\w+) under (\w+) deepness ([\d.]+) feet (-?[\d.]+) surface (\S+) oxygen ([\d.]+) loco (\w+)", ctx.log)}
    if set(rows) != {"afloat", "dived", "surfaced"}:
        ctx.err(f"08ze_swim: not logged ({sorted(rows)})")
        return
    a, d, s = rows["afloat"], rows["dived"], rows["surfaced"]
    surface = re.match(r"Some\(([\d.]+)\)", a[5])
    if a[1] != "true" or a[2] != "false" or not surface or not a[7].startswith("Swim"):
        ctx.err(f"08ze_swim: not afloat in the lake ({a})")
    elif abs(float(surface[1]) - 1.12 - float(a[4])) > 0.15:
        ctx.err(f"08ze_swim: feet at {a[4]} under a surface at {surface[1]} (want 1.12 m under)")
    if d[2] != "true" or d[7] != "SwimUnder" or float(d[6]) >= 100.0:
        ctx.err(f"08ze_swim: the dive didn't go under with the air running down ({d})")
    if s[1] != "true" or s[2] != "false" or float(s[4]) <= float(d[4]):
        ctx.err(f"08ze_swim: Space didn't bring them back up ({s})")


def sounds_play(ctx):
    """The game's SoundCues play: the rifle's shot and a hit (08g_armed), footsteps (08b_walked),
    the melee whoosh (08i_melee), the frag's explosion (08ic_throw), zombies breathing, and the
    thunderstorm's weather sounds (08o_rain)."""
    played = {}
    for step, rest in re.findall(r"autoshot sounds (\S+) ?(.*)", ctx.log):
        played[step] = {k: int(n) for k, n in (x.split("=") for x in rest.split() if "=" in x)}
    if not played:
        ctx.err("sounds not logged")
        return
    def has(step, pred, what):
        if step not in played:
            ctx.err(f"{step}: sounds not logged")
        elif not any(pred(k) for k in played[step]):
            ctx.err(f"{step}: no {what} played ({sorted(played[step])[:12]})")
    has("08b_walked", lambda k: k.removesuffix("_Cue").removesuffix("Sprint") in ("Concrete", "Dirt", "Grass", "Rock", "Sand", "Metal", "Wood", "Water", "Snow"), "footstep")
    has("08g_armed", lambda k: k.startswith(("bullet_impact_body_thump", "Head_shot")), "bullet hit")
    has("08g_armed", lambda k: ("shot" in k.lower() or "fire" in k.lower()) and not k.startswith(("Head_shot", "S_EmptyFire", "bullet")), "gunshot")
    has("08i_melee", lambda k: k.startswith("whoosh"), "melee swing")
    has("08ic_throw", lambda k: k.startswith("explosion"), "explosion")
    has("08m_hound", lambda k: k in ("ZombieBreathLoopCue", "HoundCue"), "zombie voice")
    has("08s_keypad", lambda k: k == "door_lock_open_02_Cue", "keypad door lock")
    has("08_in_game", lambda k: k == "birds__1__Cue", "daytime birds")
    has("08k_night", lambda k: k == "night_and_crickets_ambient_loop_Cue", "night crickets")
    has("08vb_garage", lambda k: k == "engine_generator_loop_01_Cue", "generator hum")
    # The thunderstorm (rain, wind and thunder all 10): UDS's heavy rain, wind, whistling and
    # distant thunder (weather_sounds.rs).
    for loop in ("UDS_Rain_Loop.Heavy", "UDS_Wind_Loop", "Wind_Whistling", "UDS_Distant_Thunder_Loop"):
        has("08o_rain", lambda k, loop=loop: k == loop, loop)


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


def traders_trade(ctx):
    """08zi_trader / 08zj_trade: F on the Supplies Trader begins its dialogue (two replies);
    "What are you selling?" opens its stock as the loot screen; buying takes the price in Money
    and selling pays it (GetAllItemsSellPrice), with no refusal."""
    m = re.search(r"autoshot trader overlay (\w+)\((\d+)\) replies (\d+)", ctx.log)
    if not m or m[1] != "Dialogue" or m[3] != "2":
        ctx.err(f"08zi_trader: F opened {m.groups() if m else 'nothing logged'} (want the dialogue with 2 replies)")
    m = re.search(r"autoshot trade screen (\w+)", ctx.log)
    if not m or m[1] != "Loot":
        ctx.err(f"08zj_trade: the trade reply opened {m[1] if m else 'nothing'} (want the loot screen)")
    for kind, sign in (("buy", -1), ("sell", 1)):
        m = re.search(rf"autoshot trade {kind} (\S+) x(\d+) price ([\d.]+) money (\d+) -> (\d+) trader \S+ -> \S+ notice (.+)", ctx.log)
        if not m:
            ctx.err(f"08zj_trade: no {kind} logged")
            continue
        price, before, after = int(float(m[3])), int(m[4]), int(m[5])
        if m[6].strip() != "None" or after - before != sign * price:
            ctx.err(f"08zj_trade: {kind} {m[1]} x{m[2]} for {price}: money {before} -> {after}, notice {m[6].strip()}")


def airdrops_fall_and_open(ctx):
    """08zk_airdrop / 08zl_airdrop_loot: the crate sinks under its parachute at about 9.8 m/s
    (gravity over LinearDamping 1), the aircraft holds 75 m/s at its height, the engine sound
    plays, "Airdrop incoming..." shows and the map gets the "Medical Airdrop" marker; landed,
    holding F opens it as "Airdrop" with loot in it."""
    m = re.search(r"autoshot airdrop falling height (-?[\d.]+) speed ([\d.]+) plane_speed ([\d.]+) plane_level (\w+) toasts (\[.*?\]) marker \"(.*?)\"", ctx.log)
    if not m:
        ctx.err("08zk_airdrop: not logged")
    else:
        height, speed, plane = float(m[1]), float(m[2]), float(m[3])
        if not (2.0 < height < 38.0) or not (8.5 < speed < 10.5):
            ctx.err(f"08zk_airdrop: crate {height} m up falling {speed} m/s (want it sinking at ~9.8 m/s)")
        if not (73.0 < plane < 77.0) or m[4] != "true":
            ctx.err(f"08zk_airdrop: aircraft {plane} m/s, level {m[4]} (want 75 m/s at its height)")
        if "Airdrop incoming..." not in m[5]:
            ctx.err(f"08zk_airdrop: toasts {m[5]}")
        if m[6] != "Medical Airdrop":
            ctx.err(f"08zk_airdrop: map marker {m[6]!r}")
    sounds = re.search(r"autoshot sounds 08zk_airdrop (.*)", ctx.log)
    if not sounds or "AirCraft_Cue=" not in sounds[1]:
        ctx.err("08zk_airdrop: no AirCraft_Cue")
    m = re.search(r"autoshot airdrop loot opened Some\(true\) name Some\(\"Airdrop\"\) items (\d+)", ctx.log)
    if not m or int(m[1]) == 0:
        ctx.err("08zl_airdrop_loot: holding F by the landed crate didn't open its loot")


def trees_fall_and_wrecks_salvage(ctx):
    """08zm_chop / 08zn_salvage: the Fireman's Axe fells a tree in 6-8 hits for 8-12 Wooden Logs
    (no Harvest Master on the tour), with the chop and fall sounds, the tree leaves the props and
    tips over; the burnt-out vehicle salvages for 12-20 Scrap Metal, the timer running midway."""
    m = re.search(r"autoshot chop felled after (\d+) clicks: hits_left Some\((-?\d+)\) logs (\d+) removed (\w+) falling (\d+)", ctx.log)
    if not m:
        ctx.err("08zm_chop: the tree wasn't felled (no reachable tree, or the swings didn't hit)")
    else:
        if not (8 <= int(m[3]) <= 12) or m[4] != "true" or int(m[5]) < 1:
            ctx.err(f"08zm_chop: logs {m[3]}, removed {m[4]}, falling {m[5]}")
        hits = len(re.findall(r"harvest: tree .*? hit, ", ctx.log))
        if not (6 <= hits <= 8):
            ctx.err(f"08zm_chop: {hits} hits to fell it (want 6-8)")
    if "No damage to tree dealt." in ctx.log:
        ctx.err("08zm_chop: the axe was refused")
    sounds = re.search(r"autoshot sounds 08zm_chop (.*)", ctx.log)
    if not sounds or "ChopWoodRandom_Cue=" not in sounds[1] or "TreeFallingRandomCue=" not in sounds[1]:
        ctx.err("08zm_chop: no ChopWoodRandom_Cue / TreeFallingRandomCue")
    if not re.search(r"autoshot salvage midway Some\([1-9]\)", ctx.log):
        ctx.err("08zn_salvage: F didn't start the salvage")
    m = re.search(r"autoshot salvage done: scrap (\d+) salvaged (\d+)", ctx.log)
    if not m or not (12 <= int(m[1]) <= 20) or m[2] != "1":
        ctx.err(f"08zn_salvage: {m[0] if m else 'not logged'} (want 12-20 Scrap Metal)")


def trash_piles_break(ctx):
    """08zo_trash: the Crowbar's swings take a trash pile's 50 Health (HittingTrash_Cue each), it
    breaks (WoodBreaking_Cue) and leaves the props, for 1-5 Scrap Metal."""
    m = re.search(r"autoshot trash broke after (\d+) clicks: scrap (\d+) removed (\w+)", ctx.log)
    if not m:
        ctx.err("08zo_trash: the pile didn't break (not reached, or the swings didn't hit)")
        return
    if not (1 <= int(m[2]) <= 5) or m[3] != "true":
        ctx.err(f"08zo_trash: scrap {m[2]}, removed {m[3]} (want 1-5 Scrap Metal)")
    hits = [float(h) for h in re.findall(r"harvest: trash .*? hit, (-?\d+) health left", ctx.log)]
    if not hits or hits[-1] > 0 or any(h <= 0 for h in hits[:-1]) or hits[0] >= 50:
        ctx.err(f"08zo_trash: health after each hit {hits} (want 50 counted down to <= 0)")
    if "No damage to trash object dealt." in ctx.log:
        ctx.err("08zo_trash: the crowbar was refused")
    sounds = re.search(r"autoshot sounds 08zo_trash (.*)", ctx.log)
    if not sounds or "HittingTrash_Cue=" not in sounds[1] or "WoodBreaking_Cue=" not in sounds[1]:
        ctx.err("08zo_trash: no HittingTrash_Cue / WoodBreaking_Cue")


def fishing_catches_a_fish(ctx):
    """08zp_fish: from the lake's shore the Fishing Rod's cast lands on the water (a buoy, the line
    out); at the bite a click reels in one Small / Raw / Large Raw Fish with its Fishing XP
    (20-25 / 40-50 / 80-100) and FishReelIN_Cue, and the line comes in."""
    if not re.search(r"autoshot fish cast: buoy at", ctx.log):
        m = re.search(r"fishing: no water along the cast.*", ctx.log)
        ctx.err(f"08zp_fish: the cast didn't land on water ({m[0] if m else 'not logged'})")
        return
    m = re.search(r"autoshot fish caught \[(\d+), (\d+), (\d+)\] \(was (\d+)\) xp ([\d.]+) still fishing (\w+)", ctx.log)
    if not m:
        ctx.err("08zp_fish: no catch logged (no bite, or the reel click was ignored)")
        return
    small, raw, large, was, xp, still = int(m[1]), int(m[2]), int(m[3]), int(m[4]), float(m[5]), m[6]
    if small + raw + large != was + 1 or still != "false":
        ctx.err(f"08zp_fish: fish {small}/{raw}/{large} (was {was}), still fishing {still} (want one fish, the line in)")
    want = (20, 25) if small else (40, 50) if raw else (80, 100)
    if not (want[0] <= xp <= want[1]):
        ctx.err(f"08zp_fish: {xp} Fishing XP (want {want[0]}-{want[1]})")
    sounds = re.search(r"autoshot sounds 08zp_fish (.*)", ctx.log)
    if not sounds or "FishReelIN_Cue=" not in sounds[1]:
        ctx.err("08zp_fish: no FishReelIN_Cue")


def vehicles_drive(ctx):
    """08zq_drive: vehicles spawn at the level's spawners (30 % chance); F gets in the nearest, W
    drives it off (speed, fuel burning) and over a roamer in the road (dead), S stops it and F
    gets out by its door."""
    m = re.search(r"vehicles: (\d+) spawned at (\d+)% chance", ctx.log)
    if not m or int(m[1]) == 0:
        ctx.err(f"08zq_drive: no vehicles spawned ({m[0] if m else 'not logged'})")
        return
    if not re.search(r"autoshot drive in: driving true", ctx.log):
        ctx.err("08zq_drive: F didn't get in")
        return
    # (The road ahead may end at a wall within the 3 s: the top speed counts, not the last.)
    moved = re.search(r"autoshot drive moved ([\d.]+) m, top ([\d.]+) mph, fuel used ([\d.]+)", ctx.log)
    if not moved or float(moved[2]) < 5 or float(moved[1]) < 3:
        ctx.err(f"08zq_drive: didn't drive off ({moved[0] if moved else 'not logged'})")
    elif not (0.3 < float(moved[3]) < 10):
        ctx.err(f"08zq_drive: fuel used {moved[3]} in 3 s (want about 1 per second moving)")
    r = re.search(r"roamer dead (\w+)", ctx.log)
    if not r or r[1] != "true" or not re.search(r"vehicles: ran over a zombie at", ctx.log):
        ctx.err("08zq_drive: the roamer in the road wasn't run over (300-350 at 5 mph or more)")
    s = re.search(r"autoshot drive stopped: driving true mph ([-\d.]+)", ctx.log)
    if not s or abs(float(s[1])) >= 1:
        ctx.err(f"08zq_drive: S didn't stop it ({s[0] if s else 'not logged'})")
    if "vehicles: engine sound on" not in ctx.log or "vehicles: lights on" not in ctx.log:
        ctx.err("08zq_drive: the engine sound or the headlights (L) didn't come on")
    st = re.search(r"autoshot drive storage: overlay (.*)", ctx.log)
    if not st or not st[1].startswith("Loot(") or "opened the" not in ctx.log:
        ctx.err(f"08zq_drive: Open Storage didn't open ({st[1] if st else 'not logged'})")
    o = re.search(r"autoshot drive out: driving (\w+) .* from the vehicle ([\d.]+) m", ctx.log)
    if not o or o[1] != "false" or float(o[2]) > 4:
        ctx.err(f"08zq_drive: F didn't get out by the door ({o[0] if o else 'not logged'})")


def laptop_quest_completes(ctx):
    """08zr_quest: scanning a laptop starts a Classified Container quest (QUEST STARTED, its map
    marker, the crate); within 10 m of it the quest moves on (QUEST UPDATED, marker gone); the
    crate's 4 s search opens it, a named weapon lands on the ground and the quest succeeds."""
    m = re.search(r'autoshot quest started (Quest_LaptopEvent_ClassifiedContainer\d+_C) state Container_Start box "QUEST STARTED" "CLASSIFIED CONTAINER" \["- [^"]+"\] marker true crate ClassifiedContainer\d+', ctx.log)
    if not m:
        s = re.search(r"autoshot quest (started|none).*", ctx.log)
        ctx.err(f"08zr_quest: the laptop didn't start a Classified Container quest ({s[0] if s else 'not logged'})")
        return
    a = re.search(r'autoshot quest arrived state Some\("QuestState_1"\) box "QUEST UPDATED" \["- [^"]+"\] marker false', ctx.log)
    if not a:
        s = re.search(r"autoshot quest arrived.*", ctx.log)
        ctx.err(f"08zr_quest: reaching the spot didn't move the quest on ({s[0] if s else 'not logged'})")
        return
    d = re.search(r'autoshot quest done outcome Some\(Succeeded\) opened true dropped \["(DA_\w+)"\] xp \((\d+), (\d+)\) -> \((\d+), (\d+)\) box "QUEST SUCCEEDED"', ctx.log)
    if not d:
        s = re.search(r"autoshot quest done.*", ctx.log)
        ctx.err(f"08zr_quest: searching the crate didn't finish the quest with a weapon ({s[0] if s else 'not logged'})")
    elif (int(d[4]), int(d[5])) <= (int(d[2]), int(d[3])):
        ctx.err(f"08zr_quest: no XP for the quest ({d[0]})")


def journal_lists_quests(ctx):
    """08zs_journal_quests: the QUESTS tab shows the page; the Quest Log folder lists the finished
    Classified Container ("Quest Log [N]" counts it); FORGET asks first and NO keeps the quest."""
    m = re.search(r'autoshot journal quests page Quests shown Some\("Quest_LaptopEvent_ClassifiedContainer\d+_C"\) log_tab true texts (\[.*\])', ctx.log)
    if not m:
        ctx.err("08zs_journal_quests: the QUESTS tab didn't open its page")
        return
    texts = m[1]
    if '"Classified Container"' not in texts or not re.search(r'"Quest Log \[[1-9]\d*\]"', texts) or '"TO DO:"' not in texts:
        ctx.err(f"08zs_journal_quests: page texts {texts}")
    if "autoshot journal quests forget asks true" not in ctx.log:
        ctx.err("08zs_journal_quests: FORGET didn't ask")
    if "autoshot journal quests no keeps true dialog false" not in ctx.log:
        ctx.err("08zs_journal_quests: NO didn't keep the quest and close the dialog")


def workbench_crafts(ctx):
    """08zt_workbench: F at a crafting station opens its crafting screen (CraftingWorkbench
    recipes); crafting Nails turns 5 Scrap Metal into 5 Nails and gives XP."""
    if not re.search(r"autoshot workbench open station true title true craftable true", ctx.log):
        s = re.search(r"autoshot workbench (open|never|none).*", ctx.log)
        ctx.err(f"08zt_workbench: F didn't open the station's crafting screen ({s[0] if s else 'not logged'})")
        return
    m = re.search(r'autoshot craft table CraftingWorkbench DA_Nails (\d+) -> (\d+) toasts \["Item Crafted, XP Given: \d+"\]', ctx.log)
    if not m or int(m[2]) - int(m[1]) != 5:
        s = re.search(r"autoshot craft .*", ctx.log)
        ctx.err(f"08zt_workbench: crafting Nails ({s[0] if s else 'not logged'})")


def elimination_quest_completes(ctx):
    """08zy_elimination: Eliminate Bandits 001's camp spawns with its 8 bandits at the goal;
    killing them completes the quest (Succeeded, 400 XP)."""
    if "autoshot elimination no marker" in ctx.log:
        ctx.err("08zy_elimination: no Bandit Camp marker")
        return
    if "quest prefab Prefab_Quest_BanditCamp1_C" not in ctx.log:
        ctx.err("08zy_elimination: the quest camp didn't spawn")
    m = re.search(r"autoshot elimination state (\S+) volumes (\d+) bandits (\d+)", ctx.log)
    if not m or int(m[2]) < 2 or int(m[3]) != 8:
        ctx.err(f"08zy_elimination: {m.groups() if m else 'not logged'} (want the volume's 8 bandits)")
    o = re.search(r"autoshot elimination outcome (\S+) xp \((\d+), (\d+)\) -> \((\d+), (\d+)\)", ctx.log)
    if not o or o[1] != "Some(Succeeded)" or (int(o[4]), int(o[5])) <= (int(o[2]), int(o[3])):
        ctx.err(f"08zy_elimination: outcome {o.groups() if o else 'not logged'} (want Succeeded and XP)")


def horde_beacon_runs(ctx):
    """08zze_horde / 08zzf_horde_end: build mode places a Horde Beacon on the ground by the spawn
    (green ghost; one of two used, build mode carries on; RMB stops it); F starts it ("Horde
    Beacon Activated...", the countdown) and wave 1's four BP_Zombie_Infected_Horde spawn within
    ~11 m of it. Killed and skipped to the last wave: "Final wave defeated!", the beacon gone and
    rewards on the ground."""
    g = re.search(r"autoshot horde ghost on (\w+) can build (\w+)", ctx.log)
    if not g or g.groups() != ("true", "true"):
        ctx.err(f"08zze_horde: the ghost {g.groups() if g else 'not logged'} (want build mode on and green)")
    p = re.search(r"autoshot horde placed (\d+) left (\d+) building (\w+)", ctx.log)
    if not p or p.groups() != ("1", "1", "true"):
        ctx.err(f"08zze_horde: placing {p.groups() if p else 'not logged'} (want 1 placed, 1 left, still building)")
    if "autoshot horde stopped building true" not in ctx.log:
        ctx.err("08zze_horde: RMB didn't stop build mode")
    h = re.search(r"autoshot horde started (\w+) wave (\d+) phase (.*?) enemies (\d+) away \[(.*?)\] toasts (.*)", ctx.log)
    if not h:
        ctx.err("08zze_horde: the horde wasn't logged")
    else:
        away = [float(x.strip('"')) for x in h[5].split(", ") if x]
        if h[1] != "true" or h[2] != "1" or h[4] != "4" or len(away) != 4 or max(away) > 11.0 or "Horde Beacon Activated..." not in h[6] or "Wave: 1 Begin!" not in h[6]:
            ctx.err(f"08zze_horde: {h.groups()} (want started, wave 1, 4 AI within ~11 m, the toasts)")
    e = re.search(r"autoshot horde end beacons (\d+) rewards \[(.*?)\] toasts (.*)", ctx.log)
    if not e or e[1] != "0" or not e[2] or "Final wave defeated!" not in e[3]:
        ctx.err(f"08zzf_horde_end: {e.groups() if e else 'not logged'} (want the beacon gone, rewards, the toast)")


def blood_moon_rises_and_settles(ctx):
    """08zzd_blood_moon: the Blood Moon turns the moonlight red, rouses the roamer (Alert Speed
    2.5 x 1.5 = 3.75 m/s) with "The Blood Moon is upon us..."; sunrise settles it."""
    m = re.search(r"autoshot blood moon on (\w+) roused (\w+) ran ([\d.]+) m/s light Some\(LinearRgba \{ red: ([\d.]+), green: ([\d.]+)", ctx.log)
    if not m or m[1] != "true" or m[2] != "true" or not (3.0 < float(m[3]) < 4.2) or float(m[4]) < 0.9 or float(m[5]) > 0.5 or "The Blood Moon is upon us..." not in ctx.log:
        ctx.err(f"08zzd_blood_moon: {m.groups() if m else 'not logged'} (want it on, the roamer roused at ~3.75 m/s, red moonlight, the toast)")
    s = re.search(r"autoshot blood moon sunrise on (\w+) roused (\w+) toasts (.*)", ctx.log)
    if not s or s[1] != "false" or s[2] != "false" or "Settling down" not in s[3]:
        ctx.err(f"08zzd_blood_moon: at sunrise {s.groups() if s else 'not logged'} (want it over, the roamer calm, the toast)")


def turrets_fire_and_go_down(ctx):
    """08zzc_turret: a bandit turret sees the survivor 12 m in front and shoots; held F from behind
    for 5 s disables it and drops a Turret Gun; a 100-damage round destroys another."""
    if "autoshot turret none" in ctx.log:
        ctx.err("08zzc_turret: no bandit turrets (actors.json / the gun meshes)")
        return
    s = re.search(r"autoshot turret sees player (\w+) ammo (\d+)", ctx.log)
    if not s or s[1] != "true" or "turret shot the survivor" not in ctx.log:
        ctx.err(f"08zzc_turret: {s.groups() if s else 'not logged'} (want the survivor as its target and shots)")
    d = re.search(r"autoshot turret disabled (\w+) gun dropped (\w+)", ctx.log)
    if not d or d.groups() != ("true", "true"):
        ctx.err(f"08zzc_turret: disabling {d.groups() if d else 'not logged'} (want disabled and a Turret Gun)")
    k = re.search(r"autoshot turret shot down Some\(\((\w+), ([\d.]+)\)\)", ctx.log)
    if not k or k[1] != "true":
        ctx.err(f"08zzc_turret: the shot-at turret {k.groups() if k else 'not logged'} (want destroyed)")


def settlement_stands(ctx):
    """08zzb_settlement: BP_SettlementManager picked one of its 4 camps and it spawned; standing
    in it the survivor is in its safe zone ("Now Entering Scavenger Safe Zone", the badge), with
    its two traders, a well, the crafting and cooking stations and its map marker around; the
    Settlement Officer sells intel for 2500 (a laptop quest, "Quest pinned to your map", its
    lines) and the settlers walk about."""
    m = re.search(r"autoshot settlement (\d+) of (\d+) camps: (\S+) at .* spawned (\w+)", ctx.log)
    if not m or m[2] != "4" or m[4] != "true":
        ctx.err(f"08zzb_settlement: {m.groups() if m else 'not logged'} (want one of 4 camps, spawned)")
        return
    z = re.search(r'autoshot settlement zone Some\("([^"]*)"\) safe (\w+) banner (\[.*?\]) traders (\[.*?\]) wells (\d+) stations (\[.*?\]) marker Some\("([^"]*)"\)', ctx.log)
    if not z:
        ctx.err("08zzb_settlement: not in a safe zone, or no marker (" + (re.search(r"autoshot settlement zone .*", ctx.log) or [""])[0][-200:] + ")")
        return
    name, safe, banner, sellers, wells, stations, marker = z.groups()
    if safe != "true" or f"Now Entering:{name}" not in banner or "BP_SafeZoneTrader_C" not in sellers or "BP_BlackMarketTrader_C" not in sellers \
            or int(wells) < 1 or "BP_CraftingStation_C" not in stations or "BP_CookingStation_C" not in stations or marker != name:
        ctx.err(f"08zzb_settlement: {z.groups()} (want the zone's banner, both traders, a well, both stations, the marker)")
    o = re.search(r"autoshot settlement officer (Some|None)", ctx.log)
    i = re.search(r"autoshot settlement intel cash (\d+) quests (\[.*?\]) lines (\[.*?\]) overlay (\S+) toasts (\[.*?\])$", ctx.log, re.M)
    if not o or o[1] != "Some" or not i or i[1] != "500" or "Quest_LaptopEvent_" not in i[2] or "marked the location on your GPS" not in i[3] or "Quest pinned to your map" not in i[5]:
        ctx.err(f"08zzb_settlement: the Officer's intel {i.groups() if i else 'not logged'} (want 2500 of 3000 cash taken, a laptop quest, its lines, the toast)")
    c = re.search(r"autoshot settlement settlers (\d+) (.*)", ctx.log)
    if not c or int(c[1]) < 2 or "BP_RandomCivilian_" not in c[2]:
        ctx.err(f"08zzb_settlement: settlers {c.groups() if c else 'not logged'} (want the camp's settlers about)")


def wandering_traders(ctx):
    """08zz_wanderer: the level's random-chance volumes rolled in groups (some wandering
    traders); the military and Shakes & Cones traders stand on the ground at a spawn point; two
    wandering traders met ("Press [F] to Trade with Wandering Trader"), the one talked to stood
    still and kept still trading, the one killed gave its toast and lost its trade."""
    v = re.search(r"autoshot wanderer volumes groups (\d+) wandering (\d+)", ctx.log)
    if not v or int(v[1]) < 10:
        ctx.err(f"08zz_wanderer: volume groups {v.groups() if v else 'not logged'} (want the rolled-in random-chance volumes)")
    placed = re.findall(r"autoshot wanderer spawned trader (\w+) at .* above ground (-?[\d.]+)", ctx.log)
    if sorted(c for c, _ in placed) != ["BP_MilitaryTrader_C", "BP_ShakesConesTrader_C"] or any(abs(float(h)) > 0.5 for _, h in placed):
        ctx.err(f"08zz_wanderer: spawned traders {placed} (want the military and Shakes & Cones traders on the ground)")
    m = re.search(r'autoshot wanderer met (\d+) prompt Some\("([^"]*)"\)', ctx.log)
    if not m or m[1] != "2" or m[2] != "Press [F] to Trade with Wandering Trader":
        ctx.err(f"08zz_wanderer: met {m.groups() if m else 'not logged'} (want 2 traders and the Trade prompt)")
    h = re.search(r"autoshot wanderer talking held Some\((\w+)\) moved Some\(([\d.e-]+)\)", ctx.log)
    if not h or h[1] != "true" or float(h[2]) > 0.1:
        ctx.err(f"08zz_wanderer: while talking {h.groups() if h else 'not logged'} (want it held still)")
    k = re.search(r"autoshot wanderer killed dead (\w+) trade gone (\w+) corpse (\w+) A held (\w+) overlay Loot\(\d+\) toasts (.*)", ctx.log)
    if not k or k.groups()[:4] != ("true", "true", "true", "true") or "Wandering Trader Killed, XP Given" not in k[5]:
        ctx.err(f"08zz_wanderer: after the kill {k.groups() if k else 'not logged'} (want B dead with its trade gone, A held on its trade screen, the toast)")


def bandits_fight_and_die(ctx):
    """08zx_bandits: a bandit camp's volume spawns armed bandits that shoot at the survivor 25 m
    off; killed, they're human kills with the bandit toast and lootable corpses."""
    m = re.search(r"autoshot bandits humans (\d+) \[(.*)\]", ctx.log)
    if not m:
        ctx.err("08zx_bandits: not logged" + (" (no camp site)" if "autoshot bandits no camp site" in ctx.log else ""))
        return
    n, who = int(m[1]), m[2]
    if n < 2 or "BP_AIWeapon_" not in who:
        ctx.err(f"08zx_bandits: humans {n} [{who}] (want the volume's armed bandits)")
    if not re.search(r"BP_Bandit_\w+ shot the survivor: \w+ for \d+", ctx.log) and not re.search(r"BP_Bandit_\w+ hit the survivor", ctx.log):
        ctx.err("08zx_bandits: no bandit shot or hit the survivor")
    k = re.search(r"autoshot bandits kills (\d+) corpses (\d+) lootable (\d+) toasts (.*)", ctx.log)
    if not k or int(k[1]) < n or int(k[2]) < n or int(k[3]) < 1 or "Bandit Killed, XP Given" not in k[4]:
        ctx.err(f"08zx_bandits: after the kill {k.groups() if k else 'not logged'} (want {n} human kills, corpses, the bandit toast)")


def prefab_camp_spawns(ctx):
    """08zw_prefab: an abandoned camp (BP_PrefabSpawner -> Prefab_AbandonedCamp*) spawns: all its
    meshes drawn, its 5 crates searchable and solid, its zombie spawner added."""
    m = re.search(r"autoshot prefab (\S+) spawned (\w+) meshes (\d+) hidden (\d+) containers (\d+) spawners (\d+) crate_hit (\w+)", ctx.log)
    if not m:
        ctx.err("08zw_prefab: not logged" + (" (no camp site)" if "autoshot prefab no camp site" in ctx.log else ""))
        return
    name, spawned, meshes, hidden, boxes, zombies, hit = m.groups()
    if spawned != "true" or int(meshes) < 12 or hidden != "0" or boxes != "5" or int(zombies) < 1 or hit != "true":
        ctx.err(f"08zw_prefab: {m.groups()} (want spawned, nothing hidden, 5 containers, a zombie spawner, the crate solid)")


def particles_play(ctx):
    """08zv_fx: the barrel fire's FX_Fire is burning, a frag blast bursts and goes once its smoke
    has died, every firearm's muzzle flash and brass are known systems, and no Fx named a
    system that isn't in particles.json (the shots, blasts and hits elsewhere in the tour)."""
    m = re.search(r"autoshot fx fire live (\d+) active (\w+) at ([\d.]+) m; (\d+) level emitters", ctx.log)
    if not m or int(m[1]) < 10 or m[2] != "true" or float(m[3]) > 1.5 or int(m[4]) < 100:
        ctx.err(f"08zv_fx: barrel fire {m.groups() if m else 'not logged'} (want 10+ live particles, spawning, 100+ level emitters)")
    g = re.search(r"autoshot fx guns (\d+) flashes (\d+) brass (\d+)", ctx.log)
    if not g or int(g[1]) < 40 or int(g[2]) < int(g[1]) - 2 or g[3] != g[1]:
        ctx.err(f"08zv_fx: muzzle flashes {g.groups() if g else 'not logged'} (want every gun's flash and brass known)")
    b = re.search(r"autoshot fx blast emitters (\d+) live (\d+) off ([\d.]+|NaN) m", ctx.log)
    if not b or b[1] != "1" or int(b[2]) < 100 or b[3] == "NaN" or float(b[3]) > 3.0:
        ctx.err(f"08zv_fx: blast {b.groups() if b else 'not logged'} (want one emitter with its 135 burst particles around it)")
    if "autoshot fx blast gone true" not in ctx.log:
        ctx.err("08zv_fx: the blast's emitter was still there 6 s on")
    missing = re.findall(r"particles: no system (\S+)", ctx.log)
    if missing:
        ctx.err(f"particles: unknown systems {sorted(set(missing))}")


def boss_swings_and_dies(ctx):
    """08zu_boss: an Undead Boss 8 m away sees the survivor, closes in and swings until RightArmTrace
    lands (DamageToDo 30, less armour); killed, it toasts "Zombie Boss Killed, XP Given", adds a
    boss kill and its corpse is an "Undead Boss" container (ZombieBossSmall) with its unique sets."""
    if "autoshot boss placed" not in ctx.log:
        ctx.err("08zu_boss: no boss placed")
        return
    hit = re.search(r"autoshot boss hit for (\d+) at [\d.]+ s from ([\d.]+) m after (\d+) swings", ctx.log)
    if not hit or not 5 <= int(hit[1]) <= 30:
        ctx.err(f"08zu_boss: hit {hit.groups() if hit else 'none'} (want DamageToDo 30 less armour)")
    m = re.search(r"autoshot boss brain (\w+) swings (\d+) hit (\w+) from_player ([\d.]+) scale ([\d.]+)", ctx.log)
    if not m or m[1] != "Chase" or int(m[2]) < 1 or float(m[4]) > 2.0 or abs(float(m[5]) - 1.2) > 0.01:
        ctx.err(f"08zu_boss: boss {m.groups() if m else 'not logged'} (want it chasing at the survivor, swinging, drawn 1.2x)")
    hp = re.search(r"autoshot boss hp (\d+)", ctx.log)
    if not hp or not 1000 <= int(hp[1]) <= 10000:
        ctx.err(f"08zu_boss: max hp {hp[1] if hp else 'not logged'} (StandardHealthValue 2000 x ZOMBIE HEALTH MULTIPLIER)")
    m = re.search(r"autoshot boss dead (\w+) boss_kills (\d+) corpse (\w+) container (.+?) unique (\S*) toast \"(.*?)\"", ctx.log)
    if not m:
        ctx.err("08zu_boss: the kill wasn't logged")
        return
    if m[1] != "true" or int(m[2]) < 1:
        ctx.err(f"08zu_boss: dead {m[1]}, boss kills {m[2]}")
    if m[3] != "true" or m[4] != 'Some(("Undead Boss", Some("ZombieBossSmall")))' or m[5] != "ColonelsRevenge,Crusher,Deadeye":
        ctx.err(f"08zu_boss: corpse {m[3]} {m[4]} unique {m[5]!r}")
    if not m[6].startswith("Zombie Boss Killed, XP Given: "):
        ctx.err(f"08zu_boss: toast {m[6]!r}")


CHECKS = [horde_beacon_runs, blood_moon_rises_and_settles, turrets_fire_and_go_down, settlement_stands, wandering_traders, elimination_quest_completes, bandits_fight_and_die, prefab_camp_spawns, particles_play, boss_swings_and_dies, workbench_crafts, journal_lists_quests, laptop_quest_completes, journal_info_shows_the_stats, vehicles_drive, fishing_catches_a_fish, trees_fall_and_wrecks_salvage, trash_piles_break, ran_cleanly, every_step_has_a_screenshot, traders_trade, airdrops_fall_and_open, shots_kill_the_zombie, lockpick_opened, door_opens, keypad_opens_the_door, clothing_hides_the_body, ladder_climbs, lab_generator_lights_the_lab, shove_and_stomp, frag_grenade_explodes, molotov_sets_the_zombie_burning, damage_numbers_float, recoil_kicks, passive_skills_gain_xp, journal_unlocks_skills, map_shows_discovered_poi, music_plays, combat_roll_rolls, fence_is_climbed_over, animals_flee_bite_and_die, swimming_floats_dives_and_surfaces, sounds_play, body_parts_move_in_step, police_garage_opens, crouch_is_slow_and_quiet, hard_landing_hurts, flashlight_lights_up, sitting_holds_still, night_is_darker_than_day, frame_rate, level_spawners_wake, hound_has_its_health, corpse_opens_as_loot, medical_icons_show, weather_changes_the_scene]


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
