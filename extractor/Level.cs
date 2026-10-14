// Level export: turns a .umap into data the Rust port can load.
//
//   placements.json  { "<mesh package>": [[12 floats: Bevy affine, column-major 3x4], ...], ... }
//   actors.json      [ { "class", "name", "transform": [12 floats], "props"? }, ... ]  (non-mesh actors: BPs, lights...)
//                    (`SdExport levelactors <out> <umap>` rewrites just this file)
//   landscape.json + heightmap.png (16-bit) + weight_<layer>.png
//
// Coordinates: UE is centimetres, Z-up, left-handed; the .glb meshes from `SdExport mesh` are
// metres with (x, y, z)_gltf = (X, Z, Y)_ue / 100. Every transform written here is converted the
// same way (B = P * M^T * P with P swapping Y/Z, translation / 100), so meshes drop straight in.

using System.Numerics;
using System.Text.Json;
using CUE4Parse.FileProvider;
using CUE4Parse.UE4.Assets.Exports;
using CUE4Parse.UE4.Assets.Exports.Actor;
using CUE4Parse.UE4.Assets.Exports.Component;
using CUE4Parse.UE4.Assets.Exports.Component.Landscape;
using CUE4Parse.UE4.Assets.Exports.Component.StaticMesh;
using CUE4Parse.UE4.Assets.Exports.Component.SplineMesh;
using CUE4Parse.UE4.Objects.Core.Math;
using CUE4Parse.UE4.Objects.UObject;
using CUE4Parse_Conversion.Dto;
using CUE4Parse_Conversion.Options;
using SixLabors.ImageSharp;
using SkiaSharp;

static class LevelExport
{
    public static void Stats(IFileProvider provider, string pkg)
    {
        var exports = provider.LoadPackage(pkg + ".umap").GetExports().ToList();
        Console.WriteLine($"{exports.Count} exports");
        foreach (var g in exports.GroupBy(e => e.ExportType).OrderByDescending(g => g.Count()).Take(60))
            Console.WriteLine($"{g.Count(),7} {g.Key}");
        int instances = exports.OfType<UInstancedStaticMeshComponent>().Sum(c => c.PerInstanceSMData?.Length ?? 0);
        Console.WriteLine($"ISM instances: {instances}");
    }

    // Every actor with its class, so gameplay actors (spawners, containers, traders, vehicles,
    // lights, Blueprint buildings) can be mapped to Rust systems. Spawners also carry their
    // per-instance property overrides (SpawnAmount, RespawnMethod, box extent...) as `props`.
    // Lab generators, light switches and lights, other generators: linked by actor refs.
    static bool Powered(string cls) => cls.StartsWith("BP_Laboratory") || cls.Contains("Generator") || cls.Contains("PoliceStation") || cls == "BP_Lever_C";
    // Actors whose components are moved per instance (ladder points, item spawner points).
    static bool Placed(string cls) => cls == "BP_Ladder_C" || cls == "BP_SpecialItemContainerSpawner_C" || cls == "BP_Keycard_Spawner_C";

