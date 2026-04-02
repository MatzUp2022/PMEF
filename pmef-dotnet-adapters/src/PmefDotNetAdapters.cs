// ============================================================================
// PmefDotNetAdapters.cs
// Combined C# source for three PMEF .NET add-ins:
//   1. RevitExporter          — Autodesk Revit API (Autodesk.Revit.DB)
//   2. AdvancedSteelExporter  — Autodesk Advanced Steel API (Autodesk.AdvancedSteeling)
//   3. NavisworksExporter     — Autodesk Navisworks API (Autodesk.Navisworks.Api)
//
// Each exporter produces a JSON file consumed by the corresponding Rust crate.
// Build as three separate .NET 8 projects — see individual csproj stubs below.
// ============================================================================

// ──────────────────────────────────────────────────────────────────────────────
// PART 1: RevitExporter.cs
// Revit Add-in: Tools → External Tools → PMEF Export
// References: RevitAPI.dll, RevitAPIUI.dll (Revit SDK)
// ──────────────────────────────────────────────────────────────────────────────

#region Revit

using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.Json;
using System.Text.Json.Serialization;
using Autodesk.Revit.Attributes;
using Autodesk.Revit.DB;
using Autodesk.Revit.DB.Mechanical;
using Autodesk.Revit.DB.Plumbing;
using Autodesk.Revit.DB.Structure;
using Autodesk.Revit.UI;

namespace PmefRevit
{
    // ── JSON schema records ───────────────────────────────────────────────────

    record RevitBboxRec(double[] Min, double[] Max);
    record RevitLevelRec(long ElementId, string Name, double ElevationMm, bool IsBuildingStory);
    record RevitGridRec(long ElementId, string Name, double[] Start, double[] End);
    record RevitRoomRec(long ElementId, string Name, string? Number, string? LevelName, double? AreaM2, double? VolumeM3);

    record RevitPipeSegmentRec(
        long ElementId, string UniqueId, string SystemType, string? SystemName,
        double DiameterMm, double? OutsideDiameterMm, double? WallThicknessMm,
        string? Material, string SegmentType,
        double[] StartPoint, double[] EndPoint, double LengthMm,
        string? LevelName, double? PressurePa, double? TemperatureK,
        double? FlowM3h, string? InsulationType, string? Comments, string? Mark,
        Dictionary<string, object?> Parameters
    );

    record RevitPipeFittingRec(
        long ElementId, string UniqueId, string FamilyName, string TypeName,
        string? SystemName, string PartType, double DiameterMm,
        double? OutletDiameterMm, double? AngleDeg,
        double[] Position, string? Material, string? LevelName, string? Mark
    );

    record RevitPipeAccessoryRec(
        long ElementId, string UniqueId, string FamilyName, string TypeName,
        string? SystemName, double DiameterMm, double[] Position,
        string? Mark, string? Comments, Dictionary<string, object?> Parameters
    );

    record RevitMechanicalEquipmentRec(
        long ElementId, string UniqueId, string FamilyName, string TypeName,
        string? Mark, string? OmniClass, double[] Position, double RotationDeg,
        string? LevelName, RevitBboxRec? BoundingBoxMm,
        double? DesignFlowM3h, double? PowerW, string? Comments,
        Dictionary<string, object?> Parameters
    );

    record RevitDuctSegmentRec(
        long ElementId, string UniqueId, string? SystemName,
        double WidthMm, double HeightMm,
        double[] StartPoint, double[] EndPoint, double LengthMm, string? LevelName
    );

    record RevitCableTrayRec(
        long ElementId, string UniqueId, string? SystemName,
        double WidthMm, double HeightMm,
        double[] StartPoint, double[] EndPoint, double LengthMm, string? LevelName
    );

    record RevitStructuralColumnRec(
        long ElementId, string UniqueId, string FamilyName, string TypeName,
        string? Mark, string? Material, double[] BasePoint, double[] TopPoint,
        double LengthMm, string? LevelName, RevitBboxRec? BoundingBoxMm
    );

    record RevitStructuralFramingRec(
        long ElementId, string UniqueId, string FamilyName, string TypeName,
        string? Mark, string? Material, string StructuralUsage,
        double[] StartPoint, double[] EndPoint, double LengthMm,
        double RotationDeg, string? LevelName, RevitBboxRec? BoundingBoxMm
    );

    record RevitExportSummaryRec(
        int PipeSegmentCount, int FittingCount,
        int EquipmentCount, int StructuralCount, int DuctCount
    );

    record RevitExportRec(
        string SchemaVersion, string RevitVersion, string ExportedAt,
        string ProjectName, string? ProjectNumber, string? BuildingName,
        string LengthUnit,
        List<RevitLevelRec> Levels,
        List<RevitGridRec> Grids,
        List<RevitPipeSegmentRec> PipeSegments,
        List<RevitPipeFittingRec> PipeFittings,
        List<RevitPipeAccessoryRec> PipeAccessories,
        List<RevitMechanicalEquipmentRec> MechanicalEquipment,
        List<RevitDuctSegmentRec> DuctSegments,
        List<RevitCableTrayRec> CableTrays,
        List<RevitStructuralColumnRec> StructuralColumns,
        List<RevitStructuralFramingRec> StructuralFraming,
        List<RevitRoomRec> Rooms,
        RevitExportSummaryRec Summary
    );

    // ── Exporter ─────────────────────────────────────────────────────────────

