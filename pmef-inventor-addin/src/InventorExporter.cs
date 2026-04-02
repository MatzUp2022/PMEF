// InventorExporter.cs
// Autodesk Inventor COM Add-in — PMEF JSON export
//
// Reads assembly hierarchy, iProperties, parameters, nozzle work points,
// Frame Generator members, and Tube & Pipe runs from an Inventor model
// and writes a structured JSON export consumed by pmef-adapter-inventor (Rust).
//
// Build:  .NET 8, x64
//         References: Autodesk.Inventor.Interop.dll
//                     System.Text.Json (BCL)
// Deploy: Tools → Customize → Add-Ins → register PmefInventor.dll
//         Or: Tools → iLogic → External Rule Directories → run as rule
//
// SDK reference: Autodesk Inventor API Reference 2024
//   https://help.autodesk.com/view/INVNTOR/2024/ENU/?guid=GUID-API_REFERENCE

using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text.Json;
using System.Text.Json.Serialization;

using Autodesk.Inventor;

namespace PmefInventor
{
    // ──────────────────────────────────────────────────────────────────────────
    // JSON export schema (mirrors export_schema.rs)
    // ──────────────────────────────────────────────────────────────────────────

    public record InvPointRec(double X, double Y, double Z);
    public record InvBboxRec(double XMin, double XMax, double YMin, double YMax, double ZMin, double ZMax);
    public record InvTransformRec(double[][] Rotation, double[] Translation);

    public record InvPropertiesRec(
        string? PartNumber, string? Description, string? Revision,
        string? Designer, string? Material,
        double? MassKg, double? SurfaceAreaM2, string? Vendor,
        string? Project, double? Cost, string? StockNumber
    );

    public record InvWorkPointRec(
        string Name, InvPointRec Position,
        double[] XAxis, double[] ZAxis,
        Dictionary<string, object?> Parameters
    );

    public record InvIPartInfoRec(
        string FactoryFile, int RowNumber, string MemberName,
        Dictionary<string, string> TableValues
    );

    public record InventorAssemblyRec(
        string OccurrencePath, string Name, string IamFile,
        string? VaultNumber,
        InvPropertiesRec Iproperties,
        Dictionary<string, object?> Parameters,
        string? PmefTag, string? PmefClass,
        double? PmefDesignPressureBarg, double? PmefDesignTempDegc,
        string? PmefDesignCode,
        InvBboxRec? BoundingBox,
        InvTransformRec Transform,
        string? StepFile,
        List<InvWorkPointRec> NozzleWorkPoints,
        bool IsIAssembly,
        InvIPartInfoRec? IpartInfo,
        string? ParentPath,
        List<string> ChildPaths
    );

    public record InventorPartRec(
        string OccurrencePath, string Name, string IptFile,
        string? VaultNumber, InvPropertiesRec Iproperties,
        Dictionary<string, object?> Parameters,
        InvBboxRec? BoundingBox, InvTransformRec Transform,
        string? StepFile, string? ParentPath
    );

    public record InventorFrameMemberRec(
        string OccurrencePath, string Name, string MemberType,
        string SectionName, string SectionStandard,
        double LengthMm, InvPointRec StartPoint, InvPointRec EndPoint,
        double RollAngleDeg, string Material,
        double? MassKg, string? VaultNumber,
        Dictionary<string, object?> Parameters
    );

    public record InventorTubeRunRec(
        string RunId, string RunName, double NominalDiameterIn,
        string? PipeSpec, double OutsideDiameterMm, double WallThicknessMm,
        InvPointRec StartPoint, InvPointRec EndPoint,
        List<InvPointRec> RoutePoints, string? Material
    );

    public record InventorNozzlePointRec(
        string WorkPointName, string NozzleMark,
        string ParentOccurrencePath, InvPointRec Position,
        double[] Direction, double? NominalDiameterMm,
        string? FlangeRating, string? FacingType, string? Service
    );

    public record InventorExportSummaryRec(
        int AssemblyCount, int PartCount, int FrameMemberCount, int TubeRunCount
    );

    public record InventorExportRec(
        string SchemaVersion, string InventorVersion, string ExportedAt,
        string AssemblyName, string AssemblyFile, string? VaultNumber,
        string CoordinateUnit,
        List<InventorAssemblyRec> Assemblies,
        List<InventorPartRec> Parts,
        List<InventorFrameMemberRec> FrameMembers,
        List<InventorTubeRunRec> TubeRuns,
        List<InventorNozzlePointRec> NozzlePoints,
        InventorExportSummaryRec Summary
    );