    public static void WriteActors(IEnumerable<UObject> exports, string outDir)
    {
        var actors = new List<object>();
        foreach (var actor in exports.OfType<AActor>())
        {
            var root = actor.GetOrDefault<FPackageIndex>("RootComponent")?.ResolvedObject?.Load() as USceneComponent;
            if (root == null) continue;
            string cls = actor.Class?.Name.Text ?? actor.ExportType;
            Dictionary<string, object?>? props = null;
            // Spawners, and the sky/weather actors whose settings drive day/night and weather.
            if (cls.Contains("AISpawner") || cls.StartsWith("Ultra_Dynamic") || cls.StartsWith("Weather_Override") || cls.StartsWith("UDS_") || cls == "BP_SkySettings_C" || cls.Contains("Door") || Placed(cls) || Powered(cls))
            {
                props = new();
                foreach (var tag in actor.Properties)
                {
                    var v = tag.Tag?.GenericValue;
                    // Class references (SpawnedActorClass) by name; other object refs are dropped.
                    // Weather actors and the power network keep object refs (actor arrays: a
                    // generator's Actors, a switch's Lights...) as names.
                    bool weather = !cls.Contains("AISpawner");
                    if (v is FPackageIndex idx)
                    {
                        // Ladders, item spawners: a component moved on the instance (PlayerPostition,
                        // Spawn1...) as "<Component>.RelativeLocation" (UE cm); absent = the BP template's.
                        if (Placed(cls) && idx.ResolvedObject?.Load() is USceneComponent sc)
                            foreach (var p in sc.Properties.Where(p => p.Name.Text is "RelativeLocation" or "RelativeRotation"))
                                props[$"{tag.Name.Text}.{p.Name.Text}"] = Newtonsoft.Json.Linq.JToken.FromObject(p.Tag?.GenericValue ?? "").ToString(Newtonsoft.Json.Formatting.None);
                        // Class references (SpawnedActorClass); weather actors keep asset refs too.
                        if (idx.Name.EndsWith("_C") || weather) props[tag.Name.Text] = idx.Name;
                        continue;
                    }
                    if (v is UObject) continue;
                    var json = Newtonsoft.Json.Linq.JToken.FromObject(v ?? "").ToString(Newtonsoft.Json.Formatting.None);
                    if (!weather && json.Contains("\"ObjectName\"")) continue; // component/actor references
                    props[tag.Name.Text] = json;
                }
            }
            actors.Add(new { @class = cls, name = actor.Name, label = actor.ActorLabel, transform = ToBevy(root.GetAbsoluteTransform().ToMatrixWithScale()), props });
        }
        File.WriteAllText(Path.Combine(outDir, "actors.json"), JsonSerializer.Serialize(actors));
        Console.WriteLine($"actors: {actors.Count}");
    }

    // Point lights on Blueprint actors (lamps, fires, lab lights): lights.json
    //   [ { "owner", "pos": [x, y, z], "intensity", "units", "color": [r, g, b], "radius", "visible" } ]
    // Intensity in the component's IntensityUnits (UE default: candelas for point lights).
    public static void Lights(IFileProvider provider, string pkg, string outDir)
    {
        var exports = provider.LoadPackage(pkg + ".umap").GetExports().ToList();
        var lights = new List<object>();
        foreach (var c in exports.OfType<USceneComponent>().Where(e => e.ExportType == "PointLightComponent"))
        {
            var m = ToBevy(c.GetAbsoluteTransform().ToMatrixWithScale());
            var col = c.GetOrDefault<FColor>("LightColor", new FColor(255, 255, 255, 255));
            var owner = c.Outer?.Name.ToString() ?? "";
            // Only properties that differ from the owner Blueprint's component template are in the
            // map; `set` lists them so the template supplies the rest (gen_lights.py).
            var set = c.Properties.Select(t => t.Name.Text).ToArray();
            lights.Add(new
            {
                owner,
                component = c.Name,
                set,
                pos = new[] { m[9], m[10], m[11] },
                intensity = c.GetOrDefault<float>("Intensity", 5000f),
                units = c.GetOrDefault<FName>("IntensityUnits").Text,
                color = new[] { col.R / 255f, col.G / 255f, col.B / 255f },
                radius = c.GetOrDefault<float>("AttenuationRadius", 1000f) / 100f,
                visible = c.GetOrDefault<bool>("bVisible", true),
                shadows = c.GetOrDefault<bool>("CastShadows", true),
            });
        }
        File.WriteAllText(Path.Combine(outDir, "lights.json"), JsonSerializer.Serialize(lights));
        Console.WriteLine($"lights: {lights.Count}");
    }

