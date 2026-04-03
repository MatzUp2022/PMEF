// ComosExporter.cs
// Siemens COMOS Engineering Data Management — PMEF JSON export
//
// Uses the COMOS .NET API (COMOS Platform SDK 10.4+) to read the plant model
// and write a structured JSON export consumed by pmef-adapter-comos (Rust).
//
// Build: .NET 8, references COMOS.Platform + COMOS.Objects.Engineering
// Usage: Run with COMOS open: ComosExporter.exe [output.json]
//
// Reference: Siemens COMOS SDK Documentation 10.4
//   https://support.industry.siemens.com/cs/products?dtp=Documentation&mfn=cs&pnid=23620

using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Threading;

// COMOS .NET API namespaces
using Comos.Platform;
using Comos.Objects;
using Comos.Objects.Engineering;
using Comos.Scripting;

namespace PmefComos
{
    // ──────────────────────────────────────────────────────────────────────────
    // JSON export schema (mirrors export_schema.rs)
    // ──────────────────────────────────────────────────────────────────────────

    public record ComosNozzleRec(
        string Cuid, string NozzleMark,
        string? Service, double? NominalDiameterMm,
        string? FlangeRating, string? FacingType,
        string? ConnectedLineCuid, string? Iec81346
    );

    public record ComosDocRef(
        string DocumentCuid, string DocumentType,
        string? DocumentNumber, string? Revision
    );

    public record ComosEquipmentDesign(
        double? DesignPressureBarg, double? DesignTemperatureDegc,
        double? DesignTemperatureMinDegc, double? OperatingPressureBarg,
        double? OperatingTemperatureDegc, double? VolumeM3,
        string? Material, string? DesignCode,
        double? WeightEmptyKg, double? WeightOperatingKg,
        string? Manufacturer, string? Model,
        double? MotorPowerKw, double? DesignFlowM3H,
        double? DesignHeadM, double? HeatDutyKw,
        double? HeatTransferAreaM2, string? TemaType,
        double? InsideDiameterMm, double? TangentLengthMm,
        double? ShellSidePressureBarg, double? TubeSidePressureBarg
    );

    public record ComosEquipmentRec(
        string Cuid, string TagNumber, string ComosClass,
        string ClassDescription, string? Description,
        string UnitCuid, string? PidReference, string? Status,
        string? Iec81346Functional, string? Iec81346Product,
        List<ComosNozzleRec> Nozzles,
        ComosEquipmentDesign DesignAttrs,
        Dictionary<string, object?> RawAttrs,
        List<ComosDocRef> Documents
    );

    public record ComosInstrumentDesign(
        string? ProcessVariable, double? RangeMin, double? RangeMax,
        string? RangeUnit, string? SignalType, string? FailSafe,
        int? SilLevel, int? ProofTestIntervalMonths,
        double? Pfd, double? Pfh, string? Architecture,
        string? SafeState, bool? IntrinsicSafe, string? HazardousArea,
        string? IpRating, string? Manufacturer, string? Model,
        string? TiaPLCAddress, string? EplanFunctionText,
        double? KvValue, string? ShutoffClass, string? ActuatorType
    );

    public record ComosInstrumentRec(
        string Cuid, string TagNumber, string ComosClass,
        string ClassDescription, string UnitCuid, string? LoopCuid,
        string? PidReference, string? Iec81346Functional,
        string? Iec81346Product, string? Status,
        ComosInstrumentDesign DesignAttrs,
        Dictionary<string, object?> RawAttrs,
        List<ComosDocRef> Documents
    );

    public record ComosLoopRec(
        string Cuid, string LoopNumber, string LoopType,
        string UnitCuid, int? SilLevel, string? Status,
        List<string> MemberCuids, string? ControllerCuid,
        string? FinalElementCuid, string? PidReference,
        string? Iec81346Functional
    );

    public record ComotLineRec(
        string Cuid, string LineNumber, string UnitCuid,
        string? Description, double? NominalDiameterMm, string? PipeClass,
        string? MediumCode, string? MediumDescription,
        double? DesignPressureBarg, double? DesignTemperatureDegc,
        double? OperatingPressureBarg, double? OperatingTemperatureDegc,
        double? TestPressureBarg, string? Material, string? InsulationType,
        string? HeatTracing, string? PidReference,
        string? Iec81346Functional, string? Status,
        Dictionary<string, object?> RawAttrs
    );