    // ──────────────────────────────────────────────────────────────────────────
    // Exporter
    // ──────────────────────────────────────────────────────────────────────────

    public class InventorExporter
    {
        private readonly Application _inv;
        private readonly List<InventorAssemblyRec> _assemblies = new();
        private readonly List<InventorPartRec>     _parts      = new();
        private readonly List<InventorFrameMemberRec> _frames  = new();
        private readonly List<InventorTubeRunRec>  _tubeRuns   = new();
        private readonly List<InventorNozzlePointRec> _nozzles = new();

        public InventorExporter(Application inv) { _inv = inv; }

        public void Export(string outputPath)
        {
            var doc = _inv.ActiveDocument as AssemblyDocument
                ?? throw new InvalidOperationException(
                    "No active assembly document. Open an .iam file first.");

            Console.WriteLine($"PMEF Export: starting for {doc.DisplayName}");
            var t0 = DateTime.UtcNow;

            var rootDef = (AssemblyComponentDefinition)doc.ComponentDefinition;
            WalkOccurrences(rootDef.Occurrences, null, "Root");

            // Detect Frame Generator members
            DetectFrameMembers(rootDef);

            // Detect Tube & Pipe runs
            DetectTubeRuns(rootDef);

            var root = new InventorExportRec(
                SchemaVersion:  "1.0",
                InventorVersion: _inv.SoftwareVersion?.DisplayVersion ?? "Inventor 2024",
                ExportedAt:     t0.ToString("O"),
                AssemblyName:   doc.DisplayName,
                AssemblyFile:   Path.GetFileName(doc.FullFileName),
                VaultNumber:    GetIProperty(doc, "Number"),
                CoordinateUnit: "MM",
                Assemblies:     _assemblies,
                Parts:          _parts,
                FrameMembers:   _frames,
                TubeRuns:       _tubeRuns,
                NozzlePoints:   _nozzles,
                Summary: new InventorExportSummaryRec(
                    _assemblies.Count, _parts.Count,
                    _frames.Count, _tubeRuns.Count)
            );

            var opts = new JsonSerializerOptions
            {
                WriteIndented          = true,
                PropertyNamingPolicy   = JsonNamingPolicy.CamelCase,
                DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
            };
            File.WriteAllText(outputPath, JsonSerializer.Serialize(root, opts));

            var elapsed = (DateTime.UtcNow - t0).TotalSeconds;
            Console.WriteLine(
                $"PMEF Export: {_assemblies.Count} assemblies, {_parts.Count} parts, " +
                $"{_frames.Count} frame members, {_tubeRuns.Count} tube runs " +
                $"in {elapsed:F1}s → {outputPath}");
        }

        // ── Assembly traversal ────────────────────────────────────────────────

        private void WalkOccurrences(
            ComponentOccurrences occs, string? parentPath, string rootPath)
        {
            foreach (ComponentOccurrence occ in occs)
            {
                var occPath = parentPath == null
                    ? $"{rootPath}:{occ.Name}"
                    : $"{parentPath}:{occ.Name}";

                try
                {
                    switch (occ.DefinitionDocumentType)
                    {
                        case DocumentTypeEnum.kAssemblyDocumentObject:
                            MapAssemblyOcc(occ, occPath, parentPath);
                            // Recurse
                            var asmDef = (AssemblyComponentDefinition)
                                ((AssemblyDocument)occ.Definition).ComponentDefinition;
                            WalkOccurrences(asmDef.Occurrences, occPath, rootPath);
                            break;

                        case DocumentTypeEnum.kPartDocumentObject:
                            MapPartOcc(occ, occPath, parentPath);
                            break;
                    }
                }
                catch (Exception ex)
                {
                    Console.Error.WriteLine($"  Skip {occPath}: {ex.Message}");
                }
            }
        }