    public static void Actors(IFileProvider provider, string pkg, string outDir)
    {
        Directory.CreateDirectory(outDir);
        WriteActors(provider.LoadPackage(pkg + ".umap").GetExports(), outDir);
    }

    static float[] ToBevy(FMatrix m)
    {
        // FMatrix is row-vector (v * M): rows 0-2 are the axes, row 3 the translation.
        var a = new float[3, 3];
        for (int r = 0; r < 3; r++)
            for (int c = 0; c < 3; c++)
                a[r, c] = (float)m.M(c, r); // transpose -> column-vector convention
        int[] p = { 0, 2, 1 };
        var b = new float[12];
        // column-major: b[col*3 + row]
        for (int col = 0; col < 3; col++)
            for (int row = 0; row < 3; row++)
                b[col * 3 + row] = a[p[row], p[col]];
        b[9] = (float)m.M(3, 0) / 100f;
        b[10] = (float)m.M(3, 2) / 100f;
        b[11] = (float)m.M(3, 1) / 100f;
        return b;
    }

    static double M(this FMatrix m, int r, int c) => (r, c) switch
    {
        (0, 0) => m.M00, (0, 1) => m.M01, (0, 2) => m.M02,
        (1, 0) => m.M10, (1, 1) => m.M11, (1, 2) => m.M12,
        (2, 0) => m.M20, (2, 1) => m.M21, (2, 2) => m.M22,
        (3, 0) => m.M30, (3, 1) => m.M31, (3, 2) => m.M32,
        _ => 0,
    };

    static string? PathOf(FPackageIndex? idx)
    {
        var o = idx?.ResolvedObject;
        if (o == null) return null;
        var full = o.GetPathName(); // "SurrounDead/Content/X/Y.Y" or "/Game/X/Y.Y"
        var pkgPart = full.Split('.')[0];
        if (pkgPart.StartsWith("/Game/")) pkgPart = "SurrounDead/Content/" + pkgPart["/Game/".Length..];
        return pkgPart.TrimStart('/');
    }

    static bool Hidden(UObject o) =>
        o.GetOrDefault("bHiddenInGame", false) || !o.GetOrDefault("bVisible", true);

    public static void Run(IFileProvider provider, string pkg, string outDir)
    {
        Directory.CreateDirectory(outDir);
        var exports = provider.LoadPackage(pkg + ".umap").GetExports().ToList();

        var placements = new Dictionary<string, List<float[]>>();
        void Add(string mesh, FTransform t)
        {
            if (!placements.TryGetValue(mesh, out var list)) placements[mesh] = list = new();
            list.Add(ToBevy(t.ToMatrixWithScale()));
        }

        int skipped = 0;
        var splines = new List<object>();
        static float[] V(FVector v) => new[] { (float)v.X, (float)v.Y, (float)v.Z };
        static float[] V2(FVector2D v) => new[] { (float)v.X, (float)v.Y };
        foreach (var comp in exports.OfType<UStaticMeshComponent>())
        {
            try
            {
                if (Hidden(comp)) { skipped++; continue; }
                var mesh = PathOf(comp.GetStaticMesh());
                if (mesh == null) continue;
                var world = comp.GetAbsoluteTransform();
                if (comp is USplineMeshComponent spline)
                {
                    // Deformed at load time (see world/level.rs); parameters stay in UE component space.
                    var sp = spline.SplineParams;
                    splines.Add(new
                    {
                        mesh,
                        transform = ToBevy(world.ToMatrixWithScale()),
                        axis = (int)spline.ForwardAxis,
                        up = V(spline.SplineUpDir),
                        boundary = new[] { spline.SplineBoundaryMin, spline.SplineBoundaryMax },
                        smooth = spline.bSmoothInterpRollScale,
                        start = new { pos = V(sp.StartPos), tangent = V(sp.StartTangent), scale = V2(sp.StartScale), roll = sp.StartRoll, offset = V2(sp.StartOffset) },
                        end = new { pos = V(sp.EndPos), tangent = V(sp.EndTangent), scale = V2(sp.EndScale), roll = sp.EndRoll, offset = V2(sp.EndOffset) },
                    });
                }
                else if (comp is UInstancedStaticMeshComponent ism)
                {
                    foreach (var inst in ism.GetInstances() ?? Array.Empty<FInstancedStaticMeshInstanceData>())
                        Add(mesh, inst.TransformData * world);
                }
                else
                {
                    Add(mesh, world);
                }
            }
            catch (Exception e)
            {
                Console.Error.WriteLine($"component {comp.Name}: {e.Message}");
            }
        }
        File.WriteAllText(Path.Combine(outDir, "placements.json"), JsonSerializer.Serialize(placements));
        File.WriteAllText(Path.Combine(outDir, "splines.json"), JsonSerializer.Serialize(splines));
        Console.WriteLine($"spline meshes: {splines.Count}");
        Console.WriteLine($"placements: {placements.Count} meshes, {placements.Values.Sum(l => l.Count)} instances ({skipped} hidden skipped)");

        WriteActors(exports, outDir);

        ExportLandscape(exports, outDir);
    }

