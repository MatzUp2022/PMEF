// PmefExporter.cs
// Tekla Structures Open API plugin — PMEF JSON export
//
// Runs inside the Tekla Structures process via the Open API plugin mechanism.
// Iterates the model, maps all structural members, connections, and assemblies
// to the TeklaExport JSON schema consumed by pmef-adapter-tekla (Rust).
//
// Build:  .NET 8, references Tekla.Structures.Model + Tekla.Structures.Drawing
// Usage:  Load as plugin via Tekla Applications & Components panel,
//         or run standalone from CLI with model open.
//
// Output: tekla-export.json (next to the .exe or in the model folder)

using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.Json;
using System.Text.Json.Serialization;

using Tekla.Structures;
using Tekla.Structures.Model;
using Tekla.Structures.Model.Operations;
using Tekla.Structures.Geometry3d;

namespace PmefTekla
{
    // ──────────────────────────────────────────────────────────────────────────
    // JSON export schema (mirrors export_schema.rs)
    // ──────────────────────────────────────────────────────────────────────────

    public record TeklaPoint(double X, double Y, double Z);
    public record TeklaBbox(TeklaPoint Min, TeklaPoint Max);

    public record TeklaEndRelease(
        bool MomentMajor = false,
        bool MomentMinor = false,
        bool Torsion = false
    );

    public record TeklaAnalysisResult(
        double? UtilisationRatio,
        string? CriticalCheck,
        double? AxialForceKn,
        double? MajorBendingKnm,
        double? MinorBendingKnm,
        double? ShearYKn,
        double? ShearZKn
    );

    public record TeklaFireProtection(
        string ProtectionType,
        int RequiredPeriodMin,
        double? SectionFactorM,
        double? ThicknessMm
    );

    public record TeklaBoltSpec(
        string Grade,
        double DiameterMm,
        int Count,
        string HoleType,
        bool Preloaded,
        string? Assembly
    );

    public record TeklaConnectionCapacity(
        double? ShearKn,
        double? MomentKnm,
        double? AxialKn
    );

    public record TeklaMemberRecord(
        string Identifier,
        long TeklaId,
        string MemberClass,
        string MemberMark,
        string? PartMark,
        string Profile,
        string Material,
        TeklaPoint StartPoint,
        TeklaPoint EndPoint,
        double RollAngleDeg,
        double LengthMm,
        double? MassKg,
        double? SurfaceAreaM2,
        string Guid,
        string? Cis2Ref,
        Dictionary<string, object?> Udas,
        string? AssemblyId,
        string? Finish,
        TeklaFireProtection? FireProtection,
        TeklaAnalysisResult? Analysis,
        TeklaBbox? Bbox,
        TeklaEndRelease StartRelease,
        TeklaEndRelease EndRelease
    );

    public record TeklaConnectionRecord(
        string Identifier,
        long TeklaId,
        int ComponentNumber,
        string ConnectionType,
        string? ConnectionMark,
        List<string> MemberGuids,
        TeklaPoint Position,
        TeklaBoltSpec? BoltSpec,
        double? WeldSizeMm,
        TeklaConnectionCapacity? DesignCapacity,
        double? UtilisationRatio
    );

    public record TeklaAssemblyRecord(
        string Identifier,
        string AssemblyMark,
        List<string> MemberGuids,
        double MassKg,
        double? SurfaceAreaM2,
        string? Finish
    );

    public record TeklaGridRecord(
        string Name,
        TeklaPoint Origin,
        List<string> XLabels,
        List<string> YLabels,
        List<string> ZLabels
    );

    public record TeklaProject(
        string ProjectName,
        string? ProjectNumber,
        string? Designer,
        string? DesignCode,
        string? SteelGrade
    );

    public record TeklaExportSummary(
        int MemberCount,
        int ConnectionCount,
        int AssemblyCount
    );