        private void MapAssemblyOcc(
            ComponentOccurrence occ, string occPath, string? parentPath)
        {
            var doc = (AssemblyDocument)occ.Definition;
            var def = (AssemblyComponentDefinition)doc.ComponentDefinition;

            // iProperties
            var props = MapIProperties(doc);

            // PMEF parameters
            var (pmefTag, pmefClass, dpBarg, dtDegc, designCode, userParams)
                = ReadPmefParams(def);

            // Bounding box
            var bbox = TryGetBbox(occ);

            // Transform to world
            var xform = MapTransform(occ.Transformation);

            // Nozzle work points (named PMEF_NOZZLE_*)
            var nozzleWPs = new List<InvWorkPointRec>();
            foreach (WorkPoint wp in def.WorkPoints)
            {
                if (!wp.Name.StartsWith("PMEF_NOZZLE_",
                    StringComparison.OrdinalIgnoreCase)) continue;

                var wpt = occ.Transformation.TransformPoint(wp.Point);
                var wpRec = new InvWorkPointRec(
                    Name:       wp.Name,
                    Position:   ToPoint(wpt),
                    XAxis:      new[] { 1.0, 0.0, 0.0 }, // default; override via axes if present
                    ZAxis:      new[] { 0.0, 0.0, 1.0 },
                    Parameters: ReadNozzleParams(def, wp.Name)
                );
                nozzleWPs.Add(wpRec);

                // Also add to global nozzle list
                var mark = wp.Name.Substring("PMEF_NOZZLE_".Length);
                _nozzles.Add(new InventorNozzlePointRec(
                    WorkPointName:           wp.Name,
                    NozzleMark:              mark,
                    ParentOccurrencePath:    occPath,
                    Position:                ToPoint(wpt),
                    Direction:               new[] { 0.0, 0.0, 1.0 },
                    NominalDiameterMm:       GetNozzleParam(def, "NZ_DN"),
                    FlangeRating:            GetStringParam(def, "NZ_RATING"),
                    FacingType:              GetStringParam(def, "NZ_FACING"),
                    Service:                 GetStringParam(def, "NZ_SERVICE")
                ));
            }

            // iPart info
            InvIPartInfoRec? ipartInfo = null;
            if (def is iPartComponentDefinition ipartDef)
            {
                try
                {
                    ipartInfo = new InvIPartInfoRec(
                        FactoryFile: Path.GetFileName(doc.FullFileName),
                        RowNumber:   (int)ipartDef.iPartTableRow,
                        MemberName:  ipartDef.iPartMemberName,
                        TableValues: new Dictionary<string, string>()
                    );
                }
                catch { /* not an iPart member */ }
            }

            _assemblies.Add(new InventorAssemblyRec(
                OccurrencePath:         occPath,
                Name:                   occ.Name,
                IamFile:                Path.GetFileName(doc.FullFileName),
                VaultNumber:            GetIProperty(doc, "Number"),
                Iproperties:            props,
                Parameters:             userParams,
                PmefTag:                pmefTag,
                PmefClass:              pmefClass,
                PmefDesignPressureBarg: dpBarg,
                PmefDesignTempDegc:     dtDegc,
                PmefDesignCode:         designCode,
                BoundingBox:            bbox,
                Transform:              xform,
                StepFile:               null,
                NozzleWorkPoints:       nozzleWPs,
                IsIAssembly:            occ.IsAdaptive,
                IpartInfo:              ipartInfo,
                ParentPath:             parentPath,
                ChildPaths:             new List<string>()
            ));
        }

        private void MapPartOcc(
            ComponentOccurrence occ, string occPath, string? parentPath)
        {
            var doc   = (PartDocument)occ.Definition;
            var props = MapIProperties(doc);
            var bbox  = TryGetBbox(occ);
            var xform = MapTransform(occ.Transformation);

            _parts.Add(new InventorPartRec(
                OccurrencePath: occPath,
                Name:           occ.Name,
                IptFile:        Path.GetFileName(doc.FullFileName),
                VaultNumber:    GetIProperty(doc, "Number"),
                Iproperties:    props,
                Parameters:     new Dictionary<string, object?>(),
                BoundingBox:    bbox,
                Transform:      xform,
                StepFile:       null,
                ParentPath:     parentPath
            ));
        }

        // ── Frame Generator ───────────────────────────────────────────────────