    [Transaction(TransactionMode.ReadOnly)]
    [Regeneration(RegenerationOption.Manual)]
    public class RevitExportCommand : IExternalCommand
    {
        public Result Execute(ExternalCommandData data, ref string message, ElementSet elements)
        {
            var doc  = data.Application.ActiveUIDocument.Document;
            var dlg  = new Microsoft.Win32.SaveFileDialog
            {
                Title = "PMEF — Export Revit Model",
                Filter = "JSON (*.json)|*.json",
                FileName = "revit-export.json"
            };
            if (dlg.ShowDialog() != true) return Result.Cancelled;

            try
            {
                var exporter = new RevitExporter(doc);
                exporter.Export(dlg.FileName);
                TaskDialog.Show("PMEF", $"Export complete:\n{dlg.FileName}");
                return Result.Succeeded;
            }
            catch (Exception ex)
            {
                message = ex.Message;
                return Result.Failed;
            }
        }
    }

    public class RevitExporter
    {
        private readonly Document _doc;
        // Revit internal unit = feet; 1 foot = 304.8 mm
        private const double FT_TO_MM = 304.8;

        public RevitExporter(Document doc) { _doc = doc; }

        public void Export(string outputPath)
        {
            var t0 = DateTime.UtcNow;
            Console.WriteLine("PMEF/Revit: collecting elements...");

            var root = new RevitExportRec(
                SchemaVersion:       "1.0",
                RevitVersion:        _doc.Application.VersionName,
                ExportedAt:          t0.ToString("O"),
                ProjectName:         _doc.ProjectInformation?.Name ?? "Unknown",
                ProjectNumber:       _doc.ProjectInformation?.Number,
                BuildingName:        _doc.ProjectInformation?.BuildingName,
                LengthUnit:          "FEET_INTERNAL_MM_OUTPUT",
                Levels:              CollectLevels(),
                Grids:               CollectGrids(),
                PipeSegments:        CollectPipes(),
                PipeFittings:        CollectPipeFittings(),
                PipeAccessories:     CollectPipeAccessories(),
                MechanicalEquipment: CollectMechanicalEquipment(),
                DuctSegments:        CollectDucts(),
                CableTrays:          CollectCableTrays(),
                StructuralColumns:   CollectStructuralColumns(),
                StructuralFraming:   CollectStructuralFraming(),
                Rooms:               CollectRooms(),
                Summary: new RevitExportSummaryRec(0,0,0,0,0) // filled below
            );

            // Re-create with counts
            var summary = new RevitExportSummaryRec(
                root.PipeSegments.Count, root.PipeFittings.Count,
                root.MechanicalEquipment.Count, root.StructuralFraming.Count + root.StructuralColumns.Count,
                root.DuctSegments.Count);
            root = root with { Summary = summary };

            var opts = new JsonSerializerOptions {
                WriteIndented = true,
                PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
                DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull
            };
            File.WriteAllText(outputPath, JsonSerializer.Serialize(root, opts));
            Console.WriteLine($"PMEF/Revit: {root.PipeSegments.Count} pipes, " +
                              $"{root.MechanicalEquipment.Count} equip, " +
                              $"{root.StructuralFraming.Count} framing → {outputPath}");
        }

        // ── Levels ────────────────────────────────────────────────────────────

        List<RevitLevelRec> CollectLevels()
        {
            return new FilteredElementCollector(_doc)
                .OfClass(typeof(Level))
                .Cast<Level>()
                .Select(l => new RevitLevelRec(
                    l.Id.IntegerValue, l.Name,
                    l.Elevation * FT_TO_MM, l.IsBuildingStory))
                .OrderBy(l => l.ElevationMm)
                .ToList();
        }

        List<RevitGridRec> CollectGrids()
        {
            return new FilteredElementCollector(_doc)
                .OfClass(typeof(Grid))
                .Cast<Grid>()
                .Select(g => {
                    var curve = g.Curve;
                    return new RevitGridRec(
                        g.Id.IntegerValue, g.Name,
                        PtMm(curve.GetEndPoint(0)),
                        PtMm(curve.GetEndPoint(1)));
                }).ToList();
        }

        // ── Pipes ─────────────────────────────────────────────────────────────

        List<RevitPipeSegmentRec> CollectPipes()
        {
            return new FilteredElementCollector(_doc)
                .OfClass(typeof(Pipe))
                .Cast<Pipe>()
                .Select(p => {
                    var loc = (p.Location as LocationCurve)?.Curve;
                    var sp = loc?.GetEndPoint(0) ?? XYZ.Zero;
                    var ep = loc?.GetEndPoint(1) ?? XYZ.Zero;
                    return new RevitPipeSegmentRec(
                        p.Id.IntegerValue,
                        p.UniqueId,
                        GetPipingSystemType(p),
                        p.get_Parameter(BuiltInParameter.RBS_PIPING_SYSTEM_TYPE_PARAM)
                            ?.AsValueString(),
                        GetDbl(p, BuiltInParameter.RBS_PIPE_DIAMETER_PARAM) * FT_TO_MM,
                        GetDbl(p, BuiltInParameter.RBS_PIPE_OUTER_DIAMETER) is double od && od > 0
                            ? od * FT_TO_MM : null,
                        GetDbl(p, BuiltInParameter.RBS_PIPE_WALL_THICKNESS) is double wt && wt > 0
                            ? wt * FT_TO_MM : null,
                        p.get_Parameter(BuiltInParameter.ELEM_MATERIAL_PARAM_MT)?.AsValueString(),
                        p.PipeType?.Name ?? "Standard",
                        PtMm(sp), PtMm(ep),
                        (loc?.Length ?? 0) * FT_TO_MM,
                        GetLevelName(p),
                        GetDbl(p, BuiltInParameter.RBS_PIPE_PRESSUREDROP_PARAM) is double pr && pr > 0
                            ? pr * 6894.757 : null,
                        null, null, null,
                        p.get_Parameter(BuiltInParameter.ALL_MODEL_MARK)?.AsString(),
                        p.get_Parameter(BuiltInParameter.ALL_MODEL_MARK)?.AsString(),
                        new Dictionary<string, object?>()
                    );
                }).ToList();
        }

