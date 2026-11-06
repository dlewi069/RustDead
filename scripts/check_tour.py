#!/usr/bin/env python3
"""Assertions on a screenshot tour run (`SD_MENU_SHOTS=<dir> cargo run`, see scripts/check.sh).

usage: check_tour.py <run.log> <shots dir> <exit code>

Every step in src/menus/autoshot.rs's TOUR must have produced a non-blank PNG, the run must not
panic, and the behaviour each step exercises must show up (shots hit, lockpick opened, night is
darker than day...). Add a check: a function taking `ctx`, listed in CHECKS.
"""
import math
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


def barber_restyles_the_survivor(ctx):
    """08zzzb_barber / 08zzzc_barber_exit: F at the barber opens CharacterBarberMenu with the view
    blended onto its Camera, the survivor at its Debug point, the HUD hidden; a switch to Female
    rebuilds the survivor (one head, the old parts gone); Esc closes it, the view blends back to
    the spring arm and the HUD returns."""
    if "autoshot barber: none" in ctx.log:
        ctx.err("08zzzb_barber: no barber in the level")
        return
    o = re.search(r"autoshot barber open (\S+) seat ([\d.]+) camera ([\d.]+|NaN) hud (\w+)", ctx.log)
    if not o or not o[1].startswith("Barber(") or float(o[2]) > 0.3 or o[3] == "NaN" or float(o[3]) > 0.05 or o[4] != "false":
        ctx.err(f"08zzzb_barber: open {o.groups() if o else 'not logged'} (want Barber(n), at the seat, the view on its camera, no HUD)")
    r = re.search(r"autoshot barber restyled heads (\d+) outgoing (\d+) built (\w+) sex (\w+)", ctx.log)
    if not r or r.groups() != ("1", "0", "true", "Female"):
        ctx.err(f"08zzzb_barber: restyle {r.groups() if r else 'not logged'} (want one head, nothing left going out, built from the new profile, Female)")
    e = re.search(r"autoshot barber exit (\S+) camera ([\d.]+|NaN) view (\w+) hud (\w+)", ctx.log)
    if not e or e[1] != "None" or e[2] == "NaN" or float(e[2]) < 0.5 or e[3] != "false" or e[4] != "true":
        ctx.err(f"08zzzc_barber_exit: {e.groups() if e else 'not logged'} (want closed, the view back on the spring arm, the HUD back)")


def stairwell_and_siren(ctx):
    """08zzzd_stairwell: F on a BP_LaboratoryStairwellLightSwitch lights its stairwell one light
    at a time (0.6-1.2 s apart: 3-6 of 6 lit 3.1 s after F, the walk still running), with the hum.
    08zzze_siren: at 22:00 the flood (4) and pole (2) lights are on and the road lights (7)
    always; F on a BP_Siren starts it (Siren_Cue fading in over 5 s), F again stops it (re-armed)."""
    if "autoshot stairwell: none" in ctx.log:
        ctx.err("08zzzd_stairwell: no stairwell switch in the level")
    b = re.search(r"autoshot stairwell lit (\d+) hums (\d+)", ctx.log)
    a = re.search(r"autoshot stairwell after lit (\d+) of (\d+) hums (\d+) busy (\w+)", ctx.log)
    if not b or not a or b[1] != "0" or not 3 <= int(a[1]) <= 6 or a[4] != "true" or int(a[3]) <= int(b[2]):
        ctx.err(f"08zzzd_stairwell: before {b.groups() if b else 'not logged'} after {a.groups() if a else 'not logged'} (want dark, then 3-6 lit one by one with their hum)")
    if "autoshot siren: none" in ctx.log:
        ctx.err("08zzze_siren: no siren in the level")
    n = re.search(r"autoshot siren night lights flood (\d+) pole (\d+) road (\d+)", ctx.log)
    if not n or n.groups() != ("4", "2", "7"):
        ctx.err(f"08zzze_siren: night lights {n.groups() if n else 'not logged'} (want 4 floods, 2 poles, 7 road lights on)")
    before = re.search(r"autoshot siren before playing (\w+) starts (\d+)", ctx.log)
    on = re.search(r"autoshot siren on playing (\w+) gain ([\d.]+) starts (\d+)", ctx.log)
    off = re.search(r"autoshot siren off playing (\w+) gain ([\d.]+) rearmed (\w+)", ctx.log)
    if not before or not on or before[1] != "false" or on[1] != "true" or not 0.3 < float(on[2]) < 0.8 or int(on[3]) <= int(before[2]):
        ctx.err(f"08zzze_siren: F {before.groups() if before else 'not logged'} -> {on.groups() if on else 'not logged'} (want playing, fading in, Siren_Cue started)")
    if not off or off[1] != "false" or float(off[2]) <= 0.0 or off[3] != "true":
        ctx.err(f"08zzze_siren: F again {off.groups() if off else 'not logged'} (want stopped, fading out, the random timer re-armed)")


def easter_eggs_show_and_the_nuke_blows(ctx):
    """08zzzf_eggs: hidden until UDS's midnight; then all 7 apparitions show (60 s), the campfire
    hiding (flames out) with the survivor within 15 m and back from 18 m, the nearest ghost
    turned to face the survivor (within 10 deg: the Blueprint turns it only on its 0.2 s timer, by
    RInterpTo with the frame's delta, so at 60 fps a quarter of the gap a tick; 2.3 s after the
    survivor's last move it was 5.0 deg off on 2026-10-30) and hovering. 08zzzg_nuke: a blast sets the nuke off once; the
    roamer 20 m from it takes 250-500, the survivor 130 m off nothing."""
    if "autoshot eggs: no campfire" in ctx.log:
        ctx.err("08zzzf_eggs: no easter-egg campfire in the level")
    b = re.search(r"autoshot eggs before shown (\d+) active (\d+) flames (\d+)", ctx.log)
    if not b or b.groups() != ("0", "0", "0"):
        ctx.err(f"08zzzf_eggs: before midnight {b.groups() if b else 'not logged'} (want nothing shown, no flames)")
    n = re.search(r"autoshot eggs near campfire hidden (\w+) flames (\d+)", ctx.log)
    if not n or n.groups() != ("true", "0"):
        ctx.err(f"08zzzf_eggs: within 10 m {n.groups() if n else 'not logged'} (want the campfire hidden, its flames out)")
    m = re.search(r"autoshot eggs midnight shown (\d+) of (\d+) active (\d+) campfire hidden (\w+) flames (\d+) ghost facing off ([\d.-]+) lift ([\d.-]+)", ctx.log)
    if not m or m[1] != m[2] or m[2] != "7" or m[3] != "7" or m[4] != "false" or m[5] == "0" or not 0.0 <= float(m[6]) < 10.0 or not 0.0 < float(m[7]) <= 0.25:
        ctx.err(f"08zzzf_eggs: after midnight {m.groups() if m else 'not logged'} (want all 7 shown, the campfire lit from 18 m, the ghost facing within 10 deg and lifted)")
    if "autoshot nuke: none" in ctx.log:
        ctx.err("08zzzg_nuke: no nuke in the level")
    hp = re.search(r"autoshot nuke roamer hp ([\d.-]+)", ctx.log)
    a = re.search(r"autoshot nuke after exploded (\w+) roamer hp ([\d.-]+) survivor ([\d.-]+) -> ([\d.-]+) away ([\d.]+)", ctx.log)
    hits = ctx.log.count("nuke exploded at")
    if not hp or not a or a[1] != "true" or hits != 1 or not 250.0 <= float(hp[1]) - float(a[2]) <= 500.0 or a[3] != a[4]:
        ctx.err(f"08zzzg_nuke: roamer {hp.groups() if hp else 'not logged'} after {a.groups() if a else 'not logged'}, {hits} explosions (want one, 250-500 off the roamer, the survivor untouched)")


def ai_health_bar_follows_the_shot_zombie(ctx):
    """08g_armed: looking at the zombie brings up the AI health bar (its name, the health a trace
    later, full), the shots take it down, and it goes when the zombie dies."""
    seen = re.findall(r'autoshot ai bar (Some\(\("([^"]*)", (\w+)\)\)|None) text (?:Some\("([\d,]+) / ([\d,]+)"\)|None)', ctx.log)
    texts = [(int(h.replace(",", "")), int(m.replace(",", ""))) for _, _, _, h, m in seen if h]
    names = {n for _, n, _, _, _ in seen if n}
    ok = names == {"Undead Shambler"} and texts and texts[0][0] == texts[0][1] and any(h < m for h, m in texts) and seen[-1][0] == "None"
    if not ok:
        ctx.err(f"08g_armed: AI health bar {seen[:6]} (want 'Undead Shambler', full, then lower, then hidden once dead)")


def binoculars_zoom_and_tag(ctx):
    """08zzzi_binoculars / 08zzzj_binoculars_down: B raises the military binoculars (12x: the
    first-person field of view 90 / 12 = 7.5, the HUD and head hidden, the camera at the eyes,
    the range shown); LMB tags the held roamer 25 m off (live, the skull, 1.3 m up) and the range
    reads its distance; F lowers them (third person, 90, HUD and head back) and the tag stays."""
    up = re.search(r"autoshot binoculars up (\w+) zoom ([\d.]+) distance shown (\w+) fov ([\d.-]+) hud hidden (\w+) head hidden (\w+) camera ([\d.-]+) m up", ctx.log)
    if not up or up.group(1, 2, 3, 5, 6) != ("true", "12", "true", "true", "true") or abs(float(up[4]) - 7.5) > 0.05 or not 1.3 <= float(up[7]) <= 1.9:
        ctx.err(f"08zzzi_binoculars: raised {up.groups() if up else 'not logged'} (want up, 12x, the range, FOV 7.5, HUD and head hidden, the camera at the eyes)")
    tag = re.search(r'autoshot binoculars tag Some\(\(true, "T_HumanSkull_256x256", 1\.3\)\) distance (\d+) roamer ([\d.]+) m', ctx.log)
    if not tag or not (float(tag[2]) - 1.5) * 100 <= int(tag[1]) <= float(tag[2]) * 100:
        ctx.err(f"08zzzi_binoculars: tag {tag.groups() if tag else 'none'} (want the roamer's skull and the range to it)")
    down = re.search(r"autoshot binoculars down active (\w+) fov ([\d.-]+) hud hidden (\w+) head hidden (\w+) tag live (\w+)", ctx.log)
    if not down or down.group(1, 3, 4, 5) != ("false", "false", "false", "true") or abs(float(down[2]) - 90.0) > 0.05:
        ctx.err(f"08zzzj_binoculars_down: {down.groups() if down else 'not logged'} (want them down, FOV 90, HUD and head back, the tag kept)")