        private void DetectFrameMembers(AssemblyComponentDefinition def)
        {
            try
            {
                // Frame Generator members appear as FrameMember objects
                foreach (FrameMember fm in def.FrameMembers)
                {
                    try
                    {
                        var sp = ToPoint(fm.StartPoint);
                        var ep = ToPoint(fm.EndPoint);
                        var len = fm.Length * 10.0; // cm → mm

                        var memberType = fm.FrameMemberType switch
                        {
                            FrameMemberTypeEnum.kBeamFrameMember   => "BEAM",
                            FrameMemberTypeEnum.kColumnFrameMember => "COLUMN",
                            FrameMemberTypeEnum.kBraceFrameMember  => "BRACE",
                            _ => "OTHER"
                        };

                        var sectionInfo = fm.Section;
                        _frames.Add(new InventorFrameMemberRec(
                            OccurrencePath: $"Root:Frame:{fm.Name}",
                            Name:           fm.Name,
                            MemberType:     memberType,
                            SectionName:    sectionInfo?.Name ?? "UNKNOWN",
                            SectionStandard:sectionInfo?.Category ?? "ISO",
                            LengthMm:       len,
                            StartPoint:     sp,
                            EndPoint:       ep,
                            RollAngleDeg:   fm.RollAngle * (180.0 / Math.PI),
                            Material:       fm.Material?.Name ?? "S355JR",
                            MassKg:         null,
                            VaultNumber:    null,
                            Parameters:     new Dictionary<string, object?>()
                        ));
                    }
                    catch (Exception ex) {
                        Console.Error.WriteLine($"  Skip frame member {fm.Name}: {ex.Message}");
                    }
                }
            }
            catch { /* Frame Generator not present or no members */ }
        }

        // ── Tube & Pipe ───────────────────────────────────────────────────────

        private void DetectTubeRuns(AssemblyComponentDefinition def)
        {
            try
            {
                var tpDef = def as PipeComponentDefinition;
                if (tpDef == null) return;

                foreach (PipeRun run in tpDef.PipeRuns)
                {
                    try
                    {
                        var sp = ToPoint(run.StartPoint.Point);
                        var ep = ToPoint(run.EndPoint.Point);
                        var dn = run.NominalDiameter; // inches in Tube & Pipe
                        var od = run.OuterDiameter * 25.4; // in → mm (Inventor stores in inches)
                        var wt = (run.OuterDiameter - run.InnerDiameter) / 2.0 * 25.4;

                        var pts = new List<InvPointRec>();
                        foreach (Point pt in run.RoutePoints)
                            pts.Add(ToPoint(pt));

                        _tubeRuns.Add(new InventorTubeRunRec(
                            RunId:              run.Name,
                            RunName:            run.Name,
                            NominalDiameterIn:  dn,
                            PipeSpec:           run.PipeSpec?.Name,
                            OutsideDiameterMm:  od,
                            WallThicknessMm:    wt,
                            StartPoint:         sp,
                            EndPoint:           ep,
                            RoutePoints:        pts,
                            Material:           run.Material?.Name
                        ));
                    }
                    catch (Exception ex) {
                        Console.Error.WriteLine($"  Skip tube run {run.Name}: {ex.Message}");
                    }
                }
            }
            catch { /* Tube & Pipe not present */ }
        }

        // ── Helper methods ────────────────────────────────────────────────────

        private static InvPropertiesRec MapIProperties(Document doc)
        {
            var p = doc.PropertySets;
            string? Get(string set, string name) {
                try { return p[set][name]?.Value?.ToString(); }
                catch { return null; }
            }
            double? GetD(string set, string name) {
                var s = Get(set, name);
                return s != null && double.TryParse(s,
                    System.Globalization.NumberStyles.Any,
                    System.Globalization.CultureInfo.InvariantCulture,
                    out var v) ? v : null;
            }

            return new InvPropertiesRec(
                PartNumber:    Get("Design Tracking Properties", "Part Number"),
                Description:   Get("Design Tracking Properties", "Description"),
                Revision:      Get("Design Tracking Properties", "Revision Number"),
                Designer:      Get("Design Tracking Properties", "Designer"),
                Material:      Get("Physical Properties", "Material"),
                MassKg:        GetD("Physical Properties", "Mass"),
                SurfaceAreaM2: GetD("Physical Properties", "Surface Area"),
                Vendor:        Get("Design Tracking Properties", "Vendor"),
                Project:       Get("Design Tracking Properties", "Project"),
                Cost:          GetD("Design Tracking Properties", "Cost"),
                StockNumber:   Get("Design Tracking Properties", "Stock Number")
            );
        }

        private static (string? tag, string? cls, double? dp, double? dt,
                         string? code, Dictionary<string, object?> others)
        ReadPmefParams(ComponentDefinition def)
        {
            string? tag = null, cls = null, code = null;
            double? dp = null, dt = null;
            var others = new Dictionary<string, object?>();

            foreach (Parameter p in def.Parameters)
            {
                var name = p.Name.ToUpper();
                var val  = p.Expression?.ToString() ?? "";

                switch (name)
                {
                    case "PMEF_TAG":              tag  = val; break;
                    case "PMEF_CLASS":            cls  = val; break;
                    case "PMEF_DESIGN_PRESSURE":  dp   = TryDouble(val); break;
                    case "PMEF_DESIGN_TEMP":      dt   = TryDouble(val); break;
                    case "PMEF_DESIGN_CODE":      code = val; break;
                    default:
                        if (!name.StartsWith("_") && !name.StartsWith("D0"))
                            others[p.Name] = val;
                        break;
                }
            }
            return (tag, cls, dp, dt, code, others);
        }

