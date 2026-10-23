using CUE4Parse_Conversion.Sounds;
// SdExport: pulls textures and meshes out of the SurrounDead UE 5.3 pak via CUE4Parse.
//
// Usage:
//   SdExport list <substring>                       list package paths containing substring
//   SdExport tex  <outdir> <package path|prefix/>... export Texture2D as PNG
//   SdExport json <outdir> <package path|prefix/>... dump all exports as JSON (widget layouts, data tables)
//   SdExport levelstats <level package>             export-type counts for a .umap
//   SdExport level <outdir> <level package>          placements/actors/landscape (see Level.cs)
//   SdExport animjson <outdir> <package path|prefix/>... AnimSequences as per-bone keyframe JSON (Anim.cs)
//   SdExport mesh <outdir> <package path|prefix/>... export StaticMesh/SkeletalMesh as glTF (.glb)
//
// Package paths look like SurrounDead/Content/UI/Textures/TitleTransparent1 (no extension).
// A trailing "/" exports every package under that folder.
// Env: SD_PAK_DIR (default: Steam install Paks dir), SD_USMAP (default: Binaries/Win64/Mappings.usmap).

using CUE4Parse.Compression;
using CUE4Parse.Encryption.Aes;
using CUE4Parse.FileProvider;
using CUE4Parse.MappingsProvider.Usmap;
using CUE4Parse.UE4.Assets.Exports.SkeletalMesh;
using CUE4Parse.UE4.Assets.Exports.StaticMesh;
using CUE4Parse.UE4.Assets.Exports.Texture;
using CUE4Parse.UE4.Objects.Core.Misc;
using CUE4Parse.UE4.Versions;
using CUE4Parse_Conversion;
using CUE4Parse_Conversion.Options;

const string GameDir = "/ntfs/SteamLibrary/steamapps/common/SurrounDead/SurrounDead";
var pakDir = Environment.GetEnvironmentVariable("SD_PAK_DIR") ?? $"{GameDir}/Content/Paks";
var usmap = Environment.GetEnvironmentVariable("SD_USMAP") ?? $"{GameDir}/Binaries/Win64/Mappings.usmap";

if (args.Length < 2)
{
    Console.Error.WriteLine("usage: SdExport list <substr> | tex <outdir> <paths...> | mesh <outdir> <paths...>");
    return 1;
}

var oodleDir = Path.Combine(AppContext.BaseDirectory, "oodle");
Directory.CreateDirectory(oodleDir);
try { OodleHelper.Initialize(Path.Combine(oodleDir, OodleHelper.OodleFileName)); } catch (Exception e) { Console.Error.WriteLine($"oodle init skipped: {e.Message}"); }

var provider = new DefaultFileProvider(pakDir, SearchOption.TopDirectoryOnly, new VersionContainer(EGame.GAME_UE5_3));
provider.MappingsContainer = new FileUsmapTypeMappingsProvider(usmap);
// SD_SCRIPT=1: also deserialize Blueprint bytecode (functions' Kismet statements) into `json`
// dumps, for reading game logic that isn't data (sweet spots, formulas, timings).
provider.ReadScriptData = Environment.GetEnvironmentVariable("SD_SCRIPT") == "1";
provider.Initialize();
provider.SubmitKey(new FGuid(), new FAesKey(new byte[32]));
provider.PostMount();
// Plugin content mount points (/ACLPlugin/... -> Engine/Plugins/Animation/ACLPlugin/Content/...),
// so ACL-compressed clips (FoldingGate_Anim) find their codec settings.
provider.LoadVirtualPaths();

IEnumerable<string> Expand(IEnumerable<string> paths) =>
    paths.SelectMany(p => p.EndsWith('/')
        ? provider.Files.Keys
            .Where(k => k.StartsWith(p, StringComparison.OrdinalIgnoreCase) && k.EndsWith(".uasset", StringComparison.OrdinalIgnoreCase))
            .Select(k => k[..^".uasset".Length])
        : new[] { p });

string OutPath(string outDir, string pkg, string ext)
{
    var rel = pkg.StartsWith("SurrounDead/Content/") ? pkg["SurrounDead/Content/".Length..] : pkg;
    var path = Path.Combine(outDir, rel + ext);
    Directory.CreateDirectory(Path.GetDirectoryName(path)!);
    return path;
}

var cmd = args[0];
if (cmd == "list")
{
    foreach (var k in provider.Files.Keys.Where(k => k.Contains(args[1], StringComparison.OrdinalIgnoreCase)).OrderBy(k => k))
        Console.WriteLine(k);
    return 0;
}