    public record TeklaExportRoot(
        string SchemaVersion,
        string TeklaVersion,
        string ExportedAt,
        string ModelName,
        TeklaProject? Project,
        List<TeklaMemberRecord> Members,
        List<TeklaConnectionRecord> Connections,
        List<TeklaAssemblyRecord> Assemblies,
        List<TeklaGridRecord> Grids,
        TeklaExportSummary Summary
    );

    // ──────────────────────────────────────────────────────────────────────────
    // Exporter
    // ──────────────────────────────────────────────────────────────────────────

    public class PmefExporter
    {
        private readonly Model _model;
        private readonly Dictionary<int, string> _idToGuid = new();

        public PmefExporter(Model model)
        {
            _model = model;
        }

        /// Run the export and write to the given output path.
        public void Export(string outputPath)
        {
            Console.WriteLine("PMEF Export: starting...");
            var t0 = DateTime.UtcNow;

            var members     = ExportMembers();
            var connections = ExportConnections();
            var assemblies  = ExportAssemblies();
            var grids       = ExportGrids();
            var project     = ExportProject();

            var root = new TeklaExportRoot(
                SchemaVersion: "1.0",
                TeklaVersion:  GetTeklaVersion(),
                ExportedAt:    t0.ToString("O"),
                ModelName:     _model.GetInfo().ModelName,
                Project:       project,
                Members:       members,
                Connections:   connections,
                Assemblies:    assemblies,
                Grids:         grids,
                Summary: new TeklaExportSummary(
                    members.Count, connections.Count, assemblies.Count)
            );

            var opts = new JsonSerializerOptions
            {
                WriteIndented         = true,
                PropertyNamingPolicy  = JsonNamingPolicy.CamelCase,
                DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
            };
            var json = JsonSerializer.Serialize(root, opts);
            File.WriteAllText(outputPath, json);

            var elapsed = (DateTime.UtcNow - t0).TotalSeconds;
            Console.WriteLine($"PMEF Export: {members.Count} members, " +
                              $"{connections.Count} connections in {elapsed:F1}s → {outputPath}");
        }

        // ── Members ───────────────────────────────────────────────────────────

        private List<TeklaMemberRecord> ExportMembers()
        {
            var result = new List<TeklaMemberRecord>();
            var selector = _model.GetModelObjectSelector();
            var objects  = selector.GetAllObjects();

            while (objects.MoveNext())
            {
                switch (objects.Current)
                {
                    case Beam beam:
                        var rec = MapBeam(beam);
                        if (rec != null)
                        {
                            result.Add(rec);
                            _idToGuid[beam.Identifier.ID] = beam.Identifier.GUID.ToString();
                        }
                        break;

                    case PolyBeam poly:
                        // Map only the first segment for now (full implementation handles all)
                        var polyRec = MapPolyBeam(poly);
                        if (polyRec != null)
                        {
                            result.Add(polyRec);
                            _idToGuid[poly.Identifier.ID] = poly.Identifier.GUID.ToString();
                        }
                        break;
                }
            }
            return result;
        }