        // ── Pipe fittings ─────────────────────────────────────────────────────

        List<RevitPipeFittingRec> CollectPipeFittings()
        {
            return new FilteredElementCollector(_doc)
                .OfCategory(BuiltInCategory.OST_PipeFitting)
                .WhereElementIsNotElementType()
                .Cast<FamilyInstance>()
                .Select(fi => {
                    var mepConn = fi.MEPModel as MechanicalFitting;
                    var partType = mepConn?.PartType.ToString() ?? "Other";
                    var pos = (fi.Location as LocationPoint)?.Point ?? XYZ.Zero;
                    return new RevitPipeFittingRec(
                        fi.Id.IntegerValue, fi.UniqueId,
                        fi.Symbol.FamilyName, fi.Symbol.Name,
                        fi.get_Parameter(BuiltInParameter.RBS_PIPING_SYSTEM_TYPE_PARAM)?.AsValueString(),
                        partType,
                        GetDbl(fi, BuiltInParameter.RBS_PIPE_DIAMETER_PARAM) * FT_TO_MM,
                        null, null,
                        PtMm(pos), null,
                        GetLevelName(fi),
                        fi.get_Parameter(BuiltInParameter.ALL_MODEL_MARK)?.AsString()
                    );
                }).ToList();
        }

        // ── Pipe accessories ──────────────────────────────────────────────────

        List<RevitPipeAccessoryRec> CollectPipeAccessories()
        {
            return new FilteredElementCollector(_doc)
                .OfCategory(BuiltInCategory.OST_PipeAccessory)
                .WhereElementIsNotElementType()
                .Cast<FamilyInstance>()
                .Select(fi => {
                    var pos = (fi.Location as LocationPoint)?.Point ?? XYZ.Zero;
                    return new RevitPipeAccessoryRec(
                        fi.Id.IntegerValue, fi.UniqueId,
                        fi.Symbol.FamilyName, fi.Symbol.Name,
                        fi.get_Parameter(BuiltInParameter.RBS_PIPING_SYSTEM_TYPE_PARAM)?.AsValueString(),
                        GetDbl(fi, BuiltInParameter.RBS_PIPE_DIAMETER_PARAM) * FT_TO_MM,
                        PtMm(pos),
                        fi.get_Parameter(BuiltInParameter.ALL_MODEL_MARK)?.AsString(),
                        fi.get_Parameter(BuiltInParameter.ALL_MODEL_INSTANCE_COMMENTS_PARAM)?.AsString(),
                        new Dictionary<string, object?>()
                    );
                }).ToList();
        }

        // ── Mechanical equipment ──────────────────────────────────────────────

        List<RevitMechanicalEquipmentRec> CollectMechanicalEquipment()
        {
            return new FilteredElementCollector(_doc)
                .OfCategory(BuiltInCategory.OST_MechanicalEquipment)
                .WhereElementIsNotElementType()
                .Cast<FamilyInstance>()
                .Select(fi => {
                    var pos = (fi.Location as LocationPoint)?.Point ?? XYZ.Zero;
                    var rot = (fi.Location as LocationPoint)?.Rotation ?? 0.0;
                    var bb  = fi.get_BoundingBox(null);
                    RevitBboxRec? bbox = null;
                    if (bb != null) bbox = new RevitBboxRec(
                        new[]{ bb.Min.X*FT_TO_MM, bb.Min.Y*FT_TO_MM, bb.Min.Z*FT_TO_MM },
                        new[]{ bb.Max.X*FT_TO_MM, bb.Max.Y*FT_TO_MM, bb.Max.Z*FT_TO_MM });
                    return new RevitMechanicalEquipmentRec(
                        fi.Id.IntegerValue, fi.UniqueId,
                        fi.Symbol.FamilyName, fi.Symbol.Name,
                        fi.get_Parameter(BuiltInParameter.ALL_MODEL_MARK)?.AsString(),
                        fi.get_Parameter(BuiltInParameter.OMNICLASS_CODE)?.AsString(),
                        PtMm(pos), rot * (180.0/Math.PI),
                        GetLevelName(fi), bbox,
                        null, null,
                        fi.get_Parameter(BuiltInParameter.ALL_MODEL_INSTANCE_COMMENTS_PARAM)?.AsString(),
                        new Dictionary<string, object?>()
                    );
                }).ToList();
        }

        // ── Ducts ─────────────────────────────────────────────────────────────

