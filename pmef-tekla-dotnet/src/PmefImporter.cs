// PmefImporter.cs
// Tekla Structures Open API plugin — PMEF NDJSON → Tekla model import
//
// Reads a PMEF NDJSON file and creates/updates SteelMember objects in Tekla.
// Uses HasEquivalentIn (targetSystem = "TEKLA_STRUCTURES") to identify
// existing objects for update vs. new objects for creation.
//
// Build: .NET 8, references Tekla.Structures.Model
// Usage: Run with model open: PmefImporter.exe input.ndjson

using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.Json;
using System.Text.Json.Serialization;

using Tekla.Structures;
using Tekla.Structures.Geometry3d;
using Tekla.Structures.Model;

namespace PmefTekla
{
    public class PmefImporter
    {
        private readonly Model _model;
        private int _created = 0;
        private int _updated = 0;
        private int _failed  = 0;

        /// PMEF @id → Tekla GUID (from HasEquivalentIn objects).
        private readonly Dictionary<string, string> _pmefToTeklaGuid = new();

        public PmefImporter(Model model) { _model = model; }

        /// Import all SteelMember objects from a PMEF NDJSON file.
        public void Import(string inputPath)
        {
            Console.WriteLine($"PMEF Import: reading {inputPath}");
            var lines = File.ReadAllLines(inputPath);

            // Pass 1: collect HasEquivalentIn → build PMEF→TeklaGuid map
            foreach (var line in lines)
            {
                if (string.IsNullOrWhiteSpace(line) || !line.Contains("HasEquivalentIn"))
                    continue;
                try
                {
                    var rel = JsonSerializer.Deserialize<JsonElement>(line);
                    if (rel.GetProperty("@type").GetString() == "pmef:HasEquivalentIn"
                        && rel.GetProperty("targetSystem").GetString() == "TEKLA_STRUCTURES")
                    {
                        var sourceId    = rel.GetProperty("sourceId").GetString() ?? "";
                        var targetGuid  = rel.GetProperty("targetSystemId").GetString() ?? "";
                        _pmefToTeklaGuid[sourceId] = targetGuid;
                    }
                }
                catch { /* skip malformed */ }
            }

            Console.WriteLine($"  Found {_pmefToTeklaGuid.Count} Tekla identity mappings");

            // Pass 2: import SteelMember objects
            foreach (var line in lines)
            {
                if (string.IsNullOrWhiteSpace(line)) continue;
                try
                {
                    var obj = JsonSerializer.Deserialize<JsonElement>(line);
                    var type = obj.GetProperty("@type").GetString();
                    if (type == "pmef:SteelMember") ImportMember(obj);
                }
                catch (Exception ex)
                {
                    _failed++;
                    Console.Error.WriteLine($"Import error: {ex.Message}");
                }
            }

            _model.CommitChanges();
            Console.WriteLine($"PMEF Import complete: created={_created}, " +
                              $"updated={_updated}, failed={_failed}");
        }

        private void ImportMember(JsonElement obj)
        {
            var pmefId   = obj.GetProperty("@id").GetString() ?? "";
            var mark     = GetString(obj, "memberMark") ?? "UNK";
            var profile  = GetString(obj, "profileId") ?? "HEA200";
            var sp       = GetPoint(obj, "startPoint");
            var ep       = GetPoint(obj, "endPoint");

            // Strip standard prefix from profileId: "EN:HEA200" → "HEA200"
            var profileDes = profile.Contains(':') ? profile.Split(':')[1] : profile;

            // Get material grade
            var matGrade = obj.TryGetProperty("material", out var mat)
                ? GetString(mat, "grade") ?? "S355JR"
                : "S355JR";

            // Check if object already exists in Tekla (via HasEquivalentIn)
            _pmefToTeklaGuid.TryGetValue(pmefId, out var existingGuid);

            if (existingGuid != null)
            {
                // Update existing member
                var existing = FindByGuid(existingGuid);
                if (existing is Beam beam)
                {
                    UpdateBeam(beam, profileDes, matGrade, sp, ep);
                    _updated++;
                    return;
                }
            }

            // Create new beam
            var newBeam = new Beam
            {
                StartPoint = sp,
                EndPoint   = ep,
                Profile    = { ProfileString = profileDes },
                Material   = { MaterialString = matGrade },
                Name       = mark,
            };
            if (newBeam.Insert())
            {
                // Write PMEF ID as UDA for future round-trips
                newBeam.SetUserProperty("PMEF_ID", pmefId);
                // Write analysis results if present
                WriteAnalysisUdas(newBeam, obj);
                newBeam.Modify();
                _created++;
            }
            else
            {
                _failed++;
                Console.Error.WriteLine($"Failed to insert beam '{mark}'");
            }
        }

        private static void UpdateBeam(
            Beam beam, string profile, string material, Point sp, Point ep)
        {
            beam.Profile.ProfileString   = profile;
            beam.Material.MaterialString = material;
            beam.StartPoint = sp;
            beam.EndPoint   = ep;
            beam.Modify();
        }

        private static void WriteAnalysisUdas(Beam beam, JsonElement obj)
        {
            if (!obj.TryGetProperty("customAttributes", out var attrs)) return;
            if (!attrs.TryGetProperty("analysisResults", out var ar)) return;
            if (ar.ValueKind == JsonValueKind.Null) return;

            if (ar.TryGetProperty("utilisationRatio", out var ur) &&
                ur.ValueKind == JsonValueKind.Number)
                beam.SetUserProperty("PMEF_UTILISATION_RATIO", ur.GetDouble());

            if (ar.TryGetProperty("criticalCheck", out var cc) &&
                cc.ValueKind == JsonValueKind.String)
                beam.SetUserProperty("PMEF_CRITICAL_CHECK", cc.GetString() ?? "");
        }

        private ModelObject? FindByGuid(string guid)
        {
            // Tekla GUID lookup (simplified — full impl uses ModelObjectSelector)
            var selector = _model.GetModelObjectSelector();
            var objs = selector.GetAllObjects(new Type[] { typeof(Beam) });
            while (objs.MoveNext())
            {
                if (objs.Current is Beam beam &&
                    beam.Identifier.GUID.ToString() == guid)
                    return beam;
            }
            return null;
        }

        private static Point GetPoint(JsonElement obj, string key)
        {
            if (!obj.TryGetProperty(key, out var arr) ||
                arr.ValueKind != JsonValueKind.Array ||
                arr.GetArrayLength() < 3)
                return new Point(0, 0, 0);
            return new Point(
                arr[0].GetDouble(),
                arr[1].GetDouble(),
                arr[2].GetDouble()
            );
        }

        private static string? GetString(JsonElement obj, string key)
        {
            if (obj.TryGetProperty(key, out var val) &&
                val.ValueKind == JsonValueKind.String)
                return val.GetString();
            return null;
        }
    }

    public class ImportProgram
    {
        [STAThread]
        static int Main(string[] args)
        {
            if (args.Length < 1) { Console.Error.WriteLine("Usage: PmefImporter <input.ndjson>"); return 1; }

            var model = new Model();
            if (!model.GetConnectionStatus())
            {
                Console.Error.WriteLine("ERROR: Tekla Structures not connected.");
                return 1;
            }
            new PmefImporter(model).Import(args[0]);
            return 0;
        }
    }
}