        private TeklaMemberRecord? MapBeam(Beam beam)
        {
            try
            {
                var guid = beam.Identifier.GUID.ToString();
                var mark = GetStringAttr(beam, "PART_POS") ?? beam.Name;
                var profile = beam.Profile.ProfileString ?? "UNKNOWN";
                var material = beam.Material.MaterialString ?? "S355JR";

                var sp = ToTeklaPoint(beam.StartPoint);
                var ep = ToTeklaPoint(beam.EndPoint);
                var length = beam.StartPoint.Distance(beam.EndPoint);

                // Class detection
                var memberClass = DetectMemberClass(beam);

                // Roll angle
                beam.GetReportProperty("ROTATION", ref double rollDeg);

                // Mass and area
                double massKg = 0; beam.GetReportProperty("WEIGHT", ref massKg);
                double areaM2 = 0; beam.GetReportProperty("PAINT_AREA", ref areaM2);

                // End releases
                var startRel = MapEndRelease(beam.StartPointOffset);
                var endRel   = MapEndRelease(beam.EndPointOffset);

                // Bounding box
                var solid = beam.GetSolid();
                TeklaBbox? bbox = null;
                if (solid != null)
                {
                    bbox = new TeklaBbox(
                        new TeklaPoint(solid.MinimumPoint.X, solid.MinimumPoint.Y, solid.MinimumPoint.Z),
                        new TeklaPoint(solid.MaximumPoint.X, solid.MaximumPoint.Y, solid.MaximumPoint.Z)
                    );
                }

                // UDAs
                var udas = GetUdas(beam);

                // Fire protection
                var fireProt = MapFireProtection(beam);

                // Analysis (from linked analysis model if available)
                var analysis = TryGetAnalysisResults(beam);

                // Finish
                var finish = MapFinish(beam);

                // Assembly
                var assembly = beam.GetAssembly();
                var assemblyId = assembly != null
                    ? assembly.Identifier.GUID.ToString()
                    : null;

                return new TeklaMemberRecord(
                    Identifier:   guid,
                    TeklaId:      beam.Identifier.ID,
                    MemberClass:  memberClass,
                    MemberMark:   mark,
                    PartMark:     GetStringAttr(beam, "PART_POS"),
                    Profile:      profile,
                    Material:     material,
                    StartPoint:   sp,
                    EndPoint:     ep,
                    RollAngleDeg: rollDeg,
                    LengthMm:     length,
                    MassKg:       massKg > 0 ? massKg : null,
                    SurfaceAreaM2:areaM2 > 0 ? areaM2 : null,
                    Guid:         guid,
                    Cis2Ref:      GetStringAttr(beam, "CIS2_MEMBER_ID"),
                    Udas:         udas,
                    AssemblyId:   assemblyId,
                    Finish:       finish,
                    FireProtection: fireProt,
                    Analysis:     analysis,
                    Bbox:         bbox,
                    StartRelease: startRel,
                    EndRelease:   endRel
                );
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine($"Warning: could not map beam {beam.Identifier.ID}: {ex.Message}");
                return null;
            }
        }

        private TeklaMemberRecord? MapPolyBeam(PolyBeam poly)
        {
            // PolyBeam: use first and last contour points as start/end
            var pts = poly.Contour.ContourPoints.Cast<ContourPoint>().ToList();
            if (pts.Count < 2) return null;
            var first = pts.First();
            var last  = pts.Last();
            var guid  = poly.Identifier.GUID.ToString();
            var mark  = GetStringAttr(poly, "PART_POS") ?? poly.Name;
            double massKg = 0; poly.GetReportProperty("WEIGHT", ref massKg);

            return new TeklaMemberRecord(
                Identifier:   guid,
                TeklaId:      poly.Identifier.ID,
                MemberClass:  "PolyBeam",
                MemberMark:   mark,
                PartMark:     null,
                Profile:      poly.Profile.ProfileString ?? "UNKNOWN",
                Material:     poly.Material.MaterialString ?? "S355JR",
                StartPoint:   ToTeklaPoint(first),
                EndPoint:     ToTeklaPoint(last),
                RollAngleDeg: 0.0,
                LengthMm:     first.Distance(last),
                MassKg:       massKg > 0 ? massKg : null,
                SurfaceAreaM2:null,
                Guid:         guid,
                Cis2Ref:      null,
                Udas:         new Dictionary<string, object?>(),
                AssemblyId:   null,
                Finish:       null,
                FireProtection:null,
                Analysis:     null,
                Bbox:         null,
                StartRelease: new TeklaEndRelease(),
                EndRelease:   new TeklaEndRelease()
            );
        }

        // ── Connections ───────────────────────────────────────────────────────