def interaction_montages_play(ctx):
    """08c_loot / 08zzs_shower: opening a container plays Loot_MidShelf_GrabItem_Montage, a
    buildable's Option1 (the shower's Interact) Loot_TableTop_LeftHand_Montage."""
    for kind, step in (("Grab", "08c_loot"), ("TableTop", "08zzs_shower")):
        if f"montage: {kind}" not in ctx.log:
            ctx.err(f"{step}: no {kind} montage logged")


def zombie_eyes_glow(ctx):
    """08zzda_eyes / 08zzd_blood_moon: a roamer's two eye boxes (3 cm, about 8 cm apart, at head
    height) in M_GlowingEyes_Roamer (orange x 10); the Blood Moon swaps M_GlowingEyes_BloodMoon
    (red x 150) on, sunrise the Roamer's back."""
    pat = r"parts (\d+) colour ([\d.,-]+) gap ([\d.]+) cm up ([\d.-]+) m shown (\w+) size ([\d.]+) cm"
    e = re.search(r"autoshot eyes roamer " + pat, ctx.log)
    if not e or e[1] != "2" or e[2] != "10.00,5.03,0.00" or not 6.0 <= float(e[3]) <= 11.0 or not 1.2 <= float(e[4]) <= 1.9 or e[5] != "true" or abs(float(e[6]) - 3.0) > 0.2:
        ctx.err(f"08zzda_eyes: {e.groups() if e else 'not logged'} (want 2 orange 3 cm eyes ~8 cm apart at head height)")
    want = {"before": "10.00,5.03,0.00", "moon": "150.00,1.43,0.00", "sunrise": "10.00,5.03,0.00"}
    for when, colour in want.items():
        m = re.search(rf"autoshot blood moon eyes {when} " + pat, ctx.log)
        if not m or m[1] != "2" or m[2] != colour:
            ctx.err(f"08zzd_blood_moon: eyes {when} {m.groups()[:2] if m else 'not logged'} (want 2 in {colour})")


def more_interaction_montages(ctx):
    """08q_door / 08qa_well / 08zq_drive / 08zzc_turret: the container, warehouse and lab doors
    play Loot_MidShelf_GrabItem on F; a well's Drink Loot_TableTop_LeftHand (and quenches thirst);
    the vehicle's storage the grab at 1.5x; the bandit turret's disabling hold the grab."""
    log = ctx.log
    d = re.search(r"autoshot door (BP_\w+) at", log)
    if d and d[1] in ("BP_ContainerDoors_C", "BP_WarehouseDoors_C", "BP_LaboratoryEntranceDoors_C", "BP_LaboratorySecureDoors_C"):
        span = log[d.end():log.find("autoshot door open", d.end())]
        if "montage: Grab" not in span:
            ctx.err(f"08q_door: no Grab montage working the {d[1]}")
    w = re.search(r"autoshot well \d+ at .* water ([\d.]+)", log)
    drank = re.search(r"autoshot well drank: thirst ([\d.-]+) water ([\d.]+)", log)
    if not w or not drank:
        ctx.err(f"08qa_well: well {'found' if w else 'not found'}, drink {'logged' if drank else 'not logged'}")
    else:
        if "montage: TableTop" not in log[w.end():drank.start()]:
            ctx.err("08qa_well: no TableTop montage on Drink")
        if float(drank[1]) <= 50 or float(drank[2]) >= float(w[1]):
            ctx.err(f"08qa_well: thirst {drank[1]} (from 50), water {w[1]} -> {drank[2]} (want thirst up, water down)")
    v = log.find("vehicles: opened the")
    if v >= 0 and "montage: Grab at Some(1.5)" not in log[max(0, v - 2000):v + 2000]:
        ctx.err("08zq_drive: the vehicle's storage played no Grab at 1.5x")
    t = re.search(r"turret \d+: F \(behind true\)", log)
    if t and "montage: Grab" not in log[t.end():t.end() + 2000]:
        ctx.err("08zzc_turret: no Grab montage starting the disabling hold")


def infestation_haze_comes_and_goes(ctx):
    """08zzu_infestation: standing at the primary its PostProcess sphere (PostProcessRadius) is at
    full weight, the secondary has none; once the primary is destroyed it's gone."""
    b = re.search(r"autoshot infestation haze before primary ([\d.]+) radius Some\(([\d.]+)\) secondary ([\d.]+)", ctx.log)
    a = re.search(r"autoshot infestation haze after primary ([\d.]+)", ctx.log)
    if not b or float(b[1]) != 1.0 or float(b[2]) < 25.0 or float(b[3]) != 0.0 or not a or float(a[1]) != 0.0:
        ctx.err(f"08zzu_infestation: haze {b.groups() if b else 'not logged'} after {a.groups() if a else 'not logged'} (want 1 at the primary, none for the secondary, none once destroyed)")


def head_gear_is_worn(ctx):
    """08zzzh_head_gear: a hat, mask, glasses and shemagh hang on their sockets and show; the
    hair under the hat and the beard under the mask are hidden; HIDE PLAYER HEADWEAR? takes the
    hat off and shows the hair."""
    m = re.search(r"autoshot head gear worn (\d+) shown \[([\w,]*)\] hair hidden (\d+) of (\d+) beard hidden (\d+) of (\d+)", ctx.log)
    if not m or m[1] != "4" or m[2] != "Headwear,Facewear,Eyewear,Accessory" or m[3] != m[4] or m[5] != m[6] or m[4] == "0" or m[6] == "0":
        ctx.err(f"08zzzh_head_gear: {m.groups() if m else 'not logged'} (want 4 pieces shown, the hair and beard hidden)")
    h = re.search(r"autoshot head gear hide setting hat shown (\w+) hair hidden (\d+) of", ctx.log)
    if not h or h.groups() != ("false", "0"):
        ctx.err(f"08zzzh_head_gear: HIDE PLAYER HEADWEAR? {h.groups() if h else 'not logged'} (want the hat hidden, the hair shown)")


def hip_fire_raises_the_gun(ctx):
    """08g_armed: a shot without aiming sets HipFire? (3 s), which Player_AnimBP ORs into its
    IsADS: the Ironsights pose, facing where the camera looks (Set Aimed Focus)."""
    m = re.search(r"autoshot hip fire aiming (\w+) hip ([\d.]+) loco Some\((\w+)\) facing off (\d+)", ctx.log)
    if not m or m[1] != "false" or not 2.0 < float(m[2]) <= 3.0 or not m[3].startswith("Aim") or int(m[4]) > 20:
        ctx.err(f"08g_armed: hip fire {m.groups() if m else 'not logged'} (want not aiming, HipFire? 2-3 s left, an Aim* pose, facing the camera within 20 deg)")


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


def notes_are_read(ctx):
    if "autoshot letter: no notes" in ctx.log:
        ctx.err("08sa_letter: the level has no BP_StandardNote")
        return
    m = re.search(r'autoshot letter overlay (\S+) shown (\w+) first "(.*)"', ctx.log)
    if not m:
        ctx.err("08sa_letter: the note was never checked")
    elif not m.group(1).startswith("Letter(") or m.group(2) != "true":
        ctx.err(f"08sa_letter: F didn't show the note (overlay {m.group(1)}, text shown {m.group(2)})")
    elif not m.group(3).startswith("SUBJECT: KEYCARD CHANGE"):
        ctx.err(f"08sa_letter: wrong note text: {m.group(3)!r}")


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
    """FloatingDamageNumbersUI: shots, swings and blasts that hurt a zombie show their damage,
    placed on what was hit (ours, the user's request: the game puts them at the screen centre):
    on screen, and not all at the centre (the frag's roamer, the swing's zombie are elsewhere)."""
    nums = re.findall(r"damage number (-?\d+)", ctx.log)
    placed = [tuple(map(float, m)) for m in re.findall(r"damage number placed (-?\d+) (-?\d+) of (\d+) (\d+)", ctx.log)]
    if not placed:
        ctx.err("no damage number was placed on screen")
    elif not any(abs(x - w / 2) > 40 or abs(y - h / 2) > 40 for x, y, w, h in placed):
        ctx.err(f"every damage number sat at the screen centre ({placed[:5]}...), not on what was hit")
    if not nums:
        ctx.err("no floating damage numbers were shown")
    elif any(int(n) == 0 for n in nums):
        ctx.err("a damage number of 0 was shown (SpawnFloatingDamageText skips those)")


def recoil_kicks(ctx):
    """StartRecoil: every shot rolls a kick (guns kick up: negative pitch input), and the
    sideways part turns the camera (the tour pins the pitch, so only the yaw shows)."""
    # Up to 08zzw_launchers: the Flare Gun's RecoilVertical is +0.5, so it kicks down in the game.
    before = ctx.log.split("autoshot launchers armed at", 1)[0]
    kicks = [(float(p), float(y)) for p, y in re.findall(r"recoil kick (-?[\d.]+) (-?[\d.]+)", before)]
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
        # Only whole "Skill 1.23" pairs: another thread's output can cut a line short.
        xp[step] = {k: float(v) for k, v in re.findall(r"([A-Za-z]+) (\d+\.\d+)(?=\s|$)", rest)}
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
    # POIDiscoveredUI's FadeStart slides ZoneArea in from -525 px (in by 1.51 s).
    for age, left, want in re.findall(r"autoshot map banner age ([\d.]+) left (-?[\d.]+|NaN) want (-?[\d.]+)", ctx.log):
        if left == "NaN" or abs(float(left) - float(want)) > 30.0:
            ctx.err(f"08zb_map: the banner at {age} s sits at {left} px, FadeStart's slide wants {want}")
    level, xp = int(disc.group(4)), int(disc.group(2))
    # Either rounding of an exact half (level 5: 262.5 -> 263 in f32; Python's round gives 262).
    want = {f(l * k * 1.05) for l in (level, level - 1) for k in (50, 250) for f in (math.floor, math.ceil)}
    if xp not in want:
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
    c = re.search(r"autoshot deer corpse players (\d+) moving clips (\d+)", ctx.log)
    if not c or int(c[1]) == 0 or c[2] != "0":
        ctx.err(f"08zg_deer: the corpse {c.groups() if c else 'not logged'} (want its animation held: no clip moving)")
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
    # DamageToDo 20, less whatever armour the survivor wears. The wolf's own hit as zombie_hit
    # logs it (the step's health drop can catch a hunger or bleed tick: 2 points, 2026-10-30).
    bite = re.search(r"survivor hit by Wolf for ([\d.]+)", ctx.log)
    if not bite or not 5.0 <= float(bite[1]) <= 20.0:
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