    public record ComosCableRec(
        string Cuid, string CableNumber, string ComosClass,
        string UnitCuid, string? CableType, double? CrossSectionMm2,
        int? NumberOfCores, int? VoltageRatingV,
        string? FromCuid, string? ToCuid, double? RouteLengthM,
        string? CableTray_Cuid, string? Iec81346Product
    );

    public record ComosPlcRec(
        string Cuid, string TagNumber, string ComosClass,
        string ClassDescription, string UnitCuid,
        string? Vendor, string? Family, string? ArticleNumber,
        int? Rack, int? Slot, string? IpAddress,
        bool? SafetyCpu, string? TiaPortalRef, string? AmlRef,
        string? Iec81346Product
    );

    public record ComosUnitRec(
        string Cuid, string Name, string? Description,
        string ComosClass, string? ParentCuid, string? Iec81346Functional
    );

    public record ComosExportSummary(
        int EquipmentCount, int InstrumentCount, int LoopCount,
        int LineCount, int CableCount, int PlcCount
    );

    public record ComosExportRoot(
        string SchemaVersion, string ComosVersion,
        string ExportedAt, string ProjectName, string ProjectCuid,
        List<ComosUnitRec> PlantUnits,
        List<ComosEquipmentRec> Equipment,
        List<ComotLineRec> PipingLines,
        List<ComosInstrumentRec> Instruments,
        List<ComosLoopRec> InstrumentLoops,
        List<ComosCableRec> Cables,
        List<ComosPlcRec> PlcObjects,
        List<object> Documents,
        ComosExportSummary Summary
    );

    // ──────────────────────────────────────────────────────────────────────────
    // Exporter
    // ──────────────────────────────────────────────────────────────────────────

    public class ComosExporter
    {
        private readonly IProject _project;

        public ComosExporter(IProject project) { _project = project; }

        public void Export(string outputPath)
        {
            Console.WriteLine("PMEF/COMOS Export: starting...");
            var t0 = DateTime.UtcNow;

            var units       = ExportUnits();
            var equipment   = ExportEquipment();
            var lines       = ExportPipingLines();
            var instruments = ExportInstruments();
            var loops       = ExportLoops();
            var cables      = ExportCables();
            var plcObjects  = ExportPlcObjects();

            var root = new ComosExportRoot(
                SchemaVersion: "1.0",
                ComosVersion:  GetComosVersion(),
                ExportedAt:    t0.ToString("O"),
                ProjectName:   _project.Name,
                ProjectCuid:   _project.CosId,
                PlantUnits:    units,
                Equipment:     equipment,
                PipingLines:   lines,
                Instruments:   instruments,
                InstrumentLoops: loops,
                Cables:        cables,
                PlcObjects:    plcObjects,
                Documents:     new List<object>(),
                Summary: new ComosExportSummary(
                    equipment.Count, instruments.Count, loops.Count,
                    lines.Count, cables.Count, plcObjects.Count)
            );

            var opts = new JsonSerializerOptions
            {
                WriteIndented = true,
                PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
                DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
            };
            File.WriteAllText(outputPath, JsonSerializer.Serialize(root, opts));

            var elapsed = (DateTime.UtcNow - t0).TotalSeconds;
            Console.WriteLine($"PMEF/COMOS Export: {equipment.Count} equip, " +
                              $"{instruments.Count} instr, {lines.Count} lines, " +
                              $"{cables.Count} cables in {elapsed:F1}s → {outputPath}");
        }

        // ── Plant units ───────────────────────────────────────────────────────

        private List<ComosUnitRec> ExportUnits()
        {
            var result = new List<ComosUnitRec>();
            // COMOS plant hierarchy: @A (Anlage/Plant) class branch
            foreach (IBase obj in _project.SystemObjects.Children)
            {
                if (IsClassMatch(obj, "@A"))
                    result.Add(MapUnit(obj));
            }
            return result;
        }

        private static ComosUnitRec MapUnit(IBase obj) => new ComosUnitRec(
            Cuid:               obj.CosId,
            Name:               obj.Name,
            Description:        GetAttr(obj, "CTA_Description"),
            ComosClass:         obj.SystemClass?.Name ?? "",
            ParentCuid:         obj.Parent?.CosId,
            Iec81346Functional: GetAttr(obj, "CTA_FunctionalDesignation")
        );

        // ── Equipment ─────────────────────────────────────────────────────────