        List<RevitDuctSegmentRec> CollectDucts()
        {
            return new FilteredElementCollector(_doc)
                .OfClass(typeof(Duct))
                .Cast<Duct>()
                .Select(d => {
                    var loc = (d.Location as LocationCurve)?.Curve;
                    return new RevitDuctSegmentRec(
                        d.Id.IntegerValue, d.UniqueId,
                        d.get_Parameter(BuiltInParameter.RBS_DUCT_SYSTEM_TYPE_PARAM)?.AsValueString(),
                        GetDbl(d, BuiltInParameter.RBS_CURVE_WIDTH_PARAM) * FT_TO_MM,
                        GetDbl(d, BuiltInParameter.RBS_CURVE_HEIGHT_PARAM) * FT_TO_MM,
                        PtMm(loc?.GetEndPoint(0) ?? XYZ.Zero),
                        PtMm(loc?.GetEndPoint(1) ?? XYZ.Zero),
                        (loc?.Length ?? 0) * FT_TO_MM,
                        GetLevelName(d)
                    );
                }).ToList();
        }

        // ── Cable trays ───────────────────────────────────────────────────────

        List<RevitCableTrayRec> CollectCableTrays()
        {
            return new FilteredElementCollector(_doc)
                .OfClass(typeof(Autodesk.Revit.DB.Electrical.CableTray))
                .Cast<Autodesk.Revit.DB.Electrical.CableTray>()
                .Select(ct => {
                    var loc = (ct.Location as LocationCurve)?.Curve;
                    return new RevitCableTrayRec(
                        ct.Id.IntegerValue, ct.UniqueId, null,
                        GetDbl(ct, BuiltInParameter.RBS_CABLETRAY_WIDTH_PARAM) * FT_TO_MM,
                        GetDbl(ct, BuiltInParameter.RBS_CABLETRAY_HEIGHT_PARAM) * FT_TO_MM,
                        PtMm(loc?.GetEndPoint(0) ?? XYZ.Zero),
                        PtMm(loc?.GetEndPoint(1) ?? XYZ.Zero),
                        (loc?.Length ?? 0) * FT_TO_MM,
                        GetLevelName(ct)
                    );
                }).ToList();
        }

        // ── Structural ────────────────────────────────────────────────────────

        List<RevitStructuralColumnRec> CollectStructuralColumns()
        {
            return new FilteredElementCollector(_doc)
                .OfCategory(BuiltInCategory.OST_StructuralColumns)
                .WhereElementIsNotElementType()
                .Cast<FamilyInstance>()
                .Select(col => {
                    var loc  = col.Location as LocationPoint;
                    var pos  = loc?.Point ?? XYZ.Zero;
                    var bb   = col.get_BoundingBox(null);
                    var bot  = bb != null ? PtMm(new XYZ(pos.X, pos.Y, bb.Min.Z)) : PtMm(pos);
                    var top  = bb != null ? PtMm(new XYZ(pos.X, pos.Y, bb.Max.Z)) : PtMm(pos);
                    var len  = bb != null ? (bb.Max.Z - bb.Min.Z) * FT_TO_MM : 0.0;
                    RevitBboxRec? bbox = null;
                    if (bb != null) bbox = new RevitBboxRec(
                        new[]{ bb.Min.X*FT_TO_MM, bb.Min.Y*FT_TO_MM, bb.Min.Z*FT_TO_MM },
                        new[]{ bb.Max.X*FT_TO_MM, bb.Max.Y*FT_TO_MM, bb.Max.Z*FT_TO_MM });
                    return new RevitStructuralColumnRec(
                        col.Id.IntegerValue, col.UniqueId,
                        col.Symbol.FamilyName, col.Symbol.Name,
                        col.get_Parameter(BuiltInParameter.ALL_MODEL_MARK)?.AsString(),
                        col.StructuralMaterialType.ToString(),
                        bot, top, len, GetLevelName(col), bbox
                    );
                }).ToList();
        }

        List<RevitStructuralFramingRec> CollectStructuralFraming()
        {
            return new FilteredElementCollector(_doc)
                .OfCategory(BuiltInCategory.OST_StructuralFraming)
                .WhereElementIsNotElementType()
                .Cast<FamilyInstance>()
                .Select(fr => {
                    var loc = (fr.Location as LocationCurve)?.Curve;
                    var sp  = loc?.GetEndPoint(0) ?? XYZ.Zero;
                    var ep  = loc?.GetEndPoint(1) ?? XYZ.Zero;
                    var bb  = fr.get_BoundingBox(null);
                    RevitBboxRec? bbox = null;
                    if (bb != null) bbox = new RevitBboxRec(
                        new[]{ bb.Min.X*FT_TO_MM, bb.Min.Y*FT_TO_MM, bb.Min.Z*FT_TO_MM },
                        new[]{ bb.Max.X*FT_TO_MM, bb.Max.Y*FT_TO_MM, bb.Max.Z*FT_TO_MM });
                    var usage = fr.get_Parameter(BuiltInParameter.INSTANCE_STRUCT_USAGE_TEXT_PARAM)
                        ?.AsString() ?? "Beam";
                    return new RevitStructuralFramingRec(
                        fr.Id.IntegerValue, fr.UniqueId,
                        fr.Symbol.FamilyName, fr.Symbol.Name,
                        fr.get_Parameter(BuiltInParameter.ALL_MODEL_MARK)?.AsString(),
                        fr.StructuralMaterialType.ToString(),
                        usage,
                        PtMm(sp), PtMm(ep),
                        (loc?.Length ?? 0) * FT_TO_MM,
                        fr.get_Parameter(BuiltInParameter.STRUCTURAL_BEND_DIR_ANGLE)
                            ?.AsDouble() * (180.0/Math.PI) ?? 0.0,
                        GetLevelName(fr), bbox
                    );
                }).ToList();
        }