def level_traps(ctx):
    """08zzq_level_traps: an AI on a level landmine sets it off (the mine and its prop gone, the AI
    hurt by 200-225 or dead); standing in a barbed wire: 8-12 on entry, then 10 every 0.5 s (two
    in 1.2 s); out of it, its timer on the survivor stops and nothing more."""
    m = re.search(r"autoshot level traps mine left (\w+) prop gone (\w+) ai (.*)", ctx.log)
    if not m or m[1] != "false" or m[2] != "true":
        ctx.err(f"08zzq_level_traps: mine {m.groups() if m else 'not logged'} (want it gone with its prop)")
    else:
        a = re.match(r"Some\(\(([\d.]+), (None|Some\(\(([-\d.]+), (\w+)\)\))\)\)", m[3])
        if not a or (a[2] != "None" and a[4] != "true" and not 195.0 <= float(a[1]) - float(a[3]) <= 230.0):
            ctx.err(f"08zzq_level_traps: the AI {m[3]} (want hurt by 200-225 or dead)")
    w = re.search(r"autoshot level traps wire hits \[(.*?)\]", ctx.log)
    hits = [float(x) for x in w[1].split(", ") if x] if w else []
    if len(hits) != 3 or not 8.0 <= hits[0] <= 12.0 or hits[1:] != [10.0, 10.0]:
        ctx.err(f"08zzq_level_traps: wire hits {hits} (want 8-12, 10, 10)")
    o = re.search(r"autoshot level traps wire after leaving timer (\w+) hits \[(.*?)\]", ctx.log)
    if not o or o[1] != "false" or o[2] != "":
        ctx.err(f"08zzq_level_traps: after leaving {o.groups() if o else 'not logged'} (want no timer, no hits)")


def presets_change_the_page(ctx):
    """06a_preset: clicking Nightmare, then Easy, on the New Game page changes the values the
    General page shows (Easy: XP MULTIPLIER 2.0)."""
    rows = {k: re.search(rf"autoshot preset {k} (.*)", ctx.log) for k in ("before", "Nightmare", "Easy")}
    if not all(rows.values()):
        ctx.err(f"06a_preset: not logged ({[k for k, m in rows.items() if not m]})")
        return
    b, n, e = (rows[k][1].split("|") for k in ("before", "Nightmare", "Easy"))
    if b == n or n == e or "2.0" not in e:
        ctx.err(f"06a_preset: the shown values didn't follow the presets ({b} / {n} / {e})")


def doors_open(ctx):
    """08zzp_doors: F swings a placed Wooden Door open, 90° away from the survivor (its leaf turned
    to UE yaw 90 = our +Z: atan2 90); the unpowered Garage Door says "Requires power" and stays
    shut; powered, it starts moving and is open and still 1.83 s on."""
    s = re.search(r"autoshot doors swing closed (\w+) moving (\w+) yaw ([-\d.]+) leaf (\S+)", ctx.log)
    if not s or s.groups()[:3] != ("false", "false", "90.0") or s[4] not in ("Some(90.0)",):
        ctx.err(f"08zzp_doors: swing {s.groups() if s else 'not logged'} (want open, still, yaw 90, the leaf at 90)")
    u = re.search(r"autoshot doors garage unpowered toast (\w+) still (\w+)", ctx.log)
    if not u or u.groups() != ("true", "true"):
        ctx.err(f"08zzp_doors: unpowered {u.groups() if u else 'not logged'} (want 'Requires power', shut)")
    m = re.search(r"autoshot doors garage powered moving (\w+)", ctx.log)
    o = re.search(r"autoshot doors garage open (\w+) still (\w+) t ([\d.]+)", ctx.log)
    if not m or m[1] != "true" or not o or o.groups() != ("true", "true", "1.83"):
        ctx.err(f"08zzp_doors: powered {m.groups() if m else None} {o.groups() if o else None} (want moving, then open at 1.83)")


def benches_craft(ctx):
    """08zzo_benches: a placed construction workbench opens its crafting (Lvl1ConstructionWorkbench)
    under CONSTRUCTION WORKBENCH with STANDARD / MODULAR; MODULAR switches the table (and the
    bench keeps it); a Wooden Foundation is crafted from 4 logs, planks and nails; the placed
    campfire has its flames and SC_CampFire."""
    o = re.search(r"autoshot benches open (\w+) station (\S+) title (\w+) tabs (\w+) (\w+) craftable (\w+)", ctx.log)
    if not o or o.groups() != ("true", 'Some("Lvl1ConstructionWorkbench")', "true", "true", "true", "true"):
        ctx.err(f"08zzo_benches: open {o.groups() if o else 'not logged'} (want the bench's crafting with both tabs)")
    m = re.search(r"autoshot benches modular station (\S+) kept (\S+)", ctx.log)
    if not m or m.groups() != ('Some("Lvl1ConstructionModularWorkbench")', 'Some("Lvl1ConstructionModularWorkbench")'):
        ctx.err(f"08zzo_benches: MODULAR {m.groups() if m else 'not logged'} (want the modular table, kept by the bench)")
    c = re.search(r"autoshot craft table Lvl1ConstructionModularWorkbench DA_WoodenFoundation (\d+) -> (\d+)", ctx.log)
    if not c or int(c[2]) != int(c[1]) + 1:
        ctx.err(f"08zzo_benches: crafting {c.groups() if c else 'not logged'} (want one Wooden Foundation)")
    f = re.search(r"autoshot benches campfire flames (\w+) crackle (\w+)", ctx.log)
    if not f or f.groups() != ("true", "true"):
        ctx.err(f"08zzo_benches: campfire {f.groups() if f else 'not logged'} (want flames and SC_CampFire)")


def traps_catch_and_blow(ctx):
    """08zzn_traps: three AIs put in a razor wire, a bear trap and a landmine: numbers of 25, 25
    and 200-225; the wire's and the trap's AI held (the mine's dead or not, unheld), the jaws
    shutting or shut, the mine gone; past the wire's 3 s its AI walks again (or died of it)."""
    m = re.search(r"autoshot traps ais (\d+) numbers \[(.*?)\] held \[(.*?)\] jaws (.*?) mine left (\w+)", ctx.log)
    if not m:
        ctx.err("08zzn_traps: not logged")
        return
    nums = [float(x) for x in m[2].split(", ") if x]
    held = m[3].split(", ")
    if m[1] != "3" or nums.count(25.0) < 2 or not any(200.0 <= n <= 225.0 for n in nums) or held[:2] != ["true", "true"] or not ("Shut" in m[4] or "Closing" in m[4]) or m[5] != "false":
        ctx.err(f"08zzn_traps: {m.groups()} (want 25, 25, 200-225; the first two held; the jaws shut; the mine gone)")
    w = re.search(r"autoshot traps wire let go (.*)", ctx.log)
    if not w or not (w[1] == "Some((false, false))" or w[1] == "None" or "true)" in w[1]):
        ctx.err(f"08zzn_traps: the wire {w.groups() if w else 'not logged'} (want its AI walking again after 3 s)")


def explosive_barrel_blows(ctx):
    """08zzm_barrel: a round into an explosive barrel (25-50 health) blows it: its prop is
    removed, the zombie beside it is killed or hurt by 75-125 (a dead one leaves the query: hp
    None) and a white number in that range shows."""
    a = re.search(r"autoshot barrel (\d+) at .* health ([\d.]+) key (\S+.*?) zombie hp (\S+)", ctx.log)
    if not a or not 25.0 <= float(a[2]) <= 50.0 or a[3] == "None":
        ctx.err(f"08zzm_barrel: before {a.groups() if a else 'not logged'} (want a barrel with 25-50 health and its prop)")
    e = re.search(r"autoshot barrel exploded (\w+) prop removed (\w+) zombie hp (\S+) numbers \[(.*?)\]", ctx.log)
    nums = [float(x) for x in e[4].split(", ") if x] if e else []
    if not e or e[1] != "true" or e[2] != "true" or not any(75.0 <= n <= 125.0 for n in nums):
        ctx.err(f"08zzm_barrel: after {e.groups() if e else 'not logged'} (want it exploded, gone, a 75-125 number)")


def generators_power_lights(ctx):
    """08zzl_power: a Small Generator and a Lamp placed green; Turn On empty says "Empty";
    Refuel takes the Gas Can and fills it to 200; Turn On; the lamp switched on is powered by
    the next scan and lit, the fuel going down, the hum playing; Turn Off unpowers it, dark."""
    for what in ("generator", "lamp"):
        if f"autoshot genpower {what} can build true" not in ctx.log:
            ctx.err(f"08zzl_power: the {what}'s ghost wasn't green")
    r = re.search(r"autoshot genpower refuelled (\S+) cans left (\d+) toasts (.*)", ctx.log)
    if not r or r[1] != "Some(200.0)" or r[2] != "0" or '"Empty"' not in r[3]:
        ctx.err(f"08zzl_power: refuelling {r.groups() if r else 'not logged'} (want Empty first, then 200 from the one Gas Can)")
    m = re.search(r"autoshot genpower running (\w+) fuel ([\d.]+) hum (\w+) lamp powered (\w+) switch (\w+) intensity (\d+)", ctx.log)
    if not m or m[1] != "true" or not 199.0 < float(m[2]) < 200.0 or m[3] != "true" or (m[4], m[5]) != ("true", "true") or int(m[6]) <= 0:
        ctx.err(f"08zzl_power: running {m.groups() if m else 'not logged'} (want it burning fuel, humming, the lamp powered, on and lit)")
    o = re.search(r"autoshot genpower off running (\w+) lamp powered (\w+) switch (\w+) intensity (\d+)", ctx.log)
    if not o or o.groups() != ("false", "false", "true", "0"):
        ctx.err(f"08zzl_power: off {o.groups() if o else 'not logged'} (want the lamp unpowered and dark, its switch still on)")