        private static Dictionary<string, object?> ReadNozzleParams(
            ComponentDefinition def, string wpName)
        {
            var result = new Dictionary<string, object?>();
            string prefix = wpName + "_";
            foreach (Parameter p in def.Parameters)
            {
                if (p.Name.StartsWith(prefix, StringComparison.OrdinalIgnoreCase))
                {
                    var suffix = p.Name.Substring(prefix.Length);
                    result[suffix] = p.Expression?.ToString();
                }
            }
            return result;
        }

        private static double? GetNozzleParam(ComponentDefinition def, string name)
        {
            try { return TryDouble(def.Parameters[name].Expression?.ToString()); }
            catch { return null; }
        }

        private static string? GetStringParam(ComponentDefinition def, string name)
        {
            try { return def.Parameters[name].Expression?.ToString(); }
            catch { return null; }
        }

        private static string? GetIProperty(Document doc, string name)
        {
            try { return doc.PropertySets["Design Tracking Properties"][name]?.Value?.ToString(); }
            catch { return null; }
        }

        private static InvBboxRec? TryGetBbox(ComponentOccurrence occ)
        {
            try
            {
                var bb = occ.RangeBox;
                // Inventor stores in cm; convert to mm
                return new InvBboxRec(
                    bb.MinPoint.X * 10.0, bb.MaxPoint.X * 10.0,
                    bb.MinPoint.Y * 10.0, bb.MaxPoint.Y * 10.0,
                    bb.MinPoint.Z * 10.0, bb.MaxPoint.Z * 10.0
                );
            }
            catch { return null; }
        }

        private static InvTransformRec MapTransform(Matrix m)
        {
            // Inventor Matrix: row-major 4×4; elements accessed via Cell(row,col) 1-based
            // Inventor internal unit: cm. Translate to mm.
            return new InvTransformRec(
                Rotation: new[]
                {
                    new[] { m.Cell[1,1], m.Cell[1,2], m.Cell[1,3] },
                    new[] { m.Cell[2,1], m.Cell[2,2], m.Cell[2,3] },
                    new[] { m.Cell[3,1], m.Cell[3,2], m.Cell[3,3] }
                },
                Translation: new[]
                {
                    m.Cell[1,4] * 10.0,  // cm → mm
                    m.Cell[2,4] * 10.0,
                    m.Cell[3,4] * 10.0
                }
            );
        }

        private static InvPointRec ToPoint(Point p)
            => new InvPointRec(p.X * 10.0, p.Y * 10.0, p.Z * 10.0); // cm → mm

        private static double? TryDouble(string? s)
            => s != null && double.TryParse(s,
               System.Globalization.NumberStyles.Any,
               System.Globalization.CultureInfo.InvariantCulture,
               out var v) ? v : null;
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Add-in registration + iLogic entry point
    // ──────────────────────────────────────────────────────────────────────────

    /// Entry point when running as an iLogic External Rule.
    public static class ILogicEntry
    {
        /// iLogic rule entry point — called as: Run()
        public static void Run()
        {
            var inv = (Application)Marshal.GetActiveObject("Inventor.Application");
            var dlg = new Microsoft.Win32.SaveFileDialog
            {
                Title  = "PMEF Export — choose output file",
                Filter = "JSON files (*.json)|*.json|All files (*.*)|*.*",
                DefaultExt = "json",
                FileName   = "inventor-export.json"
            };
            if (dlg.ShowDialog() != true) return;

            try
            {
                var exporter = new InventorExporter(inv);
                exporter.Export(dlg.FileName);
                System.Windows.MessageBox.Show(
                    $"PMEF Export complete.\n{dlg.FileName}",
                    "PMEF", System.Windows.MessageBoxButton.OK,
                    System.Windows.MessageBoxImage.Information);
            }
            catch (Exception ex)
            {
                System.Windows.MessageBox.Show(
                    $"PMEF Export failed:\n{ex.Message}",
                    "PMEF", System.Windows.MessageBoxButton.OK,
                    System.Windows.MessageBoxImage.Error);
            }
        }
    }
}