        List<RevitRoomRec> CollectRooms()
        {
            return new FilteredElementCollector(_doc)
                .OfClass(typeof(Autodesk.Revit.DB.Architecture.Room))
                .Cast<Autodesk.Revit.DB.Architecture.Room>()
                .Select(r => new RevitRoomRec(
                    r.Id.IntegerValue, r.Name,
                    r.Number,
                    r.Level?.Name,
                    r.Area > 0 ? r.Area * 0.0929 : null,  // ft² → m²
                    r.Volume > 0 ? r.Volume * 0.0283 : null // ft³ → m³
                )).ToList();
        }

        // ── Helpers ───────────────────────────────────────────────────────────

        double[] PtMm(XYZ p) => new[]{ p.X*FT_TO_MM, p.Y*FT_TO_MM, p.Z*FT_TO_MM };

        double GetDbl(Element e, BuiltInParameter bip)
            => e.get_Parameter(bip)?.AsDouble() ?? 0.0;

        string? GetLevelName(Element e)
            => (e.get_Parameter(BuiltInParameter.FAMILY_LEVEL_PARAM)
                ?? e.get_Parameter(BuiltInParameter.LEVEL_PARAM))
               ?.AsValueString();

        string GetPipingSystemType(Pipe pipe)
        {
            var sys = pipe.MEPSystem as PipingSystem;
            return sys?.SystemType.ToString() ?? "ProcessPipe";
        }
    }
}
#endregion

// ──────────────────────────────────────────────────────────────────────────────
// PART 2: AdvancedSteelExporter.cs
// References: Autodesk.AdvancedSteeling.Core.dll (installed with Advanced Steel)
// ──────────────────────────────────────────────────────────────────────────────

#region AdvancedSteel

using Autodesk.AdvancedSteeling.Core;
using Autodesk.AdvancedSteeling.Core.SteelObjects;

namespace PmefAdvancedSteel
{
    record AdvSteelReleaseRec(bool Moment, bool Torsion);
    record AdvSteelBeamRec(
        string Handle, string MemberMark, string? AssemblyMark,
        string Section, string SectionStandard, string Grade,
        string MemberType,
        double[] StartPoint, double[] EndPoint, double LengthMm,
        double RollAngleDeg, double? MassKg, double? SurfaceAreaM2,
        string? Finish, string? FireProtection,
        Dictionary<string, object?> Udas,
        AdvSteelReleaseRec StartRelease, AdvSteelReleaseRec EndRelease
    );
    record AdvSteelPlateRec(
        string Handle, string MemberMark, string Grade,
        double ThicknessMm, double LengthMm, double WidthMm,
        double? MassKg, double[] Origin, double[] Normal
    );
    record AdvSteelBoltPatternRec(
        string Handle, string BoltStandard, double BoltDiameterMm,
        string BoltGrade, int BoltCount, string HoleType, bool Preloaded,
        double[] Centroid, List<string> ConnectedHandles
    );
    record AdvSteelWeldSeamRec(
        string Handle, string WeldType, double LegSizeMm,
        double LengthMm, string WeldingProcess, string? WeldNumber,
        List<string> ConnectedHandles
    );
    record AdvSteelAnchorPatternRec(
        string Handle, string AnchorStandard, double AnchorDiameterMm,
        string AnchorGrade, int AnchorCount, double EmbeddedLengthMm,
        double[] Centroid
    );
    record AdvSteelSummaryRec(int BeamCount, int PlateCount, int BoltPatternCount, int WeldSeamCount);
    record AdvSteelExportRec(
        string SchemaVersion, string AdvancedSteelVersion, string ExportedAt,
        string ModelName, string? DrawingNumber, string CoordinateUnit,
        List<AdvSteelBeamRec> Beams,
        List<AdvSteelPlateRec> Plates,
        List<AdvSteelBoltPatternRec> BoltPatterns,
        List<AdvSteelWeldSeamRec> WeldSeams,
        List<AdvSteelAnchorPatternRec> AnchorPatterns,
        AdvSteelSummaryRec Summary
    );

    public class AdvancedSteelExporter
    {
        public void Export(string outputPath)
        {
            var t0 = DateTime.UtcNow;
            var db = DatabaseManager.GetActiveDatabase();

            var beams   = new List<AdvSteelBeamRec>();
            var plates  = new List<AdvSteelPlateRec>();
            var bolts   = new List<AdvSteelBoltPatternRec>();
            var welds   = new List<AdvSteelWeldSeamRec>();
            var anchors = new List<AdvSteelAnchorPatternRec>();

            foreach (var obj in db.ModelObjects)
            {
                switch (obj)
                {
                    case StraightBeam sb:
                        beams.Add(MapBeam(sb)); break;

                    case BentBeam bb:
                        beams.Add(MapBeamBase(bb, "BRACE")); break;

                    case Plate pl:
                        plates.Add(MapPlate(pl)); break;

                    case BoltPattern bp:
                        bolts.Add(MapBoltPattern(bp)); break;

                    case FilletWeld fw:
                        welds.Add(MapWeldSeam(fw)); break;

                    case AnchorPattern ap:
                        anchors.Add(MapAnchorPattern(ap)); break;
                }
            }

            var root = new AdvSteelExportRec(
                "1.0", GetVersion(), t0.ToString("O"),
                db.FileName ?? "Model", null, "MM",
                beams, plates, bolts, welds, anchors,
                new AdvSteelSummaryRec(beams.Count, plates.Count, bolts.Count, welds.Count)
            );

            var opts = new JsonSerializerOptions {
                WriteIndented = true,
                PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
                DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull
            };
            File.WriteAllText(outputPath, JsonSerializer.Serialize(root, opts));
            Console.WriteLine($"Advanced Steel export: {beams.Count} beams, {bolts.Count} bolt patterns → {outputPath}");
        }