def beds_sleep_and_respawn(ctx):
    """08zzj_sleep / 08zzk_respawn: a Double Bed placed green; Sleep at noon is refused ("It's
    too early to sleep yet, wait until 19:30"); at 22:00 SleepingUI opens (2 buttons) and Sleep
    eases the clock to 06:30 the next day, the widget gone; Set Spawn Point stores the bed's
    RespawnLocation (1.5, 0.85, 0 off the bed) with "Spawn point set". Killed: the respawn screen
    (5 buttons) at 0.1x time and a level lost; Respawn - Spawn Point: back at the bed, full
    health, time at 1x."""
    if "autoshot sleep bed can build true" not in ctx.log:
        ctx.err("08zzj_sleep: the bed's ghost wasn't green")
    n = re.search(r"autoshot sleep at noon overlay (\w+) toasts (.*)", ctx.log)
    if not n or n[1] != "None" or "It's too early to sleep yet, wait until 19:30" not in n[2]:
        ctx.err(f"08zzj_sleep: at noon {n.groups() if n else 'not logged'} (want refused with the toast)")
    o = re.search(r"autoshot sleep at night overlay (\w+) buttons (\d+)", ctx.log)
    if not o or o.groups() != ("Sleep", "2"):
        ctx.err(f"08zzj_sleep: at 22:00 {o.groups() if o else 'not logged'} (want SleepingUI with 2 buttons)")
    w = re.search(r"autoshot sleep woke hour ([\d.]+) days (\d+) -> (\d+) overlay (\w+)", ctx.log)
    if not w or abs(float(w[1]) - 6.5) > 0.05 or int(w[3]) != int(w[2]) + 1 or w[4] != "None":
        ctx.err(f"08zzj_sleep: woke {w.groups() if w else 'not logged'} (want 06:30 the next day, the widget gone)")
    p = re.search(r"autoshot sleep spawn point \[([-\d., ]+)\] toasts (.*)", ctx.log)
    off = [float(x) for x in p[1].split(",")] if p else []
    if not p or len(off) != 3 or abs(off[0] - 1.5) > 0.02 or abs(off[1] - 0.85) > 0.02 or abs(off[2]) > 0.02 or "Spawn point set" not in p[2]:
        ctx.err(f"08zzj_sleep: spawn point {p.groups() if p else 'not logged'} (want (1.5, 0.85, 0) off the bed and the toast)")
    r = re.search(r"autoshot respawn screen (\w+) speed ([\d.]+) buttons (\d+) level (\d+) -> (\d+)", ctx.log)
    if not r or r[1] != "true" or abs(float(r[2]) - 0.1) > 1e-3 or r[3] != "5" or (int(r[4]) > 1 and int(r[5]) != int(r[4]) - 1):
        ctx.err(f"08zzk_respawn: {r.groups() if r else 'not logged'} (want the screen, 0.1x, 5 buttons, a level lost)")
    b = re.search(r"autoshot respawn back (\w+) speed ([\d.]+) health (\d+) from spawn point ([\d.NaN]+)", ctx.log)
    if not b or b[1] != "true" or float(b[2]) != 1.0 or int(b[3]) < 99 or not float(b[4]) < 0.5:
        ctx.err(f"08zzk_respawn: {b.groups() if b else 'not logged'} (want back at the spawn point, full health, 1x)")


def attachments_work(ctx):
    """08zzt_attachments: three attachments on the M4CQB; middle mouse while aiming zooms to the
    ACOG's 25 degrees (about 14.2 vertical at 16:9); the shot is suppressed and heard 10.5 m away
    (70 x 0.15); letting go of the aim drops the zoom (back to 90 across); the rifle's model
    carries all four attachments' meshes."""
    n = re.search(r"autoshot attachments on the rifle (\d+)", ctx.log)
    if not n or n[1] != "4":
        ctx.err(f"08zzt_attachments: {n[1] if n else 'no'} attachments (want the mag and 3)")
    z = re.search(r"autoshot attachments zoom Some\(\(([\d.]+), (Some\(\w+\)|None)\)\) fov ([\d.]+) aiming (\w+) aspect ([\d.]+)", ctx.log)
    # 25 degrees across is ~14.2 tall at 16:9; Hyprland sometimes tiles the window to another
    # shape (945x1025 on 2026-10-29: 27 tall), so the window's own aspect sets the height.
    want = math.degrees(2 * math.atan(math.tan(math.radians(12.5)) / float(z[5]))) if z else 0.0
    if not z or float(z[1]) != 25.0 or z[2] != "Some(Acog)" or abs(float(z[3]) - want) > 1.5 or z[4] != "true":
        ctx.err(f"08zzt_attachments: zoom {z.groups() if z else 'not logged'} (want the ACOG's 25 across, {want:.1} tall at that aspect, aiming)")
    after = ctx.log[n.end():] if n else ""
    s = re.search(r"attachments: shot suppressed (\w+) noise ([\d.]+) m", after)
    if not s or s[1] != "true" or abs(float(s[2]) - 10.5) > 0.05:
        ctx.err(f"08zzt_attachments: shot {s.groups() if s else 'not fired'} (want suppressed, 10.5 m)")
    a = re.search(r"autoshot attachments after the aim zoom (\S+) fov ([\d.]+)", ctx.log)
    if not a or a[1] != "None" or float(a[2]) < 40.0:
        ctx.err(f"08zzt_attachments: after the aim {a.groups() if a else 'not logged'} (want no zoom, the normal view)")
    m = re.search(r"autoshot attachments on the model (.*)", ctx.log)
    want = ["DA_556Mag", "DA_MilitarySuppressor", "DA_VerticalForegrip", "DA_Acog4x32Scope"]
    if not m or not all(w in m[1] for w in want):
        ctx.err(f"08zzt_attachments: the rifle's model shows {m[1] if m else 'nothing logged'} (want the mag, suppressor, grip and ACOG on their sockets)")


def scope_overlay_and_breath(ctx):
    """08zztb_scope: the ACOG scoped shows the Acog overlay and hides the survivor, the crosshair
    and the action bar, with a SniperScopeSway playing; holding Left Shift drains 0.3 /
    Marksmanship stamina every 0.01 s (30 a second; the recovery may add up to 4 a second) and
    blends the sway out; letting go plays SniperScopeSway_1 and AimingSway_1."""
    o = re.search(r"autoshot scope overlay (\S+) survivor (\S+) hud (\S+) shakes \[([^\]]*)\] aiming (\w+)", ctx.log)
    if not o or o[1] != "Some(Acog)" or "Hidden" not in o[2] or o[3] != "Hidden/Hidden" or "SniperScopeSway" not in o[4] or o[5] != "true":
        ctx.err(f"08zztb_scope: {o.groups() if o else 'not logged'} (want the Acog overlay, survivor and HUD hidden, a scope sway)")
    b = re.search(r"autoshot scope breath stamina ([\d.]+) -> ([\d.]+) rate (-?[\d.]+) want ([\d.]+) shakes \[([^\]]*)\]", ctx.log)
    if not b or not 0.75 * float(b[4]) <= float(b[3]) <= 1.1 * float(b[4]):
        ctx.err(f"08zztb_scope: holding breath {b.groups() if b else 'not logged'} (want stamina falling at about 30 / Marksmanship a second)")
    elif b[5]:
        ctx.err(f"08zztb_scope: the sway still plays after 0.5 s of held breath: {b[5]}")
    g = re.search(r"autoshot scope breath let go shakes \[([^\]]*)\]", ctx.log)
    if not g or set(g[1].split(",")) != {"CameraShake_SniperScopeSway_1", "CameraShake_AimingSway_1"}:
        ctx.err(f"08zztb_scope: after letting go {g[1] if g else 'not logged'} (want SniperScopeSway_1 and AimingSway_1)")


def placed_turrets_fight(ctx):
    """08zzv_turrets: a placed small turret shoots roamer A in front of it (white damage numbers);
    roamer Z behind it goes for the turret, not the survivor, and its swings take the turret's 75
    health (saved, the bar shows "N/75"); the unpowered big turret's prompt says "Power: Off" and
    it isn't a zombie target; powered, it hits roamer C; 100 damage destroys the small one (gone
    from the world and the save) and F picks the big one up."""
    placed = re.search(r"autoshot turrets placed small Some\((\d+)\) at .* big Some\((\d+)\)", ctx.log)
    if not placed:
        ctx.err("08zzv_turrets: the turrets weren't placed")
        return
    a = re.search(r'autoshot turrets small Some\("(\d+) health ([\d.]+) ammo \d+"\) roamer A (?:hp (-?[\d.]+)/([\d.]+) dead (\w+)|gone)', ctx.log)
    if not a or a[2] != "75" or (a[3] is not None and a[5] != "true" and float(a[3]) >= float(a[4])):
        ctx.err(f"08zzv_turrets: roamer A {a.groups() if a else 'not logged'} (want the small turret at 75 to have shot it)")
    first = ctx.log.find("autoshot turrets placed")
    between = ctx.log[first:ctx.log.find("autoshot turrets survivor before", first)]
    if not re.search(r"damage number \d+", between):
        ctx.err("08zzv_turrets: no damage number while the small turret fired")
    z = re.search(r'autoshot turrets small (\d+) after the roamer health ([\d.]+) saved Some\((Some\(([\d.]+)\)|None)\) bar (Some\("([^"]*)"\)|None) roamer Z (?:hp -?[\d.]+/[\d.]+ dead \w+ target (Some\(\d+\)|None)|gone)', ctx.log)
    if not z:
        ctx.err("08zzv_turrets: the roamer behind the turret not logged")
    else:
        health = float(z[2])
        if health >= 75:
            ctx.err(f"08zzv_turrets: the small turret still has {z[2]} after roamer Z's swings")
        # The save follows the health in placed_sync, which runs before the frame's hits land,
        # so it can be one zombie hit (up to ~30) behind when the tour reads both.
        if z[4] is None or not (health <= float(z[4]) <= health + 30.0) or float(z[4]) >= 75:
            ctx.err(f"08zzv_turrets: saved health {z[3]} (want {z[2]}, or one hit behind it)")
        if z[6] != f"{int(health)}/75":
            ctx.err(f"08zzv_turrets: the bar shows {z[5]} (want {int(health)}/75)")
        if z[7] != f"Some({z[1]})":
            ctx.err(f"08zzv_turrets: roamer Z's target {z[7]} (want the turret, Some({z[1]}))")
    u = re.search(r'autoshot turrets big unpowered Some\("\d+ powered (\w+) attackable (\w+) target None"\) prompt (Some\("[^"]*"\)|None)', ctx.log)
    if not u or (u[1], u[2]) != ("false", "false") or "Power: Off" not in u[3]:
        ctx.err(f"08zzv_turrets: unpowered big turret {u.groups() if u else 'not logged'} (want unpowered, not a target, the prompt's Power: Off)")
    c = re.search(r'autoshot turrets big powered Some\("\d+ powered (\w+) attackable (\w+) ammo \d+"\) roamer C (?:hp (-?[\d.]+)/([\d.]+) dead (\w+)|gone)', ctx.log)
    if not c or (c[1], c[2]) != ("true", "true") or (c[3] is not None and c[5] != "true" and float(c[3]) >= float(c[4])):
        ctx.err(f"08zzv_turrets: powered big turret {c.groups() if c else 'not logged'} (want powered, a target, roamer C hit)")
    if "autoshot turrets small destroyed: placed false saved false" not in ctx.log or "buildables: DA_SmallTurret destroyed" not in ctx.log:
        ctx.err("08zzv_turrets: 100 damage didn't destroy the small turret (want it gone from the world and the save)")
    if "autoshot turrets big picked up: placed false held or at the feet 1" not in ctx.log:
        ctx.err("08zzv_turrets: F didn't pick the big turret up")