        private List<TeklaConnectionRecord> ExportConnections()
        {
            var result   = new List<TeklaConnectionRecord>();
            var selector = _model.GetModelObjectSelector();
            var objects  = selector.GetAllObjects(
                new Type[] { typeof(BoltGroup), typeof(Weld), typeof(Component) });

            int idx = 0;
            while (objects.MoveNext())
            {
                switch (objects.Current)
                {
                    case BoltGroup bolts:
                        result.Add(MapBoltGroup(bolts, idx++));
                        break;
                    case Component comp:
                        var cr = MapComponent(comp, idx++);
                        if (cr != null) result.Add(cr);
                        break;
                }
            }
            return result;
        }

        private TeklaConnectionRecord MapBoltGroup(BoltGroup bolts, int idx)
        {
            var guid = bolts.Identifier.GUID.ToString();
            var members = new List<string>();
            if (bolts.PartToBeBolted != null)
                members.Add(_idToGuid.GetValueOrDefault(bolts.PartToBeBolted.Identifier.ID, ""));
            if (bolts.PartToBoltTo != null)
                members.Add(_idToGuid.GetValueOrDefault(bolts.PartToBoltTo.Identifier.ID, ""));

            var pos = bolts.BoltPositions.Count > 0
                ? ToTeklaPoint(bolts.BoltPositions[0])
                : new TeklaPoint(0, 0, 0);

            var spec = new TeklaBoltSpec(
                Grade:      bolts.BoltStandard ?? "8.8",
                DiameterMm: bolts.BoltSize,
                Count:      bolts.BoltPositions.Count,
                HoleType:   "CLEARANCE",
                Preloaded:  false,
                Assembly:   null
            );

            return new TeklaConnectionRecord(
                Identifier:       guid,
                TeklaId:          bolts.Identifier.ID,
                ComponentNumber:  0,
                ConnectionType:   "BoltedEndPlate",
                ConnectionMark:   null,
                MemberGuids:      members.Where(s => !string.IsNullOrEmpty(s)).ToList(),
                Position:         pos,
                BoltSpec:         spec,
                WeldSizeMm:       null,
                DesignCapacity:   null,
                UtilisationRatio: null
            );
        }

        private TeklaConnectionRecord? MapComponent(Component comp, int idx)
        {
            try
            {
                var guid = comp.Identifier.GUID.ToString();
                var children = comp.GetChildren();
                var memberGuids = new List<string>();
                while (children.MoveNext())
                {
                    if (children.Current is ModelObject mo)
                    {
                        var childGuid = _idToGuid.GetValueOrDefault(mo.Identifier.ID);
                        if (childGuid != null) memberGuids.Add(childGuid);
                    }
                }

                var pos = ToTeklaPoint(comp.GetCoordinateSystem().Origin);
                var connType = MapConnectionType(comp.Number);

                return new TeklaConnectionRecord(
                    Identifier:       guid,
                    TeklaId:          comp.Identifier.ID,
                    ComponentNumber:  comp.Number,
                    ConnectionType:   connType,
                    ConnectionMark:   comp.Name,
                    MemberGuids:      memberGuids,
                    Position:         pos,
                    BoltSpec:         null,
                    WeldSizeMm:       null,
                    DesignCapacity:   null,
                    UtilisationRatio: null
                );
            }
            catch { return null; }
        }

        // ── Assemblies ────────────────────────────────────────────────────────

        private List<TeklaAssemblyRecord> ExportAssemblies()
        {
            var result   = new List<TeklaAssemblyRecord>();
            var selector = _model.GetModelObjectSelector();
            var assemblies = selector.GetAllObjects(new Type[] { typeof(Assembly) });

            while (assemblies.MoveNext())
            {
                if (assemblies.Current is Assembly asm)
                {
                    double massKg = 0; asm.GetReportProperty("WEIGHT", ref massKg);
                    var parts = asm.GetMainPart() != null
                        ? new List<ModelObject> { asm.GetMainPart() }
                        : new List<ModelObject>();

                    var subParts = asm.GetSubParts();
                    while (subParts.MoveNext())
                        parts.Add(subParts.Current as ModelObject);

                    var memberGuids = parts
                        .Where(p => p != null)
                        .Select(p => _idToGuid.GetValueOrDefault(p.Identifier.ID, ""))
                        .Where(g => !string.IsNullOrEmpty(g))
                        .ToList();

                    result.Add(new TeklaAssemblyRecord(
                        Identifier:    asm.Identifier.GUID.ToString(),
                        AssemblyMark:  asm.AssemblyNumber?.Prefix + asm.AssemblyNumber?.StartNumber,
                        MemberGuids:   memberGuids,
                        MassKg:        massKg,
                        SurfaceAreaM2: null,
                        Finish:        null
                    ));
                }
            }
            return result;
        }

