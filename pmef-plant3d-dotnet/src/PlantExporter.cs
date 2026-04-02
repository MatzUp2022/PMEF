// PlantExporter.cs
// AutoCAD Plant 3D — PMEF JSON export via Plant SDK
//
// Reads equipment and line data from the Plant 3D Project Data Store (PDS)
// via the Autodesk Plant SDK (.NET API) and writes a structured JSON export
// consumed by pmef-adapter-plant3d (Rust).
//
// Build:  .NET 8, x64, references:
//         Autodesk.ProcessPower.PlantProject.dll
//         Autodesk.AutoCAD.Interop.dll (for DWG handle resolution)
// Usage:  Run inside AutoCAD Plant 3D via the NETLOAD command:
//         Command: NETLOAD → select PlantExporter.dll
//         Command: PMEFEXPORT → prompts for output path
//
// SDK reference: Autodesk Plant SDK Developer's Guide 2024

using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.Json;
using System.Text.Json.Serialization;

using Autodesk.AutoCAD.ApplicationServices;
using Autodesk.AutoCAD.DatabaseServices;
using Autodesk.AutoCAD.EditorInput;
using Autodesk.AutoCAD.Geometry;
using Autodesk.AutoCAD.Runtime;
using Autodesk.ProcessPower.PlantProject;
using Autodesk.ProcessPower.ProjectManager;
using Autodesk.ProcessPower.PnP3dObjects;
using Autodesk.ProcessPower.DataLinks;
using Autodesk.ProcessPower.DataObjects;

[assembly: CommandClass(typeof(PmefPlant3D.PlantExporter))]
[assembly: CommandClass(typeof(PmefPlant3D.PlantImporter))]

namespace PmefPlant3D
{
    // ──────────────────────────────────────────────────────────────────────────
    // JSON export schema (mirrors equipment.rs)
    // ──────────────────────────────────────────────────────────────────────────

    public record P3dNozzleRec(
        string NozzleNumber,
        string? Service,
        double NominalDiameterIn,
        string? FlangeRating,
        string? FacingType,
        double[] PositionMm,
        double[] Direction,
        string? ConnectedLineTag
    );

    public record P3dEquipmentRec(
        string Handle,
        string TagNumber,
        string EquipmentClass,
        string? Description,
        string? PidTag,
        List<string> ConnectedLines,
        List<P3dNozzleRec> Nozzles,
        double? DesignPressurePsig,
        double? DesignTemperatureF,
        double? OperatingPressurePsig,
        double? OperatingTemperatureF,
        string? Material,
        string? DesignCode,
        string? Manufacturer,
        string? Model,
        double? WeightLbs,
        double? MotorPowerHp,
        double? DesignFlowGpm,
        double? DesignHeadFt,
        double? VolumeGal,
        double? HeatDutyBtuh,
        double? HeatTransferAreaFt2,
        double[]? BboxMinMm,
        double[]? BboxMaxMm,
        Dictionary<string, object?> Udas
    );

    // ──────────────────────────────────────────────────────────────────────────
    // Exporter
    // ──────────────────────────────────────────────────────────────────────────

    public class PlantExporter
    {
        [CommandMethod("PMEFEXPORT", CommandFlags.Modal)]
        public static void RunExport()
        {
            var doc = Application.DocumentManager.MdiActiveDocument;
            var ed  = doc.Editor;

            // Prompt for output path
            var opts = new PromptStringOptions("\nPMEF export path [equipment.json]: ")
            { DefaultValue = "plant3d-equipment.json", AllowSpaces = true };
            var result = ed.GetString(opts);
            if (result.Status != PromptStatus.OK) return;

            var outputPath = result.StringResult.Trim();
            if (string.IsNullOrEmpty(outputPath)) outputPath = "plant3d-equipment.json";

            ed.WriteMessage($"\nPMEF Export starting → {outputPath}");

            try
            {
                var exporter = new PlantExporter();
                var equipment = exporter.ExportAllEquipment(doc);
                var opts2 = new JsonSerializerOptions
                {
                    WriteIndented = true,
                    PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
                    DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
                };
                File.WriteAllText(outputPath, JsonSerializer.Serialize(equipment, opts2));
                ed.WriteMessage($"\nPMEF Export complete: {equipment.Count} objects → {outputPath}");
            }
            catch (System.Exception ex)
            {
                ed.WriteMessage($"\nPMEF Export FAILED: {ex.Message}");
            }
        }

