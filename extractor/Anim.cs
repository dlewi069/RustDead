// Animation export: CUE4Parse can't write AnimSequences as glTF, so sample each bone's local
// transform per frame and write JSON the Rust side turns into Bevy AnimationClips.
//
//   { "fps": 30, "frames": N, "duration": s, "rate": RateScale,
//     "notifies": [{ "name": "Footstep", "time": s }...],
//     "bones": { "<bone>": { "t": [[x,y,z]...], "r": [[x,y,z,w]...], "s": [[x,y,z]...] } } }
//
// Values are already in the glb skeleton's space (matches CUE4Parse's glTF skeleton export):
// translation (X, Z, Y) / 100, rotation (x, z, y, -w), scale (X, Z, Y). A bone whose track is
// constant gets a single key; bones without a track are omitted (they keep the bind pose).
// Bones the skeleton sets to `TranslationRetargetingMode::Skeleton` (everything but Root, Pelvis
// and the IK bones here) take their translation from the reference pose, as UE does; clips
// retargeted from other proportions (ThirdPersonIdle/Walk) otherwise bend the limbs.

using System.Text.Json;
using CUE4Parse.FileProvider;
using CUE4Parse.UE4.Assets.Exports.Animation;
using CUE4Parse.UE4.Objects.Core.Math;
using CUE4Parse.UE4.Objects.UObject;
using CUE4Parse.UE4.Assets.Objects;
using CUE4Parse_Conversion.Animations;

static class AnimExport
{
    public static void Run(IFileProvider provider, string outDir, IEnumerable<string> packages)
    {
        foreach (var pkg in packages)
        {
            try
            {
                var seq = provider.LoadPackage(pkg + ".uasset").GetExports().OfType<UAnimSequence>().FirstOrDefault();
                if (seq == null) continue;
                var set = seq.ConvertAnims();
                var anim = set.Sequences.FirstOrDefault();
                if (anim == null) continue;
                var bones = set.Skeleton.ReferenceSkeleton.FinalRefBoneInfo;
                var refPose = set.Skeleton.ReferenceSkeleton.FinalRefBonePose;
                var modes = set.Skeleton.BoneTree;
                int frames = Math.Max(anim.NumFrames, 1);

                var outBones = new Dictionary<string, object>();
                for (int b = 0; b < bones.Length && b < anim.Tracks.Count; b++)
                {
                    var track = anim.Tracks[b];
                    if (!track.HasKeys()) continue;
                    var t = new List<float[]>(); var r = new List<float[]>(); var s = new List<float[]>();
                    for (int f = 0; f < frames; f++)
                    {
                        var q = refPose[b].Rotation; var p = refPose[b].Translation; var sc = refPose[b].Scale3D;
                        track.GetBoneTransform(f, frames, ref q, ref p, ref sc);
                        if (modes != null && b < modes.Length && modes[b] == EBoneTranslationRetargetingMode.Skeleton)
                            p = refPose[b].Translation;
                        // Additive clips (Fire_Rifle_Ironsights) come back as full poses on their
                        // base pose, but with scale 4 on every bone: keep the reference scale.
                        if (anim.IsAdditive) sc = refPose[b].Scale3D;
                        t.Add(new[] { (float)p.X / 100f, (float)p.Z / 100f, (float)p.Y / 100f });
                        r.Add(new[] { (float)q.X, (float)q.Z, (float)q.Y, -(float)q.W });
                        s.Add(new[] { (float)sc.X, (float)sc.Z, (float)sc.Y });
                    }
                    outBones[bones[b].Name.Text] = new { t = Collapse(t), r = Collapse(r), s = Collapse(s) };
                }
                // Named notifies (AnimNotify_<Name> events in the AnimBP), by trigger time.
                var notifies = new List<object>();
                foreach (var n in seq.GetOrDefault<FStructFallback[]>("Notifies", Array.Empty<FStructFallback>()))
                {
                    var name = n.GetOrDefault<FName>("NotifyName").Text;
                    if (string.IsNullOrEmpty(name) || name == "None") continue;
                    notifies.Add(new { name, time = n.GetOrDefault<float>("LinkValue") });
                }
                var json = new
                {
                    fps = anim.FramesPerSecond,
                    rate = seq.GetOrDefault<float>("RateScale", 1f),
                    notifies,
                    frames,
                    duration = frames > 1 ? (frames - 1) / anim.FramesPerSecond : 0,
                    additive = anim.IsAdditive,
                    bones = outBones,
                };
                // Same layout as the mesh/texture exports: <out>/SurrounDead/Content/...
                var path = Path.Combine(outDir, pkg + ".anim.json");
                Directory.CreateDirectory(Path.GetDirectoryName(path)!);
                File.WriteAllText(path, JsonSerializer.Serialize(json));
                Console.WriteLine($"ok   {pkg} ({frames} frames @ {anim.FramesPerSecond} fps, {outBones.Count} bones)");
            }
            catch (Exception e)
            {
                Console.Error.WriteLine($"FAIL {pkg}: {(Environment.GetEnvironmentVariable("SD_DEBUG") != null ? e.ToString() : e.Message)}");
            }
        }
    }

    static List<float[]> Collapse(List<float[]> keys) =>
        keys.All(k => k.Zip(keys[0], (a, b) => Math.Abs(a - b) < 1e-5).All(x => x)) ? new List<float[]> { keys[0] } : keys;
}