        AdvSteelBeamRec MapBeam(StraightBeam sb)
        {
            var sp = sb.Curve.GetStartPoint();
            var ep = sb.Curve.GetEndPoint();
            var len = sb.PhysicalLength * 1000.0; // m → mm

            var memberType = sb.BeamType switch
            {
                BeamType.Beam   => "Beam",
                BeamType.Column => "Column",
                BeamType.Brace  => "Brace",
                _               => "Other"
            };

            return new AdvSteelBeamRec(
                Handle:          sb.Handle.ToString(),
                MemberMark:      sb.PartMark ?? sb.Handle.ToString(),
                AssemblyMark:    sb.AssemblyMark,
                Section:         sb.SectionName ?? "",
                SectionStandard: sb.SectionStandard ?? "EN",
                Grade:           sb.Grade ?? "S355JR",
                MemberType:      memberType,
                StartPoint:      new[]{ sp.X*1000, sp.Y*1000, sp.Z*1000 },
                EndPoint:        new[]{ ep.X*1000, ep.Y*1000, ep.Z*1000 },
                LengthMm:        len,
                RollAngleDeg:    sb.Angle * (180.0 / Math.PI),
                MassKg:          sb.Weight,
                SurfaceAreaM2:   sb.PaintArea,
                Finish:          sb.Coating,
                FireProtection:  null,
                Udas:            ReadUdas(sb),
                StartRelease:    new AdvSteelReleaseRec(sb.StartRelease?.HasMomentRelease ?? false, false),
                EndRelease:      new AdvSteelReleaseRec(sb.EndRelease?.HasMomentRelease ?? false, false)
            );
        }

        AdvSteelBeamRec MapBeamBase(BentBeam bb, string memberType)
        {
            var sp = bb.Curve.GetStartPoint();
            var ep = bb.Curve.GetEndPoint();
            return new AdvSteelBeamRec(
                bb.Handle.ToString(), bb.PartMark ?? "", null,
                bb.SectionName ?? "", bb.SectionStandard ?? "EN", bb.Grade ?? "S355JR",
                memberType,
                new[]{ sp.X*1000, sp.Y*1000, sp.Z*1000 },
                new[]{ ep.X*1000, ep.Y*1000, ep.Z*1000 },
                bb.PhysicalLength * 1000, 0.0, bb.Weight, null, null, null,
                new Dictionary<string, object?>(),
                new AdvSteelReleaseRec(false, false),
                new AdvSteelReleaseRec(false, false)
            );
        }

        AdvSteelPlateRec MapPlate(Plate pl) => new AdvSteelPlateRec(
            pl.Handle.ToString(), pl.PartMark ?? "", pl.Grade ?? "S355JR",
            pl.Thickness * 1000, pl.Length * 1000, pl.Width * 1000,
            pl.Weight, new[]{ pl.Origin.X*1000, pl.Origin.Y*1000, pl.Origin.Z*1000 },
            new[]{ pl.Normal.X, pl.Normal.Y, pl.Normal.Z }
        );

        AdvSteelBoltPatternRec MapBoltPattern(BoltPattern bp) => new AdvSteelBoltPatternRec(
            bp.Handle.ToString(), bp.BoltStandard ?? "ISO 4014",
            bp.BoltDiameter * 1000, bp.BoltGrade ?? "8.8",
            bp.BoltCount, bp.HoleType ?? "CLEARANCE", bp.IsPreloaded,
            new[]{ bp.Center.X*1000, bp.Center.Y*1000, bp.Center.Z*1000 },
            bp.ConnectedObjects.Select(o => o.Handle.ToString()).ToList()
        );

        AdvSteelWeldSeamRec MapWeldSeam(FilletWeld fw) => new AdvSteelWeldSeamRec(
            fw.Handle.ToString(), "FILLET",
            fw.Size * 1000, fw.Length * 1000, "SMAW", fw.WeldNumber,
            fw.ConnectedObjects.Select(o => o.Handle.ToString()).ToList()
        );

        AdvSteelAnchorPatternRec MapAnchorPattern(AnchorPattern ap) => new AdvSteelAnchorPatternRec(
            ap.Handle.ToString(), ap.AnchorStandard ?? "ISO",
            ap.AnchorDiameter * 1000, ap.AnchorGrade ?? "4.6",
            ap.AnchorCount, ap.EmbedmentLength * 1000,
            new[]{ ap.Center.X*1000, ap.Center.Y*1000, ap.Center.Z*1000 }
        );