// A pak file's bytes as they are: raw <out> <file>... A .ufont is the TrueType file; it's written
// as .ttf, which Bevy's font loader recognises.
if (cmd == "raw")
{
    foreach (var f in args.Skip(2))
    {
        var path = OutPath(args[1], f.EndsWith(".ufont") ? f[..^".ufont".Length] : f, f.EndsWith(".ufont") ? ".ttf" : "");
        File.WriteAllBytes(path, provider.SaveAsset(f));
        Console.WriteLine($"raw {f} -> {path}");
    }
    return 0;
}

if (cmd == "levelstats")
{
    LevelExport.Stats(provider, args[1]);
    return 0;
}
if (cmd == "level")
{
    LevelExport.Run(provider, args[2], args[1]);
    return 0;
}

if (cmd == "levellights")
{
    LevelExport.Lights(provider, args[2], args[1]);
    return 0;
}

if (cmd == "levelparticles")
{
    LevelExport.Particles(provider, args[2], args[1]);
    return 0;
}

if (cmd == "levelharvest")
{
    LevelExport.Harvest(provider, args[1], args[2]);
    return 0;
}

if (cmd == "levelactors")
{
    LevelExport.Actors(provider, args[2], args[1]);
    return 0;
}

if (cmd == "animjson")
{
    AnimExport.Run(provider, args[1], Expand(args.Skip(2)));
    return 0;
}

if (cmd == "json")
{
    foreach (var pkg in Expand(args.Skip(2)))
    {
        var exports = provider.LoadPackage(pkg + ".uasset").GetExports();
        var path = Path.Combine(args[1], (pkg.StartsWith("SurrounDead/Content/") ? pkg["SurrounDead/Content/".Length..] : pkg) + ".json");
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        File.WriteAllText(path, Newtonsoft.Json.JsonConvert.SerializeObject(exports, Newtonsoft.Json.Formatting.Indented));
        Console.WriteLine($"json {pkg} -> {path}");
    }
    return 0;
}

// SoundWaves decoded to their own format (ogg, wav, binka, ...): audio <out> <pkg or folder/>...
if (cmd == "audio")
{
    int ok = 0, n = 0;
    foreach (var pkg in Expand(args.Skip(2)))
    {
        try
        {
            foreach (var export in provider.LoadPackage(pkg + ".uasset").GetExports())
            {
                if (export is not CUE4Parse.UE4.Assets.Exports.Sound.USoundWave wave) continue;
                n++;
                wave.Decode(true, out var format, out var data);
                if (data == null || string.IsNullOrEmpty(format)) { Console.Error.WriteLine($"FAIL {pkg}: no data"); continue; }
                var path = OutPath(args[1], pkg, "." + format.ToLowerInvariant());
                File.WriteAllBytes(path, data);
                Console.WriteLine($"ok   {pkg} -> {path}");
                ok++;
            }
        }
        catch (Exception e)
        {
            Console.Error.WriteLine($"FAIL {pkg}: {e.Message}");
        }
    }
    Console.Error.WriteLine($"done: {ok}/{n} sounds");
    return 0;
}

var outDir = args[1];
var session = new ExportSession(null!);
int queued = 0;
foreach (var pkg in Expand(args.Skip(2)))
{
    try
    {
        foreach (var export in provider.LoadPackage(pkg + ".uasset").GetExports())
        {
            if ((cmd == "tex" && export is UTexture2D) || (cmd == "mesh" && export is UStaticMesh or USkeletalMesh))
            {
                session.Add(export);
                queued++;
            }
        }
    }
    catch (Exception e)
    {
        Console.Error.WriteLine($"FAIL load {pkg}: {e.Message}");
    }
}

var options = new ExportOptions(
    meshFormat: EMeshFormat.Gltf2,
    textureFormat: ETextureFormat.Png,
    exportMaterials: cmd == "mesh");
var results = await session.RunAsync(outDir, options, null!, CancellationToken.None);
foreach (var r in results)
{
    if (r.Success) Console.WriteLine($"ok   {r.ObjectPath} -> {string.Join(", ", r.DiskFilePaths.Select(FixPath))}");
    else Console.Error.WriteLine($"FAIL {r.ObjectPath}: {r.Error?.Message}");
}
// CUE4Parse-Conversion joins paths with '\\' even on Linux, so files land in the cwd
// with backslashes in their names. Move them to where they were meant to go.
static string FixPath(string written)
{
    if (!written.Contains('\\') || !File.Exists(written)) return written;
    var target = written.Replace('\\', '/');
    Directory.CreateDirectory(Path.GetDirectoryName(target)!);
    File.Move(written, target, true);
    return target;
}

Console.Error.WriteLine($"done: {results.Count(r => r.Success)}/{queued} exported");
return 0;