        // ── Grids ─────────────────────────────────────────────────────────────

        private List<TeklaGridRecord> ExportGrids()
        {
            var result   = new List<TeklaGridRecord>();
            var selector = _model.GetModelObjectSelector();
            var grids    = selector.GetAllObjects(new Type[] { typeof(Grid) });

            while (grids.MoveNext())
            {
                if (grids.Current is Grid grid)
                {
                    result.Add(new TeklaGridRecord(
                        Name:    grid.Name ?? "Grid",
                        Origin:  new TeklaPoint(grid.Origin.X, grid.Origin.Y, grid.Origin.Z),
                        XLabels: grid.CoordinateX?.Trim().Split(' ')
                                     .Where(s => !string.IsNullOrEmpty(s)).ToList()
                                 ?? new List<string>(),
                        YLabels: grid.CoordinateY?.Trim().Split(' ')
                                     .Where(s => !string.IsNullOrEmpty(s)).ToList()
                                 ?? new List<string>(),
                        ZLabels: grid.CoordinateZ?.Trim().Split(' ')
                                     .Where(s => !string.IsNullOrEmpty(s)).ToList()
                                 ?? new List<string>()
                    ));
                }
            }
            return result;
        }

        // ── Project ───────────────────────────────────────────────────────────

        private TeklaProject? ExportProject()
        {
            var info = _model.GetProjectInfo();
            if (info == null) return null;
            return new TeklaProject(
                ProjectName:   info.ProjectName ?? "",
                ProjectNumber: info.ProjectNumber,
                Designer:      null,
                DesignCode:    null,
                SteelGrade:    null
            );
        }

        // ── Helper methods ────────────────────────────────────────────────────

        private static TeklaPoint ToTeklaPoint(Point p) =>
            new TeklaPoint(Math.Round(p.X, 3), Math.Round(p.Y, 3), Math.Round(p.Z, 3));

        private static TeklaPoint ToTeklaPoint(ContourPoint p) =>
            new TeklaPoint(Math.Round(p.X, 3), Math.Round(p.Y, 3), Math.Round(p.Z, 3));

        private static string DetectMemberClass(Beam beam)
        {
            return beam.Type switch
            {
                Beam.BeamTypeEnum.BEAM   => "Beam",
                Beam.BeamTypeEnum.COLUMN => "Column",
                _                        => "Beam"
            };
        }

        private static TeklaEndRelease MapEndRelease(Offset offset)
        {
            // Tekla uses release codes in the offset; simplified mapping
            return new TeklaEndRelease(
                MomentMajor: false,
                MomentMinor: false,
                Torsion:     false
            );
        }

        private static TeklaFireProtection? MapFireProtection(Beam beam)
        {
            string? fpType = null;
            beam.GetUserProperty("FIRE_PROTECTION_TYPE", ref fpType);
            if (string.IsNullOrEmpty(fpType)) return null;

            int period = 0;
            beam.GetUserProperty("FIRE_RESISTANCE_PERIOD", ref period);
            double thickness = 0;
            beam.GetUserProperty("INTUMESCENT_THICKNESS", ref thickness);

            return new TeklaFireProtection(
                ProtectionType:   fpType,
                RequiredPeriodMin: period,
                SectionFactorM:   null,
                ThicknessMm:      thickness > 0 ? thickness : null
            );
        }