def inspector_frames_the_item(ctx):
    """08fc_inspect: the context menu's Inspect on the first pocket stack opens the inspector at
    the item's FOV x 1.5 and frames the model once it's loaded."""
    m = re.search(r"inspect: (DA_\w+) \(fov ([\d.]+), zoom", ctx.log)
    if not m or float(m[2]) <= 0:
        ctx.err("08fc_inspect: the inspector didn't open")
    if not re.search(r"inspect: window \d+ framed, radius [\d.]+ m at [\d.]+ m", ctx.log):
        ctx.err("08fc_inspect: the inspected model never framed (its mesh didn't load?)")


def bag_windows_open(ctx):
    """08fcc_bag_window: a double click on a Lunch Box in the inventory opens its window, one on
    the Wallet inside that window opens a second; the Wallet's beans dropped from its window into
    the Lunch Box's are in the Lunch Box's contents; both windows were placed inside the screen. 08fd_repair takes
    the Lunch Box back out."""
    if "08fcc_bag_window" not in ctx.steps:
        return
    opened = re.findall(r"inventory: opened (DA_\w+)'s (SC_\w+) in window (\d+)", ctx.log)
    if [o[:2] for o in opened[-2:]] != [("DA_LunchBox", "SC_LunchBox"), ("DA_Wallet", "SC_Wallet")]:
        ctx.err(f"08fcc_bag_window: opened {opened} (want the Lunch Box's, then the Wallet's)")
    m = re.search(r"autoshot bag window: windows \[([^\]]*)\], the lunch box holds \[([^\]]*)\] \(moved (?:Some\(\"(\w+)\"\)|None)\)", ctx.log)
    if not m:
        ctx.err("08fcc_bag_window: no bag window log")
        return
    wins = re.findall(r'"(\w+)"', m[1])
    holds = re.findall(r'"(\w+)"', m[2])
    if wins != ["DA_LunchBox", "DA_Wallet"]:
        ctx.err(f"08fcc_bag_window: windows {wins}")
    if not holds or holds[0] != "DA_Wallet" or m[3] != "DA_CannedBakedBeans" or m[3] not in holds[1:]:
        ctx.err(f"08fcc_bag_window: the lunch box holds {holds} (want the Wallet, then {m[3]})")
    placed = re.findall(r"inventory: (DA_LunchBox|DA_Wallet) window \d+ at \[\s*(-?[\d.]+),\s*(-?[\d.]+)\] \(\[\s*([\d.]+),\s*([\d.]+)\]\)", ctx.log)
    if sorted({p[0] for p in placed}) != ["DA_LunchBox", "DA_Wallet"]:
        ctx.err(f"08fcc_bag_window: windows placed {placed} (want both laid out)")
    elif any(float(p[1]) < 0 or float(p[2]) < 0 or float(p[3]) < 100 or float(p[4]) < 100 for p in placed):
        ctx.err(f"08fcc_bag_window: placed {placed} (want inside the screen and at least 100 px)")
    if "autoshot bag window: the lunch box taken back out" not in ctx.log:
        ctx.err("08fd_repair: the Lunch Box wasn't taken back out (later steps need the pocket room)")


def inspector_opens_more_windows(ctx):
    """08fcb_inspect_more: a double click on a pocket item opens a second inspector window (on
    the one already open: nothing new); on the M4 it's its SC_ window (not ported), and its
    Inspect opens a third with its 4 attachments on. The windows sit within 50 px of the middle
    each way, the last opened in front."""
    clicks = re.findall(r"autoshot double click (DA_\w+): (\w+)(?:\(\"(\w+)\"\))?", ctx.log)
    if len(clicks) != 3 or [c[1] for c in clicks] != ["Inspect", "Inspect", "Window"] or clicks[2][0] != "DA_M4CQB" or not clicks[2][2].startswith("SC_"):
        ctx.err(f"08fcb_inspect_more: double clicks {clicks}")
        return
    if f"inspect: {clicks[1][0]} already has a window" not in ctx.log:
        ctx.err("08fcb_inspect_more: the second double click on the inspected item opened another window")
    wins = re.findall(r"autoshot inspect window (\d+) (DA_\w+) offset \[\s*(-?[\d.]+),\s*(-?[\d.]+)\] active (\w+) attached (\S+)", ctx.log)
    n = re.search(r"autoshot inspect windows (\d+)", ctx.log)
    if not n or n[1] != "3" or len(wins) != 3:
        ctx.err(f"08fcb_inspect_more: {n[1] if n else 'no'} windows ({wins})")
        return
    if [w[1] for w in wins] != [clicks[1][0], clicks[0][0], "DA_M4CQB"]:
        ctx.err(f"08fcb_inspect_more: windows on {[w[1] for w in wins]}")
    if any(abs(float(w[2])) > 50 or abs(float(w[3])) > 50 for w in wins) or [w[4] for w in wins] != ["false", "false", "true"]:
        ctx.err(f"08fcb_inspect_more: offsets / front {wins}")
    if wins[2][5] != "Some(4)":
        ctx.err(f"08fcb_inspect_more: the M4 has {wins[2][5]} attachments on (want Some(4))")


def security_camera_sweeps_and_breaks(ctx):
    """08zzcb_camera: the 5 security cameras found their heads; one sweeps (its yaw moves over
    3 s, tipped 20 degrees down) and two 15-point rounds destroy it (25 health), base and all."""
    if "security cameras: 5" not in ctx.log:
        ctx.err("08zzcb_camera: not 5 security cameras with their heads")
    m = re.search(r"autoshot camera yaw from (-?[\d.]+) to (-?[\d.]+) pitch (-?[\d.]+)", ctx.log)
    if not m or float(m[2]) - float(m[1]) < 10 or abs(float(m[3]) + 20) > 5:
        ctx.err(f"08zzcb_camera: no sweep ({m.groups() if m else None})")
    if "autoshot camera dead true health 0" not in ctx.log or not re.search(r"security camera \d+ destroyed \(2 props gone\)", ctx.log):
        ctx.err("08zzcb_camera: two 15s didn't destroy it with its base and light")


def lab_corpses_lie_posed(ctx):
    """The lab's 28 BP_LaboratoryCorpse (9 guards) are placed, and the ones the tour passes in
    the lab (08v_lab_power) take their pose: the clip's 70 bones sampled at their AnimFloat."""
    if "lab corpses: 28 (9 guards)" not in ctx.log:
        ctx.err("lab corpses: not 28 placed (9 guards)")
    bones = [int(b) for b in re.findall(r"lab corpse posed at .*?: (\d+) bones", ctx.log)]
    if not bones or max(bones) < 60:
        ctx.err(f"lab corpses: none posed in the lab ({bones[:3]})")


def gun_locker_needs_cutters(ctx):
    """08vd_locker: F on a gun locker without Bolt Cutters says "Bolt Cutters not found"; with
    them a held search opens it ("Locker"), the cutters lose 10 durability and the first loot
    gives "Container Looted, XP Given"."""
    if "autoshot locker: none" in ctx.log or "autoshot locker at" not in ctx.log:
        ctx.err("08vd_locker: no gun locker")
        return
    if "Bolt Cutters not found" not in ctx.log:
        ctx.err("08vd_locker: no \"Bolt Cutters not found\" without them")
    m = re.search(r'autoshot locker "([^"]*)" opened, (\d+) items, cutters (\S+)', ctx.log)
    if not m or m[1] != "Locker" or m[3] != "Some(90.0)":
        ctx.err(f"08vd_locker: {m.groups() if m else 'never opened'} (want Locker, the cutters at 90)")
    if not re.search(r"Container Looted, XP Given: \d+", ctx.log):
        ctx.err("08vd_locker: no XP toast")
    # BP_LootContainerWidget during the Quick Search hold: the name, its loot type, the
    # "[LShift] Quick Search" line, the square filling, the crosshair hidden.
    w = re.search(r'autoshot locker widget shown (\w+) name "([^"]*)" type "([^"]*)" looted (\w+) quick (\w+) progress ([\d.]+) crosshair (-?[\d.]+)', ctx.log)
    if not w or w[1] != "true" or w[2] != "Locker" or not w[3] or w[4] != "false" or w[5] != "true" or not 0.0 < float(w[6]) < 1.0 or float(w[7]) != 0.0:
        ctx.err(f"08vd_locker: search widget {w.groups() if w else 'not logged'} (want shown, Locker, a loot type, not looted, Quick Search shown, part filled, crosshair hidden)")


def hidden_cache_opens(ctx):
    """08vc_cache: a Hidden Cache spawned at one of its spawner's points, at the ground (its
    points sit the barrel's foot within ~1 m of the landscape), and F held opens it."""
    m = re.search(r"autoshot cache at \[([-\d.]+), ([-\d.]+), ([-\d.]+)\] ground ([-\d.]+|NaN) tilt", ctx.log)
    if not m:
        ctx.err("08vc_cache: " + ("no cache spawned" if "autoshot cache: none" in ctx.log else "not logged"))
        return
    if m[4] == "NaN" or abs(float(m[2]) - float(m[4])) > 1.5:
        ctx.err(f"08vc_cache: the barrel at y {m[2]} vs the ground at {m[4]}")
    o = re.search(r'autoshot cache "([^"]*)" opened, (\d+) items', ctx.log)
    if not o or o[1] != "Hidden Cache":
        ctx.err(f"08vc_cache: {o.groups() if o else 'never opened'}")