        private List<ComosEquipmentRec> ExportEquipment()
        {
            var result = new List<ComosEquipmentRec>();
            var query = _project.CreateQueryBySystemClass("@E");
            foreach (IBase obj in query.Execute())
            {
                try { result.Add(MapEquipment(obj)); }
                catch (Exception ex) {
                    Console.Error.WriteLine($"Warning: skip equipment {obj.Name}: {ex.Message}");
                }
            }
            return result;
        }

        private static ComosEquipmentRec MapEquipment(IBase obj)
        {
            var nozzles = obj.Children
                .Cast<IBase>()
                .Where(c => IsClassMatch(c, "@N"))
                .Select(MapNozzle)
                .ToList();

            var docs = GetLinkedDocuments(obj);

            var design = new ComosEquipmentDesign(
                DesignPressureBarg:       GetAttrDouble(obj, "CTA_DesignPressure"),
                DesignTemperatureDegc:    GetAttrDouble(obj, "CTA_DesignTemperature"),
                DesignTemperatureMinDegc: GetAttrDouble(obj, "CTA_DesignTemperatureMin"),
                OperatingPressureBarg:    GetAttrDouble(obj, "CTA_OperatingPressure"),
                OperatingTemperatureDegc: GetAttrDouble(obj, "CTA_OperatingTemperature"),
                VolumeM3:                GetAttrDouble(obj, "CTA_Volume"),
                Material:                GetAttr(obj, "CTA_Material"),
                DesignCode:              GetAttr(obj, "CTA_DesignCode"),
                WeightEmptyKg:           GetAttrDouble(obj, "CTA_Weight"),
                WeightOperatingKg:       GetAttrDouble(obj, "CTA_OperatingWeight"),
                Manufacturer:            GetAttr(obj, "CTA_Manufacturer"),
                Model:                   GetAttr(obj, "CTA_Type"),
                MotorPowerKw:            GetAttrDouble(obj, "CTA_MotorPower"),
                DesignFlowM3H:           GetAttrDouble(obj, "CTA_FlowDesign"),
                DesignHeadM:             GetAttrDouble(obj, "CTA_Head"),
                HeatDutyKw:              GetAttrDouble(obj, "CTA_Duty"),
                HeatTransferAreaM2:      GetAttrDouble(obj, "CTA_HeatTransferArea"),
                TemaType:                GetAttr(obj, "CTA_TEMAType"),
                InsideDiameterMm:        GetAttrDouble(obj, "CTA_InsideDiameter"),
                TangentLengthMm:         GetAttrDouble(obj, "CTA_LengthTangentTangent"),
                ShellSidePressureBarg:   GetAttrDouble(obj, "CTA_ShellPressure"),
                TubeSidePressureBarg:    GetAttrDouble(obj, "CTA_TubePressure")
            );

            return new ComosEquipmentRec(
                Cuid:               obj.CosId,
                TagNumber:          GetAttr(obj, "TAG") ?? obj.Name,
                ComosClass:         obj.SystemClass?.Name ?? "",
                ClassDescription:   obj.SystemClass?.Description ?? "",
                Description:        GetAttr(obj, "CTA_Description"),
                UnitCuid:           obj.Parent?.CosId ?? "",
                PidReference:       GetAttr(obj, "CTA_PIDReference"),
                Status:             GetAttr(obj, "CTA_Status"),
                Iec81346Functional: GetAttr(obj, "CTA_FunctionalDesignation"),
                Iec81346Product:    GetAttr(obj, "CTA_ProductDesignation"),
                Nozzles:            nozzles,
                DesignAttrs:        design,
                RawAttrs:           new Dictionary<string, object?>(),
                Documents:          docs
            );
        }

        private static ComosNozzleRec MapNozzle(IBase noz) => new ComosNozzleRec(
            Cuid:               noz.CosId,
            NozzleMark:         GetAttr(noz, "TAG") ?? noz.Name,
            Service:            GetAttr(noz, "CTA_Service"),
            NominalDiameterMm:  GetAttrDouble(noz, "CTA_NominalDiameter"),
            FlangeRating:       GetAttr(noz, "CTA_FlangeRating"),
            FacingType:         GetAttr(noz, "CTA_FacingType"),
            ConnectedLineCuid:  GetLinkedObjectCuid(noz, "@L10"),
            Iec81346:           GetAttr(noz, "CTA_ProductDesignation")
        );

        // ── Piping lines ──────────────────────────────────────────────────────