        private static TeklaAnalysisResult? TryGetAnalysisResults(Beam beam)
        {
            // Read analysis results from UDAs if they were written by an analysis link
            double util = 0;
            if (!beam.GetUserProperty("PMEF_UTILISATION_RATIO", ref util)) return null;

            string? critCheck = null;
            beam.GetUserProperty("PMEF_CRITICAL_CHECK", ref critCheck);
            double axial = 0; beam.GetUserProperty("PMEF_AXIAL_KN", ref axial);
            double mjBend = 0; beam.GetUserProperty("PMEF_MAJOR_BEND_KNM", ref mjBend);

            return new TeklaAnalysisResult(
                UtilisationRatio: util,
                CriticalCheck:    critCheck,
                AxialForceKn:     axial != 0 ? axial : null,
                MajorBendingKnm:  mjBend != 0 ? mjBend : null,
                MinorBendingKnm:  null,
                ShearYKn:         null,
                ShearZKn:         null
            );
        }

        private static string? MapFinish(Beam beam)
        {
            string? finish = null;
            beam.GetUserProperty("SURFACE_TREATMENT", ref finish);
            return finish switch
            {
                "HDG" or "HOT_DIP_GALVANIZED" => "HotDipGalvanized",
                "EPOXY"                        => "PaintedEpoxy",
                "ALKYD"                        => "PaintedAlkyd",
                "BLAST"                        => "Blasted",
                _                              => null
            };
        }

        private static Dictionary<string, object?> GetUdas(ModelObject obj)
        {
            var udas = new Dictionary<string, object?>();
            // Common UDAs to export (extend as needed)
            string[] udaNames =
            [
                "ERECTION_SEQUENCE", "SHOP_MARK", "FIRE_ZONE", "BLAST_ZONE",
                "HOLD_POINT", "WPS_REFERENCE", "INSPECTION_CLASS", "CORROSION_ZONE"
            ];
            foreach (var name in udaNames)
            {
                string? val = null;
                if (obj.GetUserProperty(name, ref val) && !string.IsNullOrEmpty(val))
                    udas[name] = val;
            }
            return udas;
        }

        private static string? GetStringAttr(ModelObject obj, string attr)
        {
            string? val = null;
            obj.GetReportProperty(attr, ref val);
            return string.IsNullOrEmpty(val) ? null : val;
        }

        private static string GetTeklaVersion()
        {
            try { return TeklaStructuresInfo.GetCurrentProgramVersion(); }
            catch { return "UNKNOWN"; }
        }

        /// Map Tekla system component number to a connection type string.
        private static string MapConnectionType(int componentNumber) => componentNumber switch
        {
            142 | 143 => "BoltedEndPlate",
            144        => "MomentEndPlate",
            145 | 146 => "BoltedCleat",
            1003       => "WeldedDirect",
            1004       => "BoltedSplice",
            1047 | 1048=> "PinnedBase",
            1049       => "FixedBase",
            11 | 12    => "TubularKJoint",
            _          => "Other"
        };
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Entry point — runs as standalone or Tekla plugin
    // ──────────────────────────────────────────────────────────────────────────

    public class Program
    {
        [STAThread]
        static int Main(string[] args)
        {
            var outputPath = args.Length > 0 ? args[0] : "tekla-export.json";

            var model = new Model();
            if (!model.GetConnectionStatus())
            {
                Console.Error.WriteLine(
                    "ERROR: Tekla Structures is not running or not connected. " +
                    "Please open a model in Tekla Structures and try again.");
                return 1;
            }

            try
            {
                var exporter = new PmefExporter(model);
                exporter.Export(outputPath);
                Console.WriteLine($"SUCCESS: Export written to {outputPath}");
                return 0;
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine($"EXPORT FAILED: {ex.Message}");
                Console.Error.WriteLine(ex.StackTrace);
                return 2;
            }
        }
    }
}