def loot_refills(ctx):
    """08vc_cache / 08vca_refill: the unsearched cache shows its search icon and, with its prompt
    up, the outline; with LOOT REGENERATION on (DISABLED ON LOAD? ticked) its search arms the
    refill timer (30 min); the timer
    brought forward with its loot screen up keeps the items but unsearches it, then with it shut
    empties it, and the icon is back."""
    m = re.search(r"autoshot cache icon (\w+) outline (\S+) key (\d+)", ctx.log)
    if not m or m[1] != "true" or m[2] != f"Some({m[3]})":
        ctx.err(f"08vc_cache: icon / outline {m.groups() if m else 'not logged'}")
    elif f"loot refill: {m[3]} armed for 1800 s" not in ctx.log:
        ctx.err("08vc_cache: the search didn't arm the refill timer")
    want = ["autoshot refill armed true looted true",
            "loot refill: {} Kept".format(m[3] if m else "?"),
            "autoshot refill open: looted false items ",
            "loot refill: {} Cleared".format(m[3] if m else "?"),
            "autoshot refill closed: looted false grid false icon true"]
    for w in want:
        if w not in ctx.log:
            got = re.findall(r"autoshot refill .*", ctx.log)
            ctx.err(f"08vca_refill: missing '{w}' ({got})")
            break
    o = re.search(r"autoshot refill open: looted false items (\d+) rearmed (\w+)", ctx.log)
    if o and (int(o[1]) == 0 or o[2] != "true"):
        ctx.err(f"08vca_refill: kept {o[1]} items, rearmed {o[2]}")


def containers_mark_as_storage(ctx):
    """08vcb_storage: the cache's prompt offers Search Container and Mark As Storage Container;
    the wheel selects the second, and F on the unsearched cache says "Container Not Searched".
    Searched, then marked: "[Storage]" and "Mark As Loot Container"; its refill timer fires to
    no effect (still looted, items kept) and the save holds it as marked and searched. Unmarked,
    it reads "[Looted]" and "Mark As Storage Container" again."""
    opts = '["Search Container", "Mark As Storage Container"]'
    o = re.search(r'autoshot storage option2 shown (\w+) key Some\((\d+)\) options (\[.*?\]) selected (\d+) additional "([^"]*)"', ctx.log)
    if not o or o[1] != "true" or o[3] != opts or o[4] != "1" or o[5] != "":
        ctx.err(f"08vcb_storage: option 2 {o.groups() if o else 'not logged'} (want shown, {opts}, selected 1, no tag)")
        return
    key = o[2]
    after = ctx.log[o.end():]
    if "toast: Container Not Searched" not in after.split("autoshot storage searched")[0]:
        ctx.err("08vcb_storage: no \"Container Not Searched\" on the unsearched cache")
    s = re.search(r"autoshot storage searched: (\d+) items", after)
    if not s:
        ctx.err("08vcb_storage: the Quick Search didn't open the cache")
        return
    if f"loot: {key} marked as storage container" not in after:
        ctx.err("08vcb_storage: F on Option2 didn't mark the cache")
    m = re.search(r'autoshot storage marked (\w+) shown \w+ key \S+ options (\[.*?\]) selected (\d+) additional "([^"]*)"', after)
    if not m or m[1] != "true" or m[2] != '["Search Container", "Mark As Loot Container"]' or m[4] != "[Storage]":
        ctx.err(f"08vcb_storage: marked {m.groups() if m else 'not logged'} (want true, Mark As Loot Container, [Storage])")
    if f"loot refill: {key} marked, no refill" not in after:
        ctx.err("08vcb_storage: the marked cache's refill timer didn't end without a refill")
    r = re.search(r"autoshot storage after refill: looted (\w+) items (\d+) saved (.*)", after)
    if not r or r[1] != "true" or r[2] != s[1] or r[3] != "Some((true, true))":
        ctx.err(f"08vcb_storage: after the refill {r.groups() if r else 'not logged'} (want looted, {s[1]} items, saved marked + searched)")
    u = re.search(r'autoshot storage unmarked: marked (\w+) shown \w+ key \S+ options (\[.*?\]) selected \d+ additional "([^"]*)"', after)
    if not u or u[1] != "false" or u[2] != opts or u[3] != "[Looted]":
        ctx.err(f"08vcb_storage: unmarked {u.groups() if u else 'not logged'} (want false, {opts}, [Looted])")


def repairs_fix_the_rifle(ctx):
    """08fd_repair: the M4 at 40 repaired from its context menu with a Weapon Cleaning Kit
    (60 fixed for 30 of the kit: ReduceDurabilityAmount 2), at 50 by dropping the kit onto it
    (50 for 25), then with no kit: unchanged and the game's notice."""
    want = ('autoshot repair context rifle 40 -> 100 kit Some(100.0) -> Some(70.0); '
            'drop rifle 50 -> 100 kit Some(70.0) -> Some(45.0); '
            'none rifle 40 notice Some("No Weapon Cleaning Kits found in this inventory")')
    if want not in ctx.log:
        m = re.search(r"autoshot repair .*", ctx.log)
        ctx.err(f"08fd_repair: {m[0] if m else 'not logged'}")


def drop_selector_shows(ctx):
    """08fb_drop: DropItemAmountSelector over the first pocket stack ("Drop {name}", the whole
    stack picked)."""
    if not re.search(r"autoshot drop selector over .+ x\d+", ctx.log):
        ctx.err("08fb_drop: the drop amount selector wasn't shown")


def permadeath_ends_the_game(ctx):
    """08zzkb_permadeath: with PERMADEATH a death shows ScreenPermadeath (no respawn buttons, time
    at 0.1x) and deletes the save ("TourPermadeath Deleted"); turned off again, the survivor
    respawns for the steps after."""
    if "autoshot permadeath save written true exists true" not in ctx.log:
        ctx.err("08zzkb_permadeath: the save wasn't written first")
        return
    m = re.search(r"autoshot permadeath screen (\w+) widget (\w+) respawn offered (\w+) speed ([\d.]+) save exists (\w+) toast (\w+) slot (\S+)", ctx.log)
    if not m or m.groups() != ("true", "true", "false", "0.10", "false", "true", "None"):
        ctx.err(f"08zzkb_permadeath: {m.groups() if m else 'not logged'} (want the screen, its widget, no respawn, 0.1x, the save gone with its toast)")
    a = re.search(r"autoshot permadeath after: overlay (\w+) health ([\d.]+) speed ([\d.]+)", ctx.log)
    if not a or a[1] != "None" or float(a[2]) <= 0 or a[3] != "1.00":
        ctx.err(f"08zzkb_permadeath: after {a.groups() if a else 'not logged'} (want respawned, the screen gone, time back to 1x)")


def power_attack_hits(ctx):
    """08zzz_power: LMB held past 0.5 s turns the crosshair (PowerAttackAnim); released after
    0.9 s it's a power swing (a PowerMeleeAttackMontages pick at 0.75) that hits the roamer and
    costs StaminaDrain x 6-8; a quick click is a normal swing (the Crowbar's Longs_Attack)."""
    if "autoshot power held from" not in ctx.log:
        ctx.err("08zzz_power: the step didn't run")
        return
    c = re.search(r"autoshot power cue Some\(\((-?[\d.]+), ([\d.]+)\)\) hold Some", ctx.log)
    if not c or (abs(float(c[1])) < 1.0 and abs(float(c[2]) - 1.0) < 0.01):
        ctx.err(f"08zzz_power: crosshair {c.groups() if c else 'not logged'} (want PowerAttackAnim turning / growing it mid-hold)")
    p = re.search(r'autoshot power swing Some\(\(true, "(Attack_0\d_Seq_Montage_RM)", 0\.75\)\) roamer Some\(\(([-\d.]+), ([\d.]+)\)\) stamina used ([\d.]+)', ctx.log)
    if not p or float(p[2]) >= float(p[3]) or float(p[4]) < 15.0:
        ctx.err(f"08zzz_power: power swing {p.groups() if p else 'not logged'} (want a power montage at 0.75 that hurt the roamer and cost 6-8x the drain)")
    if not re.search(r"melee: power hit for [\d.]+", ctx.log):
        ctx.err("08zzz_power: the power swing hit nothing")
    n = re.search(r'autoshot power normal swing Some\(\(false, "(Longs_Attack_\w+_Montage)", 1\.0\)\)', ctx.log)
    if not n:
        ctx.err("08zzz_power: the click wasn't a normal Longs_Attack swing")


def utility_benches_work(ctx):
    """08zzy_utility: the trash bin opens with its "Destroy Items" and empties; the charger
    refuses shells, says "Generator needed to charge battery" for a battery put in without power
    ("[No Power]" in its stat), then charges it 50 -> 52 in its first 5 s; the disassembly
    workbench turns 5 shells into 5 casings and 5 gunpowder and the M4CQB into parts, its
    "Disassemble Items" and "DISASSEMBLED" grid on screen."""
    if "autoshot utility placed" not in ctx.log:
        ctx.err("08zzy_utility: the step didn't run")
        return
    if 'button: Some("Destroy Items"), second: None }) holds Some(3)' not in ctx.log or "autoshot utility bin after destroy Some(0)" not in ctx.log:
        ctx.err("08zzy_utility: the trash bin didn't open with Destroy Items and empty")
    if "shells refused Some(true)" not in ctx.log:
        ctx.err("08zzy_utility: the battery charger took shells (OnlySupportTheseItems is DA_Battery)")
    if 'charger unpowered toast true stat Some(Some("Battery: 1 Charging [No Power]"))' not in ctx.log:
        ctx.err("08zzy_utility: no \"Generator needed to charge battery\" / [No Power] stat without power")
    u = re.search(r'autoshot utility charger battery (.*?) stat (.*?); workbench input (.*?) output Some\(\((\d+), (\d+), \[(.*?)\]\)\) widget (\w+)', ctx.log)
    if not u:
        ctx.err("08zzy_utility: the end of the step not logged")
        return
    if u[1] != "Some(Some(52.0))" or u[2] != 'Some(Some("Battery: 1 Charging "))':
        ctx.err(f"08zzy_utility: charger battery {u[1]} stat {u[2]} (want 52 after one 5 s tick, powered)")
    if u[3] != "Some(0)" or (u[4], u[5]) != ("5", "5") or "DA_HighQualityMetal" not in u[6] or u[7] != "true":
        ctx.err(f"08zzy_utility: workbench {u.groups()[2:]} (want the input emptied, 5 casings, 5 gunpowder, the rifle's parts, the widget up)")