        private List<ComotLineRec> ExportPipingLines()
        {
            var result = new List<ComotLineRec>();
            var query = _project.CreateQueryBySystemClass("@L10");
            foreach (IBase obj in query.Execute())
            {
                try { result.Add(MapPipingLine(obj)); }
                catch (Exception ex) {
                    Console.Error.WriteLine($"Warning: skip line {obj.Name}: {ex.Message}");
                }
            }
            return result;
        }

        private static ComotLineRec MapPipingLine(IBase obj) => new ComotLineRec(
            Cuid:                   obj.CosId,
            LineNumber:             GetAttr(obj, "CTA_LineNumber") ?? obj.Name,
            UnitCuid:               obj.Parent?.CosId ?? "",
            Description:            GetAttr(obj, "CTA_Description"),
            NominalDiameterMm:      GetAttrDouble(obj, "CTA_NominalDiameter"),
            PipeClass:              GetAttr(obj, "CTA_PipeClass"),
            MediumCode:             GetAttr(obj, "CTA_MediumCode"),
            MediumDescription:      GetAttr(obj, "CTA_Medium"),
            DesignPressureBarg:     GetAttrDouble(obj, "CTA_DesignPressure"),
            DesignTemperatureDegc:  GetAttrDouble(obj, "CTA_DesignTemperature"),
            OperatingPressureBarg:  GetAttrDouble(obj, "CTA_OperatingPressure"),
            OperatingTemperatureDegc: GetAttrDouble(obj, "CTA_OperatingTemperature"),
            TestPressureBarg:       GetAttrDouble(obj, "CTA_TestPressure"),
            Material:               GetAttr(obj, "CTA_Material"),
            InsulationType:         GetAttr(obj, "CTA_Insulation"),
            HeatTracing:            GetAttr(obj, "CTA_HeatTracing"),
            PidReference:           GetAttr(obj, "CTA_PIDReference"),
            Iec81346Functional:     GetAttr(obj, "CTA_FunctionalDesignation"),
            Status:                 GetAttr(obj, "CTA_Status"),
            RawAttrs:               new Dictionary<string, object?>()
        );

        // ── Instruments ───────────────────────────────────────────────────────

        private List<ComosInstrumentRec> ExportInstruments()
        {
            var result = new List<ComosInstrumentRec>();
            var query = _project.CreateQueryBySystemClass("@I");
            foreach (IBase obj in query.Execute())
            {
                // Skip instrument loops (handled separately)
                if (IsClassMatch(obj, "@I05")) continue;
                try { result.Add(MapInstrument(obj)); }
                catch (Exception ex) {
                    Console.Error.WriteLine($"Warning: skip instrument {obj.Name}: {ex.Message}");
                }
            }
            return result;
        }

        private static ComosInstrumentRec MapInstrument(IBase obj)
        {
            var loopRef = GetLinkedObjectCuid(obj, "@I05");
            var design = new ComosInstrumentDesign(
                ProcessVariable:        GetAttr(obj, "CTA_ProcessVariable"),
                RangeMin:               GetAttrDouble(obj, "CTA_RangeMin"),
                RangeMax:               GetAttrDouble(obj, "CTA_RangeMax"),
                RangeUnit:              GetAttr(obj, "CTA_Unit"),
                SignalType:             GetAttr(obj, "CTA_SignalType"),
                FailSafe:               GetAttr(obj, "CTA_FailSafe"),
                SilLevel:               GetAttrInt(obj, "CTA_SIL"),
                ProofTestIntervalMonths:GetAttrInt(obj, "CTA_ProofTestInterval"),
                Pfd:                    GetAttrDouble(obj, "CTA_PFD"),
                Pfh:                    GetAttrDouble(obj, "CTA_PFH"),
                Architecture:           GetAttr(obj, "CTA_Architecture"),
                SafeState:              GetAttr(obj, "CTA_SafeState"),
                IntrinsicSafe:          GetAttrBool(obj, "CTA_ExProtection"),
                HazardousArea:          GetAttr(obj, "CTA_HazArea"),
                IpRating:               GetAttr(obj, "CTA_IPRating"),
                Manufacturer:           GetAttr(obj, "CTA_Manufacturer"),
                Model:                  GetAttr(obj, "CTA_Model"),
                TiaPLCAddress:          GetAttr(obj, "CTA_TIAAddress"),
                EplanFunctionText:      GetAttr(obj, "CTA_EPLANFunctionText"),
                KvValue:                GetAttrDouble(obj, "CTA_Kv"),
                ShutoffClass:           GetAttr(obj, "CTA_ShutoffClass"),
                ActuatorType:           GetAttr(obj, "CTA_ActuatorType")
            );

            return new ComosInstrumentRec(
                Cuid:               obj.CosId,
                TagNumber:          GetAttr(obj, "TAG") ?? obj.Name,
                ComosClass:         obj.SystemClass?.Name ?? "",
                ClassDescription:   obj.SystemClass?.Description ?? "",
                UnitCuid:           obj.Parent?.CosId ?? "",
                LoopCuid:           loopRef,
                PidReference:       GetAttr(obj, "CTA_PIDReference"),
                Iec81346Functional: GetAttr(obj, "CTA_FunctionalDesignation"),
                Iec81346Product:    GetAttr(obj, "CTA_ProductDesignation"),
                Status:             GetAttr(obj, "CTA_Status"),
                DesignAttrs:        design,
                RawAttrs:           new Dictionary<string, object?>(),
                Documents:          GetLinkedDocuments(obj)
            );
        }