    static void ExportLandscape(List<UObject> exports, string outDir)
    {
        var proxies = exports.OfType<ALandscapeProxy>().ToList();
        var comps = exports.OfType<ULandscapeComponent>().ToArray();
        if (proxies.Count == 0 || comps.Length == 0) { Console.WriteLine("no landscape"); return; }
        var proxy = proxies[0];
        var root = proxy.GetOrDefault<FPackageIndex>("RootComponent")?.ResolvedObject?.Load() as USceneComponent;
        var t = root?.GetAbsoluteTransform() ?? FTransform.Identity;

        using var dto = new LandscapeMeshDto(proxy, ELandscapeFlags.Heightmap | ELandscapeFlags.Weightmap, comps);
        dto.HeightmapTexture?.SaveAsPng(Path.Combine(outDir, "heightmap.png"));
        var layers = new List<string>();
        foreach (var (name, bmp) in dto.BitmapTextures)
        {
            var safe = string.Concat(name.Select(ch => char.IsLetterOrDigit(ch) || ch == '_' ? ch : '_'));
            using var img = SKImage.FromBitmap(bmp);
            using var data = img.Encode(SKEncodedImageFormat.Png, 100);
            File.WriteAllBytes(Path.Combine(outDir, $"weight_{safe}.png"), data.ToArray());
            layers.Add(safe);
        }

        int minX = comps.Min(c => c.SectionBaseX), minY = comps.Min(c => c.SectionBaseY);
        int maxX = comps.Max(c => c.SectionBaseX + c.ComponentSizeQuads), maxY = comps.Max(c => c.SectionBaseY + c.ComponentSizeQuads);
        var info = new
        {
            // UE values; the Rust side converts (see world/landscape.rs).
            location = new[] { t.Translation.X, t.Translation.Y, t.Translation.Z },
            scale = new[] { t.Scale3D.X, t.Scale3D.Y, t.Scale3D.Z },
            rotation = new[] { t.Rotation.X, t.Rotation.Y, t.Rotation.Z, t.Rotation.W },
            section_min = new[] { minX, minY },
            section_max = new[] { maxX, maxY },
            component_size_quads = comps[0].ComponentSizeQuads,
            heightmap_size = dto.HeightmapTexture == null ? null : new[] { dto.HeightmapTexture.Width, dto.HeightmapTexture.Height },
            material = PathOf(proxy.LandscapeMaterial),
            layers,
            components = comps.Length,
        };
        File.WriteAllText(Path.Combine(outDir, "landscape.json"), JsonSerializer.Serialize(info, new JsonSerializerOptions { WriteIndented = true }));
        Console.WriteLine($"landscape: {comps.Length} components, sections {minX},{minY}..{maxX},{maxY}, layers {string.Join(",", layers)}");
    }
}