        Dictionary<string, object?> ReadUdas(SteelObject obj)
        {
            var d = new Dictionary<string, object?>();
            string[] names = { "ERECTION_SEQUENCE","FIRE_ZONE","PAINT_SYSTEM",
                               "INSPECTION_CLASS","SHOP_MARK","HOLD_POINT" };
            foreach (var n in names)
            {
                try
                {
                    var val = obj.GetUserDefinedAttribute(n);
                    if (val != null) d[n] = val.ToString();
                }
                catch { }
            }
            return d;
        }

        string GetVersion()
        {
            try { return ApplicationInfo.Version; }
            catch { return "Advanced Steel 2024"; }
        }
    }
}
#endregion

// ──────────────────────────────────────────────────────────────────────────────
// PART 3: NavisworksExporter.cs
// References: Autodesk.Navisworks.Api.dll (Navisworks SDK)
// ──────────────────────────────────────────────────────────────────────────────

#region Navisworks

using Autodesk.Navisworks.Api;
using Autodesk.Navisworks.Api.Clash;
using Autodesk.Navisworks.Api.Plugins;

namespace PmefNavisworks
{
    record NavisBboxRec(double[] Min, double[] Max);
    record NavisSourceFileRec(string FileName, string FilePath, string SourceSystem, string? AppendedAt, int ItemCount);
    record NavisItemRec(
        string InstanceGuid, string DisplayName, string SourceFile, string Category,
        string? SourceObjectId, NavisBboxRec? BoundingBox,
        Dictionary<string, Dictionary<string, object?>> Properties
    );
    record NavisClashResultRec(
        string ClashId, string ClashName, string ClashType, string Status,
        string ItemAGuid, string ItemBGuid,
        string ItemAName, string ItemBName,
        string ItemASource, string ItemBSource,
        double[] ClashPoint, double DistanceMm,
        string? AssignedTo, string? Description,
        string? FoundDate, string? ResolvedDate
    );
    record NavisClashTestRec(
        string TestName, string SelectionA, string SelectionB,
        double ToleranceMm, List<NavisClashResultRec> Results,
        Dictionary<string, int> StatusCounts
    );
    record NavisViewpointRec(
        string Name, double[] CameraPosition, double[] LookAt,
        string? AssociatedClashId, string? Comment
    );
    record NavisSummaryRec(int ItemCount, int ClashCount, int HardClashCount, int ClearanceClashCount);
    record NavisExportRec(
        string SchemaVersion, string NavisworksVersion, string ExportedAt,
        string ModelName, string FileName, string Units,
        List<NavisItemRec> ModelItems,
        List<NavisClashTestRec> ClashTests,
        List<NavisViewpointRec> Viewpoints,
        List<NavisSourceFileRec> SourceFiles,
        NavisSummaryRec Summary
    );

    [Plugin("PmefNavisworksExporter", "PMEF", DisplayName = "PMEF Export")]
    [AddInPlugin(AddInLocation.Export)]
    public class NavisworksExporter : AddInPlugin
    {
        // Scale factor: Navisworks uses metres internally
        private const double M_TO_MM = 1000.0;

        public override int Execute(params string[] parameters)
        {
            var doc = Application.ActiveDocument;
            if (doc == null) return 0;

            var outputPath = parameters.Length > 0
                ? parameters[0]
                : Path.Combine(
                    Environment.GetFolderPath(Environment.SpecialFolder.Desktop),
                    "navisworks-export.json");

            var t0 = DateTime.UtcNow;

            var items      = CollectModelItems(doc);
            var clashTests = CollectClashTests(doc);
            var viewpoints = CollectViewpoints(doc);
            var sources    = CollectSourceFiles(doc);

            var hardCount  = clashTests.Sum(t => t.Results.Count(r => r.ClashType == "HardClash"));
            var clearCount = clashTests.Sum(t => t.Results.Count(r => r.ClashType == "Clearance"));
            var allClash   = clashTests.Sum(t => t.Results.Count);

            var root = new NavisExportRec(
                "1.0", Application.Version, t0.ToString("O"),
                doc.Title, doc.FileName, "METERS_INTERNAL_MM_OUTPUT",
                items, clashTests, viewpoints, sources,
                new NavisSummaryRec(items.Count, allClash, hardCount, clearCount)
            );

            var opts = new JsonSerializerOptions {
                WriteIndented = true,
                PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
                DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull
            };
            File.WriteAllText(outputPath, JsonSerializer.Serialize(root, opts));
            Console.WriteLine(
                $"PMEF/Navisworks: {items.Count} items, {allClash} clashes → {outputPath}");
            return 0;
        }

        // ── Model items ───────────────────────────────────────────────────────

        List<NavisItemRec> CollectModelItems(Document doc)
        {
            var result = new List<NavisItemRec>();
            CollectItemsRecursive(doc.Models.RootItems, result);
            return result;
        }

        void CollectItemsRecursive(ModelItemCollection items, List<NavisItemRec> result)
        {
            foreach (ModelItem item in items)
            {
                if (item.IsLeaf || item.HasGeometry)
                {
                    var guid = item.InstanceGuid.ToString();
                    var props = ExtractProperties(item);
                    var bb    = TryGetBbox(item);

                    result.Add(new NavisItemRec(
                        guid, item.DisplayName ?? "",
                        item.Ancestors?.FirstOrDefault(a => a.Model != null)?.Model?.RootItem?.DisplayName ?? "",
                        GetCategory(item), null, bb, props
                    ));
                }
                if (!item.IsLeaf)
                    CollectItemsRecursive(item.Children, result);
            }
        }