        // ── Instrument loops ──────────────────────────────────────────────────

        private List<ComosLoopRec> ExportLoops()
        {
            var result = new List<ComosLoopRec>();
            var query = _project.CreateQueryBySystemClass("@I05");
            foreach (IBase obj in query.Execute())
            {
                try
                {
                    var members = obj.Children.Cast<IBase>()
                        .Select(c => c.CosId).ToList();
                    result.Add(new ComosLoopRec(
                        Cuid:               obj.CosId,
                        LoopNumber:         GetAttr(obj, "CTA_LoopNumber") ?? obj.Name,
                        LoopType:           GetAttr(obj, "CTA_LoopType") ?? "",
                        UnitCuid:           obj.Parent?.CosId ?? "",
                        SilLevel:           GetAttrInt(obj, "CTA_SIL"),
                        Status:             GetAttr(obj, "CTA_Status"),
                        MemberCuids:        members,
                        ControllerCuid:     GetLinkedObjectCuid(obj, "@I20"),
                        FinalElementCuid:   GetLinkedObjectCuid(obj, "@I30"),
                        PidReference:       GetAttr(obj, "CTA_PIDReference"),
                        Iec81346Functional: GetAttr(obj, "CTA_FunctionalDesignation")
                    ));
                }
                catch (Exception ex) {
                    Console.Error.WriteLine($"Warning: skip loop {obj.Name}: {ex.Message}");
                }
            }
            return result;
        }

        // ── Cables ────────────────────────────────────────────────────────────

        private List<ComosCableRec> ExportCables()
        {
            var result = new List<ComosCableRec>();
            var query = _project.CreateQueryBySystemClass("@K");
            foreach (IBase obj in query.Execute())
            {
                try
                {
                    result.Add(new ComosCableRec(
                        Cuid:              obj.CosId,
                        CableNumber:       GetAttr(obj, "CTA_CableNumber") ?? obj.Name,
                        ComosClass:        obj.SystemClass?.Name ?? "",
                        UnitCuid:          obj.Parent?.CosId ?? "",
                        CableType:         GetAttr(obj, "CTA_CableType"),
                        CrossSectionMm2:   GetAttrDouble(obj, "CTA_CrossSection"),
                        NumberOfCores:     GetAttrInt(obj, "CTA_NumberOfCores"),
                        VoltageRatingV:    GetAttrInt(obj, "CTA_VoltageRating"),
                        FromCuid:          GetLinkedObjectCuid(obj, "@I"),
                        ToCuid:            GetLinkedObjectCuid(obj, "@S"),
                        RouteLengthM:      GetAttrDouble(obj, "CTA_CableLength"),
                        CableTray_Cuid:    GetLinkedObjectCuid(obj, "@KT"),
                        Iec81346Product:   GetAttr(obj, "CTA_ProductDesignation")
                    ));
                }
                catch (Exception ex) {
                    Console.Error.WriteLine($"Warning: skip cable {obj.Name}: {ex.Message}");
                }
            }
            return result;
        }

        // ── PLC objects ───────────────────────────────────────────────────────