def radios_play(ctx):
    """08zzx_radio: F switches a BP_Radio's broadcast to 0.5 and back to 0; the CoyoteCall text
    radio shows its first line under the muffled voice, F again closes it (static); an unpowered
    jukebox says "Requires power"; a placed radio's RadioUI turns on a song from the Radio folder,
    NEXT, rate 1.5, pause / play: on, playing, unpaused at 1.5, the voice playing, the widget up."""
    if "autoshot radio broadcast on" not in ctx.log:
        ctx.err("08zzx_radio: the step didn't run")
        return
    if 'broadcast on: volume Some(0.5) loop Some("Radio_Cue active true")' not in ctx.log or 'broadcast off: volume Some(0.0) loop Some("Radio_Cue active false")' not in ctx.log:
        ctx.err("08zzx_radio: the broadcast radio didn't switch on (0.5) and off (0)")
    t = re.search(r'autoshot radio text Some\(\("(\w+)", (\d+), "([^"]*)"\)\) widget (\w+) loop Some\("(\w+) active', ctx.log)
    if not t or t[1] != "CoyoteCall" or not t[3].startswith("[MILITARY]") or t[4] != "true" or t[5] != "RadioMuffled_Cue":
        ctx.err(f"08zzx_radio: text radio {t.groups() if t else 'not logged'} (want CoyoteCall's first line shown over RadioMuffled_Cue)")
    if 'autoshot radio text closed true loop Some("RadioStatic_Cue active true")' not in ctx.log:
        ctx.err("08zzx_radio: F again didn't close the text and go back to static")
    if "autoshot radio jukebox unpowered: overlay None toast true" not in ctx.log:
        ctx.err("08zzx_radio: the unpowered jukebox didn't refuse with \"Requires power\"")
    u = re.search(r'autoshot radio ui: Some\("on (\w+) paused (\w+) current (.*?) previous (.*?) rate ([\d.]+) playing (Some\(.*?\)|None)"\) voices \[(.*)\] overlay (\S+) widget (\w+)', ctx.log)
    if not u:
        ctx.err("08zzx_radio: the radio's state not logged")
    elif (u[1], u[2], u[5], u[9]) != ("true", "false", "1.5", "true") or not u[3].endswith(".mp3") or not u[4].endswith(".mp3") or "sink true paused false" not in u[7] or not u[8].startswith("Radio(Placed"):
        ctx.err(f"08zzx_radio: radio {u.groups()} (want on, playing a song after another at 1.5, its voice playing, RadioUI up)")


def launchers_fire_projectiles(ctx):
    """08zzw_launchers: an M32 grenade flies to roamer A and blows up there (7.5 m, A hurt or
    dead), a rocket does the same to roamer B (12.5 m), and a flare fired into the ground stops
    and stays; each launcher used one round (M32 5 of 6 left)."""
    if "autoshot launchers armed at" not in ctx.log:
        ctx.err("08zzw_launchers: the step didn't arm the survivor")
        return
    step = ctx.log[ctx.log.find("autoshot launchers armed at"):]
    for cls, radius, roamer, after in (("BP_ProjectileGrenade_C", "7.5", "A", "after the grenade A"), ("BP_ProjectileRocket_C", "12.5", "B", "after the rocket B")):
        b = re.search(rf"projectile: {cls} exploded at .* after ([\d.]+) s \((\w+)\): (\d+) zombies in {re.escape(radius)} m", step)
        if not b or int(b[3]) < 1:
            ctx.err(f"08zzw_launchers: {cls} {b.groups() if b else 'never exploded'} (want a blast that catches roamer {roamer})")
        h = re.search(rf"autoshot launchers {after} (?:hp (-?[\d.]+)/([\d.]+) dead (\w+)|gone)", step)
        if not h or (h[1] is not None and h[3] != "true" and float(h[1]) >= float(h[2])):
            ctx.err(f"08zzw_launchers: roamer {roamer} {h.groups() if h else 'not logged'} (want it hurt by the {cls})")
    if not re.search(r"projectile: BP_FlareRoundProjectile_C stopped at", step):
        ctx.err("08zzw_launchers: the flare never stopped")
    p = re.search(r"autoshot launchers projectiles \[(.*)\] rounds left \[(\d+), (\d+), (\d+)\]", step)
    if not p or "BP_FlareRoundProjectile_C stopped true" not in p[1] or (p[2], p[3], p[4]) != ("5", "0", "0"):
        ctx.err(f"08zzw_launchers: after the shots {p.groups() if p else 'not logged'} (want the flare lying there, rounds 5 / 0 / 0)")


def infestations_fall(ctx):
    """08zzu_infestation: a primary (2500) refuses a round while its secondary stands; 400 into
    the secondary (1000) leaves 600, under 1000 / 1.5, so its waves start (horde AIs spawn); 1000
    more destroys it: XP, InfestationsDestroyed 1, the destroyed mesh, its "Infestation Pile", the
    primary damageable and bare; 3000 destroys the primary (2 destroyed); F opens its pile."""
    if "autoshot infestation: no primary with one secondary standing" in ctx.log:
        ctx.err("08zzu_infestation: no primary with one secondary standing")
        return
    b = re.search(r"autoshot infestation before primary health ([\d.]+) can damage (\w+) secondary health ([\d.]+) destroyed (\d+) xp (\d+)", ctx.log)
    if not b or (b[1], b[2], b[3]) != ("2500", "false", "1000"):
        ctx.err(f"08zzu_infestation: before {b.groups() if b else 'not logged'} (want 2500, armored, 1000)")
        return
    if not re.search(r"infestations: BP_PrimaryInfestation_C_\d+ refused 100", ctx.log):
        ctx.err("08zzu_infestation: the primary's refusal not logged")
    h = re.search(r"autoshot infestation hit primary health ([\d.]+) secondary health ([\d.]+) waves (\w+)", ctx.log)
    if not h or h.groups() != ("2500", "600", "true"):
        ctx.err(f"08zzu_infestation: after the hits {h.groups() if h else 'not logged'} (want 2500 untouched, 600, waves started)")
    d = re.search(r'autoshot infestation secondary destroyed (\w+) swapped (\w+) pile Some\(\("([^"]*)", \d+\)\) primary can damage (\w+) bare (\w+) destroyed (\d+) xp (\d+)', ctx.log)
    if not d or d.groups()[:6] != ("true", "true", "Infestation Pile", "true", "true", str(int(b[4]) + 1)) or int(d[7]) <= int(b[5]):
        ctx.err(f"08zzu_infestation: the secondary's end {d.groups() if d else 'not logged'} (want destroyed, swapped, its pile, the primary open and bare, one more destroyed, XP)")
    p = re.search(r"autoshot infestation primary destroyed (\w+) swapped (\w+) destroyed (\d+) xp (\d+) horde AIs (\d+)", ctx.log)
    if not p or (p[1], p[2], p[3]) != ("true", "true", str(int(b[4]) + 2)) or int(p[5]) < 1:
        ctx.err(f"08zzu_infestation: the primary's end {p.groups() if p else 'not logged'} (want destroyed, swapped, two destroyed, a horde AI from the waves)")
    o = re.search(r'autoshot infestation pile opened (\w+) Some\(\("([^"]*)", (\d+)\)\)', ctx.log)
    if not o or o[1] != "true" or o[2] != "Infestation Pile" or int(o[3]) == 0:
        ctx.err(f"08zzu_infestation: the pile {o.groups() if o else 'not logged'} (want F to open a filled Infestation Pile)")


def tactical_attachments_toggle(ctx):
    """08zztc_tactical: K lights the Tactical Light's beam (65000, a battery in it) and K again
    puts it out; the Range Finder with no battery says "No Battery" and stays dark; with one
    BP_Ingame's RangeFinderDistance shows "{N}m"."""
    on = re.search(r"autoshot tactical light on Some\((\w+)\) lit Some\(\[([^\]]*)\]\) beams (\d+) intensity ([\d.]+)", ctx.log)
    if not on or on[1] != "true" or "DA_TacticalLight" not in on[2] or on[3] == "0" or float(on[4]) <= 0:
        ctx.err(f"08zztc_tactical: light {on.groups() if on else 'not logged'} (want on, lit, a beam shining)")
    off = re.search(r"autoshot tactical light off Some\((\w+)\) intensity ([\d.]+)", ctx.log)
    if not off or off[1] != "false" or float(off[2]) != 0:
        ctx.err(f"08zztc_tactical: off {off.groups() if off else 'not logged'} (want off, dark)")
    nb = re.search(r"autoshot tactical finder without a battery on Some\((\w+)\) lit Some\(\[\]\)", ctx.log)
    if not nb or nb[1] != "true" or not re.search(r'tactical: Primary on lit \[\] refused \["No Battery"\]', ctx.log):
        ctx.err(f"08zztc_tactical: the battery-less range finder {nb.groups() if nb else 'not logged'} (want on, nothing lit, \"No Battery\")")
    r = re.search(r'autoshot tactical finder reads "(\S*)" (\w+) range Some\(([\d.]+)\)', ctx.log)
    if not r or not re.fullmatch(r"[\d,]+m", r[1]) or r[2] != "Inherited":
        ctx.err(f"08zztc_tactical: range finder {r.groups() if r else 'not logged'} (want a shown reading like 123m)")