        string GetCategory(ModelItem item)
        {
            // Try to find the category from Revit/Plant3D properties
            var catProp = item.PropertyCategories
                .FindPropertyByDisplayName("Category")?.Value?.ToDisplayString()
                ?? item.ClassDisplayName ?? "Unknown";
            return catProp;
        }

        NavisBboxRec? TryGetBbox(ModelItem item)
        {
            try
            {
                var bb = item.FindFirstGeometryBoundingBox();
                if (bb == null || !bb.IsValid) return null;
                return new NavisBboxRec(
                    new[]{ bb.Min.X*M_TO_MM, bb.Min.Y*M_TO_MM, bb.Min.Z*M_TO_MM },
                    new[]{ bb.Max.X*M_TO_MM, bb.Max.Y*M_TO_MM, bb.Max.Z*M_TO_MM }
                );
            }
            catch { return null; }
        }

        Dictionary<string, Dictionary<string, object?>> ExtractProperties(ModelItem item)
        {
            var result = new Dictionary<string, Dictionary<string, object?>>();
            foreach (var cat in item.PropertyCategories)
            {
                var catDict = new Dictionary<string, object?>();
                foreach (var prop in cat.Properties)
                    catDict[prop.DisplayName] = prop.Value?.ToDisplayString();
                result[cat.DisplayName] = catDict;
            }
            return result;
        }

        // ── Clash tests ───────────────────────────────────────────────────────

        List<NavisClashTestRec> CollectClashTests(Document doc)
        {
            var result = new List<NavisClashTestRec>();
            var clashPlugin = doc.GetClash();
            if (clashPlugin == null) return result;

            foreach (ClashTest test in clashPlugin.TestsData.Tests)
            {
                var results = new List<NavisClashResultRec>();
                foreach (ClashResultGroup grp in test.FindAll<ClashResultGroup>())
                {
                    foreach (ClashResult clash in grp.Children.OfType<ClashResult>())
                    {
                        var ptM = clash.Center;
                        var ptMm = new[]{ ptM.X*M_TO_MM, ptM.Y*M_TO_MM, ptM.Z*M_TO_MM };

                        var itemA = clash.CompositeItem1?.FirstOrDefault();
                        var itemB = clash.CompositeItem2?.FirstOrDefault();

                        results.Add(new NavisClashResultRec(
                            clash.Guid.ToString(),
                            clash.DisplayName ?? clash.Guid.ToString(),
                            clash.ClashType.ToString(),
                            clash.Status.ToString(),
                            itemA?.InstanceGuid.ToString() ?? "",
                            itemB?.InstanceGuid.ToString() ?? "",
                            itemA?.DisplayName ?? "", itemB?.DisplayName ?? "",
                            itemA?.AncestorModels?.FirstOrDefault()?.RootItem?.DisplayName ?? "",
                            itemB?.AncestorModels?.FirstOrDefault()?.RootItem?.DisplayName ?? "",
                            ptMm,
                            clash.Distance * M_TO_MM,
                            clash.AssignedTo, clash.Description,
                            clash.FoundDate?.ToString("O"),
                            clash.ApprovedDate?.ToString("O")
                        ));
                    }
                }

                var statusCounts = results
                    .GroupBy(r => r.Status)
                    .ToDictionary(g => g.Key, g => g.Count());

                result.Add(new NavisClashTestRec(
                    test.DisplayName,
                    test.SelectionA?.DisplayName ?? "A",
                    test.SelectionB?.DisplayName ?? "B",
                    test.Tolerance * M_TO_MM,
                    results, statusCounts
                ));
            }
            return result;
        }

        // ── Viewpoints ────────────────────────────────────────────────────────

        List<NavisViewpointRec> CollectViewpoints(Document doc)
        {
            var result = new List<NavisViewpointRec>();
            foreach (SavedViewpoint svp in doc.SavedViewpoints.RootItem.Children)
            {
                var cam = svp.Viewpoint?.Camera;
                if (cam == null) continue;
                result.Add(new NavisViewpointRec(
                    svp.DisplayName,
                    new[]{ cam.Position.X*M_TO_MM, cam.Position.Y*M_TO_MM, cam.Position.Z*M_TO_MM },
                    new[]{ cam.LookAt.X*M_TO_MM,   cam.LookAt.Y*M_TO_MM,   cam.LookAt.Z*M_TO_MM },
                    null, svp.Comment
                ));
            }
            return result;
        }

        // ── Source files ──────────────────────────────────────────────────────

        List<NavisSourceFileRec> CollectSourceFiles(Document doc)
        {
            return doc.Models.Select(m => new NavisSourceFileRec(
                Path.GetFileName(m.RootItem?.DisplayName ?? ""),
                m.RootItem?.DisplayName ?? "",
                InferSourceSystem(m.RootItem?.DisplayName ?? ""),
                null,
                m.RootItem?.Descendants.Count() ?? 0
            )).ToList();
        }

        static string InferSourceSystem(string name)
        {
            var n = name.ToLower();
            if (n.EndsWith(".rvt")) return "REVIT";
            if (n.Contains("plant3d") || n.Contains("pcf")) return "PLANT3D";
            if (n.Contains("e3d") || n.EndsWith(".rvm")) return "AVEVA_E3D";
            if (n.EndsWith(".ifc")) return "IFC";
            if (n.Contains("tekla")) return "TEKLA_STRUCTURES";
            return "AUTOCAD";
        }
    }
}
#endregion