        private List<ComosPlcRec> ExportPlcObjects()
        {
            var result = new List<ComosPlcRec>();
            var query = _project.CreateQueryBySystemClass("@S");
            foreach (IBase obj in query.Execute())
            {
                try
                {
                    result.Add(new ComosPlcRec(
                        Cuid:            obj.CosId,
                        TagNumber:       GetAttr(obj, "TAG") ?? obj.Name,
                        ComosClass:      obj.SystemClass?.Name ?? "",
                        ClassDescription:obj.SystemClass?.Description ?? "",
                        UnitCuid:        obj.Parent?.CosId ?? "",
                        Vendor:          GetAttr(obj, "CTA_Manufacturer"),
                        Family:          GetAttr(obj, "CTA_Family"),
                        ArticleNumber:   GetAttr(obj, "CTA_ArticleNumber"),
                        Rack:            GetAttrInt(obj, "CTA_Rack"),
                        Slot:            GetAttrInt(obj, "CTA_Slot"),
                        IpAddress:       GetAttr(obj, "CTA_IPAddress"),
                        SafetyCpu:       GetAttrBool(obj, "CTA_SafetyCPU"),
                        TiaPortalRef:    GetAttr(obj, "CTA_TIAPortalReference"),
                        AmlRef:          GetAttr(obj, "CTA_AMLReference"),
                        Iec81346Product: GetAttr(obj, "CTA_ProductDesignation")
                    ));
                }
                catch (Exception ex) {
                    Console.Error.WriteLine($"Warning: skip PLC {obj.Name}: {ex.Message}");
                }
            }
            return result;
        }

        // ── Helper methods ────────────────────────────────────────────────────

        private static bool IsClassMatch(IBase obj, string classPrefix)
            => obj.SystemClass?.Name?.StartsWith(classPrefix,
               StringComparison.OrdinalIgnoreCase) == true;

        private static string? GetAttr(IBase obj, string attrName)
        {
            try
            {
                var attr = obj.Attributes[attrName];
                var val = attr?.Value?.ToString();
                return string.IsNullOrWhiteSpace(val) ? null : val;
            }
            catch { return null; }
        }

        private static double? GetAttrDouble(IBase obj, string attrName)
        {
            var s = GetAttr(obj, attrName);
            return s != null && double.TryParse(s,
                System.Globalization.NumberStyles.Any,
                System.Globalization.CultureInfo.InvariantCulture, out var v) ? v : null;
        }

        private static int? GetAttrInt(IBase obj, string attrName)
        {
            var s = GetAttr(obj, attrName);
            return s != null && int.TryParse(s, out var v) ? v : null;
        }

        private static bool? GetAttrBool(IBase obj, string attrName)
        {
            var s = GetAttr(obj, attrName);
            if (s == null) return null;
            return s == "1" || s.Equals("true", StringComparison.OrdinalIgnoreCase)
                || s.Equals("yes", StringComparison.OrdinalIgnoreCase);
        }

        private static string? GetLinkedObjectCuid(IBase obj, string classPrefix)
        {
            try
            {
                foreach (IBase linked in obj.Links)
                    if (IsClassMatch(linked, classPrefix))
                        return linked.CosId;
                return null;
            }
            catch { return null; }
        }

        private static List<ComosDocRef> GetLinkedDocuments(IBase obj)
        {
            var result = new List<ComosDocRef>();
            try
            {
                foreach (IBase linked in obj.Links)
                {
                    if (IsClassMatch(linked, "@D"))
                    {
                        result.Add(new ComosDocRef(
                            DocumentCuid:   linked.CosId,
                            DocumentType:   linked.SystemClass?.Name ?? "DOC",
                            DocumentNumber: GetAttr(linked, "CTA_DocumentNumber"),
                            Revision:       GetAttr(linked, "CTA_Revision")
                        ));
                    }
                }
            }
            catch { }
            return result;
        }

        private static string GetComosVersion()
        {
            try
            {
                var asm = System.Reflection.Assembly.GetAssembly(typeof(IProject));
                return asm?.GetName().Version?.ToString() ?? "UNKNOWN";
            }
            catch { return "UNKNOWN"; }
        }
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Entry point
    // ──────────────────────────────────────────────────────────────────────────

    public class Program
    {
        [STAThread]
        static int Main(string[] args)
        {
            var outputPath = args.Length > 0 ? args[0] : "comos-export.json";

            try
            {
                // Connect to running COMOS instance
                var comos = new ComosApplication();
                if (!comos.IsConnected)
                {
                    Console.Error.WriteLine(
                        "ERROR: COMOS is not running. Start COMOS and open a project.");
                    return 1;
                }

                var project = comos.ActiveProject;
                if (project == null)
                {
                    Console.Error.WriteLine("ERROR: No active COMOS project.");
                    return 1;
                }

                var exporter = new ComosExporter(project);
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