        public List<P3dEquipmentRec> ExportAllEquipment(Document doc)
        {
            var result = new List<P3dEquipmentRec>();
            var db = doc.Database;

            using (var tr = db.TransactionManager.StartTransaction())
            {
                // Iterate the model space for Plant 3D equipment objects
                var bt  = (BlockTable)tr.GetObject(db.BlockTableId, OpenMode.ForRead);
                var btr = (BlockTableRecord)tr.GetObject(
                    bt[BlockTableRecord.ModelSpace], OpenMode.ForRead);

                foreach (ObjectId id in btr)
                {
                    var obj = tr.GetObject(id, OpenMode.ForRead);
                    if (obj is Equipment3d equip)
                    {
                        try { result.Add(MapEquipment(equip, tr)); }
                        catch (System.Exception ex) {
                            Application.DocumentManager.MdiActiveDocument.Editor
                                .WriteMessage($"\n  Skip {equip.Handle}: {ex.Message}");
                        }
                    }
                }
                tr.Commit();
            }
            return result;
        }

        private static P3dEquipmentRec MapEquipment(Equipment3d equip, Transaction tr)
        {
            var handle = equip.Handle.ToString();
            var props  = equip.GetProperties();

            // Tag number
            var tag = GetProp(props, "TagNumber")
                   ?? GetProp(props, "Equipment Tag")
                   ?? equip.Name ?? handle;

            // Equipment class from Plant 3D category
            var category = equip.Category?.CategoryName ?? "Unknown";

            // Nozzles
            var nozzles = new List<P3dNozzleRec>();
            foreach (ObjectId nozzId in equip.NozzleIds)
            {
                var nozz = tr.GetObject(nozzId, OpenMode.ForRead) as Nozzle3d;
                if (nozz == null) continue;
                var np = nozz.GetProperties();
                var pos = nozz.Position;
                var dir = nozz.NozzleDirection;
                nozzles.Add(new P3dNozzleRec(
                    NozzleNumber:      GetProp(np, "NozzleNumber") ?? nozz.Name,
                    Service:           GetProp(np, "Service"),
                    NominalDiameterIn: GetPropDouble(np, "NominalDiameter") ?? 4.0,
                    FlangeRating:      GetProp(np, "FlangeRating"),
                    FacingType:        GetProp(np, "FacingType") ?? "RF",
                    PositionMm:        PointToMmArray(pos),
                    Direction:         new[] { dir.X, dir.Y, dir.Z },
                    ConnectedLineTag:  GetProp(np, "ConnectedLine")
                ));
            }

            // Bounding box
            var ext = equip.GeometricExtents;
            var bboxMin = PointToMmArray(ext.MinPoint);
            var bboxMax = PointToMmArray(ext.MaxPoint);

            return new P3dEquipmentRec(
                Handle:              handle,
                TagNumber:           tag,
                EquipmentClass:      category,
                Description:         GetProp(props, "Description"),
                PidTag:              GetProp(props, "P&IDTag"),
                ConnectedLines:      GetConnectedLines(equip, tr),
                Nozzles:             nozzles,
                DesignPressurePsig:  GetPropDouble(props, "DesignPressure"),
                DesignTemperatureF:  GetPropDouble(props, "DesignTemperature"),
                OperatingPressurePsig: GetPropDouble(props, "OperatingPressure"),
                OperatingTemperatureF: GetPropDouble(props, "OperatingTemperature"),
                Material:            GetProp(props, "Material"),
                DesignCode:          GetProp(props, "DesignCode"),
                Manufacturer:        GetProp(props, "Manufacturer"),
                Model:               GetProp(props, "Model"),
                WeightLbs:           GetPropDouble(props, "Weight"),
                MotorPowerHp:        GetPropDouble(props, "MotorPower"),
                DesignFlowGpm:       GetPropDouble(props, "DesignFlow"),
                DesignHeadFt:        GetPropDouble(props, "DesignHead"),
                VolumeGal:           GetPropDouble(props, "Volume"),
                HeatDutyBtuh:        GetPropDouble(props, "HeatDuty"),
                HeatTransferAreaFt2: GetPropDouble(props, "HeatTransferArea"),
                BboxMinMm:           bboxMin,
                BboxMaxMm:           bboxMax,
                Udas:                GetUdas(props)
            );
        }