def radiation_zones_and_the_shower(ctx):
    """08zzr_radiation / 08zzs_shower: in Creek Prison (Level 1) bare-faced the meter climbs
    (0.75 per 0.2 s tick), the "Now Entering" banner and the zone's notification show and the
    Geiger plays (one voice: a 2D loop used to outlive its owner and stack a copy per restart);
    with a filtered respirator (96 %) it barely moves, the HUD box is up and the filter wears.
    Outside, the Geiger is gone (StopRadiation) and a powered shower cleans the meter; again, the
    cooldown message."""
    if "autoshot radiation: no Creek Prison zone" in ctx.log:
        ctx.err("08zzr_radiation: no Creek Prison zone")
        return
    b = re.search(r"autoshot radiation bare meter ([\d.]+) inside (\d+) banner (.*) geiger (\w+) voices (\d+) toasts (.*)", ctx.log)
    if not b or float(b[1]) < 1.0 or b[2] == "0" or "true Creek Prison" not in b[3] or b[4] != "true" or b[5] != "1" or "Increased Radiation [Level 1]" not in b[6]:
        ctx.err(f"08zzr_radiation: bare-faced {b.groups() if b else 'not logged'} (want the meter up, inside, the banner, the Geiger with one voice, the notification)")
    m = re.search(r"autoshot radiation masked meter ([\d.]+) filter ui (\w+) filter Some\(([\d.]+)\)", ctx.log)
    if not m or float(m[1]) > 0.3 or m[2] != "true" or not float(m[3]) < 100.0:
        ctx.err(f"08zzr_radiation: masked {m.groups() if m else 'not logged'} (want the meter under 0.3, the HUD box, the filter worn)")
    z = re.search(r"autoshot shower zone timer (\w+) inside (\d+) geiger voices (\d+)", ctx.log)
    if not z or z.groups() != ("false", "0", "0"):
        ctx.err(f"08zzs_shower: {z.groups() if z else 'not logged'} (want out of every zone, the timer stopped, the Geiger silent)")
    a = re.search(r"autoshot shower meter after ([\d.]+)", ctx.log)
    if not a or float(a[1]) != 0.0:
        ctx.err(f"08zzs_shower: meter {a[1] if a else 'not logged'} after the shower (want 0)")
    g = re.search(r"autoshot shower again toasts (.*)", ctx.log)
    if not g or "Decontamination shower cooldown in effect..." not in g[1]:
        ctx.err(f"08zzs_shower: second press {g[1] if g else 'not logged'} (want the cooldown message)")
    # Light / Light2: green with power, orange (OnRep_CooldownRunning?) once it runs; FX_Water and
    # FX_Water2 on for its 3 s.
    lb = re.search(r"autoshot shower lights before (\S+)", ctx.log)
    la = re.search(r"autoshot shower lights after (\S+) water (\d+)", ctx.log)
    if not lb or lb[1] != "Some(Green)" or not la or la.groups() != ("Some(Orange)", "2"):
        ctx.err(f"08zzs_shower: lights {lb[1] if lb else 'not logged'} -> {la.groups() if la else 'not logged'} (want green, then orange with 2 water emitters)")


def death_leaves_a_crate(ctx):
    """08zzka_death_loot: the death left a Container_DeadPlayerLoot holding the 2 beans the
    survivor carried, settled on the floor with its "Death" marker; F opens it and the marker goes.
    With KEEP INVENTORY ON DEATH? there is no crate (the game keeps the inventory)."""
    if re.search(r"autoshot death loot: no crate \(keep inventory true\)", ctx.log):
        return
    c = re.search(r"autoshot death loot crate \d+ items (\d+) beans (\d+) settled (\w+) marker (\w+)", ctx.log)
    if not c or int(c[2]) < 2 or c[3] != "true" or c[4] != "true":
        ctx.err(f"08zzka_death_loot: crate {c.groups() if c else 'not found'} (want the 2 beans, settled, marked)")
    o = re.search(r"autoshot death loot opened (\w+) beans (\d+) marker (\w+)", ctx.log)
    if not o or o[1] != "true" or int(o[2]) < 2 or o[3] != "false":
        ctx.err(f"08zzka_death_loot: opened {o.groups() if o else 'not logged'} (want its loot screen with the beans, the marker gone)")


def storage_stores_renames_and_picks_up(ctx):
    """08zzh_storage / 08zzi_storage_pickup: a Storage Crate placed green; F opens its 6 x 6 grid
    titled "Storage Crate"; renamed "Base Stash" by typing, F opens it under that name with the
    Gas Can, both saved; Pickup asks first, YES drops the Gas Can at the survivor's feet and
    puts the crate back in the inventory."""
    if "autoshot storage can build true" not in ctx.log:
        ctx.err("08zzh_storage: the crate's ghost wasn't green")
    o = re.search(r'autoshot storage opened (\w+) grid (.*)', ctx.log)
    if not o or o[1] != "true" or o[2] != 'Some(("Storage Crate", 6, 6))':
        ctx.err(f"08zzh_storage: opening {o.groups() if o else 'not logged'} (want the 6 x 6 Storage Crate grid open)")
    r = re.search(r'autoshot storage renamed (.*?) open (\w+) title (.*?) items (\d+) saved (.*)', ctx.log)
    if not r or r.groups() != ('Some("Base Stash")', "true", 'Some("Base Stash")', "1", 'Some((Some("Base Stash"), 1))'):
        ctx.err(f"08zzh_storage: renaming {r.groups() if r else 'not logged'} (want Base Stash, open with the Gas Can, saved)")
    a = re.search(r"autoshot storage asked (\w+) buttons (\d+)", ctx.log)
    if not a or a.groups() != ("true", "2"):
        ctx.err(f"08zzi_storage_pickup: {a.groups() if a else 'not logged'} (want the YES/NO question)")
    p = re.search(r"autoshot storage picked up crates (\d+) dropped \[(.*?)\] in inventory (\d+)", ctx.log)
    if not p or p[1] != "0" or '"DA_GasCan"' not in p[2] or int(p[3]) < 1:
        ctx.err(f"08zzi_storage_pickup: {p.groups() if p else 'not logged'} (want the crate gone, the Gas Can dropped, the crate in the inventory)")


def buildables_snap(ctx):
    """08zzg_snap: a Wooden Foundation placed on the ground; the next one's ghost, on it, snaps
    to a Foundation point 3 m beside it at the same height and is placed; a Wooden Wall's ghost
    snaps to a Wall point (1.5 m out from a foundation's centre, 1.82 m up) and is placed."""
    f = re.search(r"autoshot snap first can build (\w+)", ctx.log)
    if not f or f[1] != "true":
        ctx.err(f"08zzg_snap: the first foundation {f.groups() if f else 'not logged'} (want green)")
    s = re.search(r"autoshot snap second can build (\w+) side ([\d.NaN-]+) rise ([\d.NaN-]+)", ctx.log)
    if not s or s[1] != "true" or abs(float(s[2]) - 3.0) > 0.02 or abs(float(s[3])) > 0.02:
        ctx.err(f"08zzg_snap: the second foundation {s.groups() if s else 'not logged'} (want green, 3 m beside, level)")
    w = re.search(r"autoshot snap wall can build (\w+) out ([\d.NaN-]+) up ([\d.NaN-]+)", ctx.log)
    if not w or w[1] != "true" or abs(float(w[2]) - 1.5) > 0.02 or abs(float(w[3]) - 1.82) > 0.02:
        ctx.err(f"08zzg_snap: the wall {w.groups() if w else 'not logged'} (want green, 1.5 m out, 1.82 m up)")
    p = re.search(r"autoshot snap placed foundations (\d+) walls (\d+) building (\w+)", ctx.log)
    if not p or p.groups() != ("2", "1", "false"):
        ctx.err(f"08zzg_snap: placed {p.groups() if p else 'not logged'} (want 2 foundations, 1 wall, build mode over)")


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


def outlines(ctx):
    """08q_door / 08qa_well / 08zi_trader: with each prompt up, the thing prompted is outlined (the
    door's moving parts or a lab sliding door's frame; the well's StaticMesh copied from the static
    props; the trader's skinned mesh on the mask's layer)."""
    want = {"08q_door": (r"Some\(Door\(\d+\)\)", False), "08qa_well": (r'Some\(Actor \{ class: "BP_WaterWell_C"', True),
            "08zi_trader": (r"Some\(Key\(\d+\)\)", False)}
    for step, (target, copies) in want.items():
        m = re.search(rf"autoshot outline {step} (.*) copies (\d+) meshes (\d+)", ctx.log)
        if not m:
            ctx.err(f"{step}: outline not logged")
        elif not re.match(target, m[1]):
            ctx.err(f"{step}: outlined {m[1]}")
        elif copies and int(m[2]) < 1:
            ctx.err(f"{step}: no copy of the well's mesh drawn ({m[2]})")
        elif int(m[2]) + int(m[3]) < 1:
            ctx.err(f"{step}: nothing drawn into the outline (copies {m[2]}, meshes {m[3]})")


CHECKS = [outlines, 
    bag_windows_open, inspector_opens_more_windows, containers_mark_as_storage, loot_refills, zombie_eyes_glow, more_interaction_montages, binoculars_zoom_and_tag, interaction_montages_play, infestation_haze_comes_and_goes, ai_health_bar_follows_the_shot_zombie, head_gear_is_worn, easter_eggs_show_and_the_nuke_blows, stairwell_and_siren, barber_restyles_the_survivor, hip_fire_raises_the_gun, security_camera_sweeps_and_breaks, lab_corpses_lie_posed, gun_locker_needs_cutters, hidden_cache_opens, repairs_fix_the_rifle, inspector_frames_the_item, drop_selector_shows, permadeath_ends_the_game, power_attack_hits, utility_benches_work, radios_play, launchers_fire_projectiles, placed_turrets_fight, infestations_fall, level_traps, presets_change_the_page, doors_open, benches_craft, traps_catch_and_blow, explosive_barrel_blows, generators_power_lights, beds_sleep_and_respawn, death_leaves_a_crate, radiation_zones_and_the_shower, attachments_work, scope_overlay_and_breath, tactical_attachments_toggle, storage_stores_renames_and_picks_up, buildables_snap, horde_beacon_runs, blood_moon_rises_and_settles, turrets_fire_and_go_down, settlement_stands, wandering_traders, elimination_quest_completes, bandits_fight_and_die, prefab_camp_spawns, particles_play, boss_swings_and_dies, workbench_crafts, journal_lists_quests, laptop_quest_completes, journal_info_shows_the_stats, vehicles_drive, fishing_catches_a_fish, trees_fall_and_wrecks_salvage, trash_piles_break, ran_cleanly, every_step_has_a_screenshot, traders_trade, airdrops_fall_and_open, shots_kill_the_zombie, lockpick_opened, door_opens, keypad_opens_the_door, notes_are_read, clothing_hides_the_body, ladder_climbs, lab_generator_lights_the_lab, shove_and_stomp, frag_grenade_explodes, molotov_sets_the_zombie_burning, damage_numbers_float, recoil_kicks, passive_skills_gain_xp, journal_unlocks_skills, map_shows_discovered_poi, music_plays, combat_roll_rolls, fence_is_climbed_over, animals_flee_bite_and_die, swimming_floats_dives_and_surfaces, sounds_play, body_parts_move_in_step, police_garage_opens, crouch_is_slow_and_quiet, hard_landing_hurts, flashlight_lights_up, sitting_holds_still, night_is_darker_than_day, frame_rate, level_spawners_wake, hound_has_its_health, corpse_opens_as_loot, medical_icons_show, weather_changes_the_scene]


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