        private static List<string> GetConnectedLines(Equipment3d equip, Transaction tr)
        {
            var lines = new List<string>();
            // Equipment → connected PipingLine objects via DataLink
            var dl = DataLinksManager.GetManager();
            foreach (ObjectId linkedId in dl.GetLinks(equip.ObjectId))
            {
                var linked = tr.GetObject(linkedId, OpenMode.ForRead);
                if (linked is PipingLine3d line)
                {
                    var lp = line.GetProperties();
                    var lineTag = GetProp(lp, "LineNumber") ?? linked.ObjectId.ToString();
                    lines.Add(lineTag);
                }
            }
            return lines;
        }

        // ── Helpers ───────────────────────────────────────────────────────────

        private static double[] PointToMmArray(Point3d p)
        {
            // AutoCAD stores coordinates in inches; convert to mm
            const double IN_TO_MM = 25.4;
            return new[] { p.X * IN_TO_MM, p.Y * IN_TO_MM, p.Z * IN_TO_MM };
        }

        private static string? GetProp(
            PnPPropertyCollection props, string name)
        {
            try
            {
                var prop = props[name];
                var val = prop?.Value?.ToString();
                return string.IsNullOrWhiteSpace(val) ? null : val;
            }
            catch { return null; }
        }

        private static double? GetPropDouble(
            PnPPropertyCollection props, string name)
        {
            var s = GetProp(props, name);
            return s != null && double.TryParse(s,
                System.Globalization.NumberStyles.Any,
                System.Globalization.CultureInfo.InvariantCulture, out var v) ? v : null;
        }

        private static Dictionary<string, object?> GetUdas(PnPPropertyCollection props)
        {
            var udas = new Dictionary<string, object?>();
            // Export all non-null properties as UDAs for round-trip support
            var udaNames = new[]
            {
                "ErectionSequence", "ShopMark", "InspectionClass",
                "FireZone", "AreaClassification", "PaintSystem",
                "EquipmentWeight", "EmptyWeight", "OperatingWeight",
                "PurchaseOrder", "DeliveryDate", "Vendor"
            };
            foreach (var name in udaNames)
            {
                var val = GetProp(props, name);
                if (val != null) udas[name] = val;
            }
            return udas;
        }
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Importer — write PMEF objects back to Plant 3D PDS
    // ──────────────────────────────────────────────────────────────────────────

    public class PlantImporter
    {
        [CommandMethod("PMEFIMPORT", CommandFlags.Modal)]
        public static void RunImport()
        {
            var doc = Application.DocumentManager.MdiActiveDocument;
            var ed  = doc.Editor;

            var opts = new PromptStringOptions("\nPMEF import path [output.ndjson]: ")
            { DefaultValue = "output.ndjson", AllowSpaces = true };
            var result = ed.GetString(opts);
            if (result.Status != PromptStatus.OK) return;

            var inputPath = result.StringResult.Trim();
            if (string.IsNullOrEmpty(inputPath)) inputPath = "output.ndjson";

            try
            {
                var importer = new PlantImporter();
                var (created, updated, failed) = importer.Import(doc, inputPath);
                ed.WriteMessage($"\nPMEF Import complete: created={created}, updated={updated}, failed={failed}");
            }
            catch (System.Exception ex)
            {
                ed.WriteMessage($"\nPMEF Import FAILED: {ex.Message}");
            }
        }

        public (int created, int updated, int failed) Import(Document doc, string inputPath)
        {
            int created = 0, updated = 0, failed = 0;
            var lines = File.ReadAllLines(inputPath);

            // Pass 1: collect identity mappings
            var pmefToHandle = new Dictionary<string, string>();
            foreach (var line in lines)
            {
                if (string.IsNullOrWhiteSpace(line) || !line.Contains("HasEquivalentIn")) continue;
                try
                {
                    var rel = JsonSerializer.Deserialize<JsonElement>(line);
                    if (rel.GetProperty("@type").GetString() == "pmef:HasEquivalentIn"
                        && rel.GetProperty("targetSystem").GetString() == "PLANT3D")
                    {
                        var sourceId = rel.GetProperty("sourceId").GetString() ?? "";
                        var handle   = rel.GetProperty("targetSystemId").GetString() ?? "";
                        pmefToHandle[sourceId] = handle;
                    }
                }
                catch { }
            }

            // Pass 2: process equipment objects
            var db = doc.Database;
            using (var tr = db.TransactionManager.StartTransaction())
            {
                foreach (var line in lines)
                {
                    if (string.IsNullOrWhiteSpace(line)) continue;
                    try
                    {
                        var obj = JsonSerializer.Deserialize<JsonElement>(line);
                        var type = obj.GetProperty("@type").GetString();
                        if (type?.StartsWith("pmef:") == true &&
                            IsEquipmentType(type))
                        {
                            var pmefId = obj.GetProperty("@id").GetString() ?? "";
                            pmefToHandle.TryGetValue(pmefId, out var handle);

                            if (handle != null && TryGetEquipment(db, tr, handle) is Equipment3d equip)
                            {
                                // Update existing
                                UpdateEquipmentProperties(equip, obj, tr);
                                updated++;
                            }
                            // Note: creating new equipment requires 3D geometry
                            // which is not contained in PMEF — skip creation.
                            // A full implementation would use a template equipment
                            // and apply properties.
                        }
                    }
                    catch { failed++; }
                }
                tr.Commit();
            }
            return (created, updated, failed);
        }

        private static bool IsEquipmentType(string type) =>
            type is "pmef:Pump" or "pmef:Vessel" or "pmef:HeatExchanger"
                 or "pmef:Compressor" or "pmef:Tank" or "pmef:Reactor"
                 or "pmef:Filter" or "pmef:Turbine" or "pmef:GenericEquipment";

        private static Equipment3d? TryGetEquipment(
            Database db, Transaction tr, string handle)
        {
            try
            {
                var h = new Handle(Convert.ToInt64(handle, 16));
                db.TryGetObjectId(h, out var id);
                return tr.GetObject(id, OpenMode.ForWrite) as Equipment3d;
            }
            catch { return null; }
        }

        private static void UpdateEquipmentProperties(
            Equipment3d equip, JsonElement obj, Transaction tr)
        {
            var props = equip.GetProperties();
            var eb = obj.GetProperty("equipmentBasic");

            // Update description
            if (eb.TryGetProperty("serviceDescription", out var desc) &&
                desc.ValueKind != JsonValueKind.Null)
                TrySetProp(props, "Description", desc.GetString());

            // Update design data from customAttributes
            if (obj.TryGetProperty("customAttributes", out var attrs))
            {
                if (attrs.TryGetProperty("designPressure_Pa", out var dp) &&
                    dp.ValueKind == JsonValueKind.Number)
                {
                    // Pa abs → psig: (Pa - 101325) / 6894.757
                    var psig = (dp.GetDouble() - 101325.0) / 6894.757;
                    TrySetProp(props, "DesignPressure", psig.ToString("F2"));
                }
                if (attrs.TryGetProperty("designTemperature_K", out var dt) &&
                    dt.ValueKind == JsonValueKind.Number)
                {
                    // K → °F: (K - 273.15) × 9/5 + 32
                    var f = (dt.GetDouble() - 273.15) * 1.8 + 32.0;
                    TrySetProp(props, "DesignTemperature", f.ToString("F1"));
                }
                if (attrs.TryGetProperty("material", out var mat) &&
                    mat.ValueKind == JsonValueKind.String)
                    TrySetProp(props, "Material", mat.GetString());
            }
            equip.Upgrade();
        }

        private static void TrySetProp(
            PnPPropertyCollection props, string name, string? value)
        {
            if (value == null) return;
            try { props[name].Value = value; }
            catch { }
        }
    }
}
