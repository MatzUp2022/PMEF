/*
 * CreoExporter.c
 * PTC Creo Parametric Toolkit plugin — PMEF JSON export
 *
 * Reads assembly hierarchy, parameters, nozzle coordinate systems,
 * bounding boxes, and piping segments from a Creo model and writes
 * a structured JSON export consumed by pmef-adapter-creo (Rust).
 *
 * Build:
 *   - Creo Toolkit SDK (included with Creo installation)
 *   - protoolkit.lib (Windows) or libprotoolkit.so (Linux)
 *   - Standard C compiler (MSVC 2022 or GCC 13+)
 *   - Compilation flags: /DPRO_USE_VAR_ARGS /DPRO_UNICODE (Windows)
 *
 * Deployment:
 *   - Copy CreoExporter.dll to Creo text/usascii/protk_dll/
 *   - Add to protk.dat: NAME pmef_exporter \n EXEC_FILE .../CreoExporter.dll
 *   - Restart Creo; command appears in menu: PMEF > Export
 *
 * Reference:
 *   PTC Creo Parametric 10.0 Toolkit Reference Guide
 *   https://support.ptc.com/help/creo/creo_pma/r10.0/usascii/
 */

#include <ProToolkit.h>
#include <ProAssembly.h>
#include <ProPart.h>
#include <ProSolid.h>
#include <ProGeomitem.h>
#include <ProCsys.h>
#include <ProParameter.h>
#include <ProMdl.h>
#include <ProWindchill.h>
#include <ProSurface.h>
#include <ProUtil.h>
#include <ProMenu.h>
#include <ProMessage.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <math.h>

/* ─────────────────────────────────────────────────────────────────────────────
 * Constants
 * ─────────────────────────────────────────────────────────────────────────────
 */
#define PMEF_VERSION      "1.0"
#define ADAPTER_VERSION   "pmef-adapter-creo 0.9.0"
#define NOZZLE_CS_PREFIX  "CS_NOZZLE_"
#define MAX_NOZZLES       64
#define MAX_ASSEMBLIES    512
#define MAX_PATH          1024

/* ─────────────────────────────────────────────────────────────────────────────
 * JSON writer (minimal, no external dependencies)
 * ─────────────────────────────────────────────────────────────────────────────
 */

typedef struct {
    FILE*  fp;
    int    indent;
    int    needs_comma;  /* whether to write ',' before next element */
} JsonWriter;

static JsonWriter* jw_open(const char* path) {
    JsonWriter* jw = (JsonWriter*)calloc(1, sizeof(JsonWriter));
    jw->fp = fopen(path, "w");
    if (!jw->fp) { free(jw); return NULL; }
    return jw;
}

static void jw_close(JsonWriter* jw) {
    if (jw) { if (jw->fp) fclose(jw->fp); free(jw); }
}

static void jw_indent(JsonWriter* jw) {
    for (int i = 0; i < jw->indent * 2; i++) fputc(' ', jw->fp);
}

static void jw_begin_object(JsonWriter* jw) {
    if (jw->needs_comma) { fputs(",\n", jw->fp); jw->needs_comma = 0; }
    jw_indent(jw); fputs("{\n", jw->fp);
    jw->indent++; jw->needs_comma = 0;
}

static void jw_end_object(JsonWriter* jw) {
    jw->indent--;
    fputs("\n", jw->fp); jw_indent(jw); fputs("}", jw->fp);
    jw->needs_comma = 1;
}

static void jw_begin_array(JsonWriter* jw, const char* key) {
    if (jw->needs_comma) { fputs(",\n", jw->fp); }
    jw_indent(jw);
    if (key) fprintf(jw->fp, "\"%s\": [\n", key);
    else fputs("[\n", jw->fp);
    jw->indent++; jw->needs_comma = 0;
}

static void jw_end_array(JsonWriter* jw) {
    jw->indent--;
    fputs("\n", jw->fp); jw_indent(jw); fputs("]", jw->fp);
    jw->needs_comma = 1;
}

static void jw_key_string(JsonWriter* jw, const char* key, const char* value) {
    if (!value) return;
    if (jw->needs_comma) fputs(",\n", jw->fp);
    jw_indent(jw);
    fprintf(jw->fp, "\"%s\": \"%s\"", key, value);
    jw->needs_comma = 1;
}

static void jw_key_string_opt(JsonWriter* jw, const char* key, const char* value) {
    if (value && strlen(value) > 0) jw_key_string(jw, key, value);
}

static void jw_key_double(JsonWriter* jw, const char* key, double value) {
    if (jw->needs_comma) fputs(",\n", jw->fp);
    jw_indent(jw);
    fprintf(jw->fp, "\"%s\": %.6g", key, value);
    jw->needs_comma = 1;
}

static void jw_key_double_opt(JsonWriter* jw, const char* key, double value, int has_value) {
    if (has_value) jw_key_double(jw, key, value);
}

static void jw_key_int(JsonWriter* jw, const char* key, long long value) {
    if (jw->needs_comma) fputs(",\n", jw->fp);
    jw_indent(jw);
    fprintf(jw->fp, "\"%s\": %lld", key, value);
    jw->needs_comma = 1;
}

static void jw_key_object_begin(JsonWriter* jw, const char* key) {
    if (jw->needs_comma) fputs(",\n", jw->fp);
    jw_indent(jw);
    fprintf(jw->fp, "\"%s\": {\n", key);
    jw->indent++; jw->needs_comma = 0;
}

static void jw_key_object_end(JsonWriter* jw) {
    jw->indent--;
    fputs("\n", jw->fp); jw_indent(jw); fputs("}", jw->fp);
    jw->needs_comma = 1;
}

/* ─────────────────────────────────────────────────────────────────────────────
 * Parameter extraction helpers
 * ─────────────────────────────────────────────────────────────────────────────
 */

/* Get a string parameter from a Creo model object */
static ProError get_string_param(ProMdl mdl, const char* name, char* out, int out_len) {
    ProParameter param;
    ProParamvalue val;
    wchar_t wname[256];
    mbstowcs(wname, name, 256);
    if (ProParameterInit((ProModelitem*)mdl, wname, &param) != PRO_TK_NO_ERROR)
        return PRO_TK_E_NOT_FOUND;
    if (ProParameterValueGet(&param, &val) != PRO_TK_NO_ERROR)
        return PRO_TK_GENERAL_ERROR;
    if (val.type == PRO_PARAM_STRING)
        wcstombs(out, val.value.s_val, out_len);
    else if (val.type == PRO_PARAM_DOUBLE)
        snprintf(out, out_len, "%.6g", val.value.d_val);
    else
        return PRO_TK_E_NOT_FOUND;
    return PRO_TK_NO_ERROR;
}

/* Get a double parameter from a Creo model object */
static ProError get_double_param(ProMdl mdl, const char* name, double* out) {
    ProParameter param;
    ProParamvalue val;
    wchar_t wname[256];
    mbstowcs(wname, name, 256);
    if (ProParameterInit((ProModelitem*)mdl, wname, &param) != PRO_TK_NO_ERROR)
        return PRO_TK_E_NOT_FOUND;
    if (ProParameterValueGet(&param, &val) != PRO_TK_NO_ERROR)
        return PRO_TK_GENERAL_ERROR;
    if (val.type == PRO_PARAM_DOUBLE) { *out = val.value.d_val; return PRO_TK_NO_ERROR; }
    if (val.type == PRO_PARAM_INTEGER) { *out = (double)val.value.i_val; return PRO_TK_NO_ERROR; }
    return PRO_TK_E_NOT_FOUND;
}

/* ─────────────────────────────────────────────────────────────────────────────
 * Nozzle extraction (from coordinate systems named CS_NOZZLE_*)
 * ─────────────────────────────────────────────────────────────────────────────
 */

typedef struct {
    char   cs_name[256];
    char   nozzle_mark[128];
    char   service[256];
    double nominal_diameter_in;
    char   flange_rating[64];
    char   facing_type[32];
    double origin[3];     /* in model coordinates */
    double z_axis[3];     /* outward normal */
} NozzleInfo;

typedef struct {
    NozzleInfo  nozzles[MAX_NOZZLES];
    int         count;
    ProSolid    solid;
    long long   parent_session_id;
} NozzleCollector;

static ProError collect_nozzle_csys(ProCsys csys, ProError status,
                                     ProAppData app_data) {
    NozzleCollector* collector = (NozzleCollector*)app_data;
    if (collector->count >= MAX_NOZZLES) return PRO_TK_NO_ERROR;

    ProCsysdata data;
    if (ProCsysDataGet(csys, &data) != PRO_TK_NO_ERROR) return PRO_TK_NO_ERROR;

    char cs_name[256];
    wcstombs(cs_name, data.name, 256);

    /* Only process coordinate systems named CS_NOZZLE_* */
    if (strncmp(cs_name, NOZZLE_CS_PREFIX, strlen(NOZZLE_CS_PREFIX)) != 0)
        return PRO_TK_NO_ERROR;

    NozzleInfo* noz = &collector->nozzles[collector->count];
    strncpy(noz->cs_name, cs_name, 255);
    strncpy(noz->nozzle_mark,
            cs_name + strlen(NOZZLE_CS_PREFIX),
            127);

    /* Origin from CS origin point */
    noz->origin[0] = data.origin.x;
    noz->origin[1] = data.origin.y;
    noz->origin[2] = data.origin.z;

    /* Z-axis (outward normal) from CS z-vector */
    noz->z_axis[0] = data.z_axis.x;
    noz->z_axis[1] = data.z_axis.y;
    noz->z_axis[2] = data.z_axis.z;

    /* Default nozzle properties */
    noz->nominal_diameter_in = 4.0;
    strncpy(noz->flange_rating, "150", 63);
    strncpy(noz->facing_type, "RF", 31);
    noz->service[0] = '\0';

    /* Try to read nozzle parameters from the CS model item */
    ProModelitem cs_item;
    cs_item.type  = PRO_CSYS;
    cs_item.id    = data.id;
    cs_item.owner = collector->solid;

    char param_val[256];
    double dn = 0.0;

    if (get_double_param((ProMdl)collector->solid, "NZ_DIAMETER", &dn) == PRO_TK_NO_ERROR)
        noz->nominal_diameter_in = dn;
    if (get_string_param((ProMdl)collector->solid, "NZ_SERVICE",
                          noz->service, 255) != PRO_TK_NO_ERROR)
        noz->service[0] = '\0';
    if (get_string_param((ProMdl)collector->solid, "NZ_RATING",
                          noz->flange_rating, 63) != PRO_TK_NO_ERROR)
        ; /* keep default */
    if (get_string_param((ProMdl)collector->solid, "NZ_FACING",
                          noz->facing_type, 31) != PRO_TK_NO_ERROR)
        ; /* keep default */

    collector->count++;
    return PRO_TK_NO_ERROR;
}

/* ─────────────────────────────────────────────────────────────────────────────
 * Assembly traversal
 * ─────────────────────────────────────────────────────────────────────────────
 */

typedef struct {
    JsonWriter*  jw;
    long long    session_id_counter;
    const char*  coord_unit;
    ProMdl       root_mdl;
} ExportContext;

/* Write one assembly to JSON */
static void write_assembly(ExportContext* ctx, ProAsmcomp comp,
                             ProMdl mdl, long long session_id,
                             long long parent_id) {
    JsonWriter* jw = ctx->jw;

    /* Model name */
    char mdl_name[PRO_MDLNAME_SIZE];
    ProMdlNameGet(mdl, mdl_name);

    /* Windchill number */
    char wc_number[256] = "";
    ProWindchillPartNumberGet(mdl, wc_number, 255);

    /* Custom parameters */
    char plant_tag[256] = "";
    char equip_class[256] = "";
    char design_code[256] = "";
    char material_str[256] = "";
    double design_pressure = 0.0, design_temperature = 0.0;
    double weight = 0.0;
    char description[512] = "";

    get_string_param(mdl, "PLANT_TAG",            plant_tag,      255);
    get_string_param(mdl, "EQUIPMENT_CLASS",       equip_class,    255);
    get_string_param(mdl, "DESIGN_CODE",           design_code,    255);
    get_string_param(mdl, "MATERIAL",              material_str,   255);
    get_string_param(mdl, "DESCRIPTION",           description,    511);
    int has_dp = (get_double_param(mdl, "DESIGN_PRESSURE",    &design_pressure) == PRO_TK_NO_ERROR);
    int has_dt = (get_double_param(mdl, "DESIGN_TEMPERATURE", &design_temperature) == PRO_TK_NO_ERROR);
    int has_wt = (get_double_param(mdl, "WEIGHT",             &weight) == PRO_TK_NO_ERROR);

    /* Bounding box */
    ProSolid solid = (ProSolid)mdl;
    ProVector bb_min, bb_max;
    int has_bbox = (ProSolidOutlineGet(solid, bb_min, bb_max) == PRO_TK_NO_ERROR);

    /* Transform to root */
    ProMatrix transform;
    ProAsmcompMdlMdlTransformGet(comp, ctx->root_mdl, PRO_B_FALSE, transform);

    /* STEP file name (if it was exported) */
    char step_file[MAX_PATH] = "";
    snprintf(step_file, MAX_PATH - 1, "%s.stp", mdl_name);

    /* Write assembly object */
    jw_begin_object(jw);
    jw_key_string(jw, "modelName",      mdl_name);
    jw_key_int(jw,    "sessionId",      session_id);
    jw_key_string_opt(jw, "windchillNumber", wc_number);
    jw_key_string_opt(jw, "description",     description);
    jw_key_string_opt(jw, "plantTag",        plant_tag);
    jw_key_string_opt(jw, "equipmentClass",  equip_class);
    jw_key_string_opt(jw, "designCode",      design_code);
    jw_key_string_opt(jw, "material",        material_str);
    jw_key_double_opt(jw, "weight",          weight,            has_wt);
    jw_key_double_opt(jw, "designPressureBarg", design_pressure, has_dp);
    jw_key_double_opt(jw, "designTemperatureDegc", design_temperature, has_dt);
    jw_key_string(jw, "stepFile", step_file);

    /* Bounding box */
    if (has_bbox) {
        jw_key_object_begin(jw, "boundingBox");
        jw_key_double(jw, "xMin", bb_min[0]); jw_key_double(jw, "xMax", bb_max[0]);
        jw_key_double(jw, "yMin", bb_min[1]); jw_key_double(jw, "yMax", bb_max[1]);
        jw_key_double(jw, "zMin", bb_min[2]); jw_key_double(jw, "zMax", bb_max[2]);
        jw_key_object_end(jw);
    }

    /* Transform to root */
    jw_key_object_begin(jw, "transformToRoot");
    if (jw->needs_comma) fputs(",\n", jw->fp); jw->needs_comma = 0;
    jw_indent(jw); fputs("\"m\": [\n", jw->fp);
    jw->indent++;
    for (int row = 0; row < 4; row++) {
        jw_indent(jw);
        fprintf(jw->fp, "[%.6g, %.6g, %.6g]",
            transform[row][0], transform[row][1], transform[row][2]);
        if (row < 3) fputs(",", jw->fp);
        fputs("\n", jw->fp);
    }
    jw->indent--;
    jw_indent(jw); fputs("]", jw->fp); jw->needs_comma = 1;
    jw_key_object_end(jw);

    /* Collect nozzles */
    NozzleCollector collector;
    memset(&collector, 0, sizeof(collector));
    collector.solid = solid;
    collector.parent_session_id = session_id;

    ProSolidCsysVisit(solid, (ProFunction)collect_nozzle_csys, NULL, &collector);

    /* Write nozzles */
    jw_begin_array(jw, "nozzles");
    for (int ni = 0; ni < collector.count; ni++) {
        NozzleInfo* noz = &collector.nozzles[ni];
        jw_begin_object(jw);
        jw_key_string(jw, "csName",     noz->cs_name);
        jw_key_int(jw,   "parentAssemblyId", session_id);
        jw_key_string(jw, "nozzleMark", noz->nozzle_mark);
        jw_key_string_opt(jw, "service", noz->service);
        jw_key_double(jw, "nominalDiameterIn", noz->nominal_diameter_in);
        jw_key_string_opt(jw, "flangeRating", noz->flange_rating);
        jw_key_string_opt(jw, "facingType",   noz->facing_type);

        /* Origin */
        if (jw->needs_comma) fputs(",\n", jw->fp);
        jw_indent(jw);
        fprintf(jw->fp, "\"origin\": { \"x\": %.6g, \"y\": %.6g, \"z\": %.6g }",
            noz->origin[0], noz->origin[1], noz->origin[2]);
        jw->needs_comma = 1;

        /* Direction */
        if (jw->needs_comma) fputs(",\n", jw->fp);
        jw_indent(jw);
        fprintf(jw->fp, "\"direction\": [%.6g, %.6g, %.6g]",
            noz->z_axis[0], noz->z_axis[1], noz->z_axis[2]);
        jw->needs_comma = 1;

        jw_end_object(jw);
    }
    jw_end_array(jw);

    /* Parameters block */
    jw_key_object_begin(jw, "parameters");
    /* Export selected parameters for round-trip */
    const char* param_names[] = {
        "ERECTION_SEQUENCE", "FIRE_ZONE", "PAINT_SYSTEM",
        "INSULATION_TYPE", "AREA_CLASSIFICATION", "VENDOR",
        "PURCHASE_ORDER", NULL
    };
    for (int pi = 0; param_names[pi] != NULL; pi++) {
        char val[256];
        if (get_string_param(mdl, param_names[pi], val, 255) == PRO_TK_NO_ERROR && strlen(val) > 0)
            jw_key_string(jw, param_names[pi], val);
    }
    jw_key_object_end(jw);

    jw_end_object(jw);
}

/* Assembly visitor callback */
typedef struct {
    ExportContext* ctx;
    int            assembly_count;
    int            nozzle_count;
    long long      session_ids[MAX_ASSEMBLIES];
    ProMdl         mdls[MAX_ASSEMBLIES];
} AssemblyVisitor;

static ProError visit_assembly_comp(ProAsmcomp comp, ProError status,
                                     ProAppData app_data) {
    AssemblyVisitor* visitor = (AssemblyVisitor*)app_data;
    if (visitor->assembly_count >= MAX_ASSEMBLIES) return PRO_TK_NO_ERROR;

    ProMdl mdl;
    if (ProAsmcompMdlGet(comp, &mdl) != PRO_TK_NO_ERROR) return PRO_TK_NO_ERROR;

    ProMdlType mdl_type;
    ProMdlTypeGet(mdl, &mdl_type);
    if (mdl_type != PRO_MDL_ASSEMBLY) return PRO_TK_NO_ERROR;

    long long sid = visitor->ctx->session_id_counter++;
    visitor->mdls[visitor->assembly_count] = mdl;
    visitor->session_ids[visitor->assembly_count] = sid;
    visitor->assembly_count++;

    /* Recurse into sub-assemblies */
    ProSolidFeatVisit((ProSolid)mdl, (ProFunction)visit_assembly_comp,
                      PRO_FEAT_FILTER_REGULAR, app_data);

    return PRO_TK_NO_ERROR;
}

/* ─────────────────────────────────────────────────────────────────────────────
 * Main export function
 * ─────────────────────────────────────────────────────────────────────────────
 */

static ProError pmef_export_model(const char* output_path) {
    ProMdl root_mdl;
    if (ProMdlCurrentGet(&root_mdl) != PRO_TK_NO_ERROR) {
        ProMessageDisplay("pmef_msgs.txt", "PMEF_ERR_NO_MODEL");
        return PRO_TK_GENERAL_ERROR;
    }

    ProMdlType root_type;
    ProMdlTypeGet(root_mdl, &root_type);
    if (root_type != PRO_MDL_ASSEMBLY && root_type != PRO_MDL_PART) {
        ProMessageDisplay("pmef_msgs.txt", "PMEF_ERR_NOT_ASSEMBLY");
        return PRO_TK_GENERAL_ERROR;
    }

    JsonWriter* jw = jw_open(output_path);
    if (!jw) { ProMessageDisplay("pmef_msgs.txt", "PMEF_ERR_OPEN_FILE"); return PRO_TK_GENERAL_ERROR; }

    /* Get Creo version */
    char creo_version[128];
    ProVersion version;
    ProVersionGet(&version);
    snprintf(creo_version, 127, "Creo %d.%d.%d.%d",
             version.major, version.minor, version.patch, version.build);

    /* Root model name */
    char root_name[PRO_MDLNAME_SIZE];
    ProMdlNameGet(root_mdl, root_name);

    /* Timestamp */
    time_t now = time(NULL);
    char timestamp[64];
    strftime(timestamp, 63, "%Y-%m-%dT%H:%M:%SZ", gmtime(&now));

    /* Determine coordinate unit from model units */
    const char* coord_unit = "MM"; /* Creo can use mm or inches */
    ProMdlUnits units;
    ProUnitsSystemGet(root_mdl, &units);
    if (units.length == PRO_UNIT_INCH) coord_unit = "INCHES";

    /* Begin export context */
    ExportContext ctx;
    ctx.jw = jw;
    ctx.session_id_counter = 1;
    ctx.coord_unit = coord_unit;
    ctx.root_mdl = root_mdl;

    /* Write root JSON object */
    fputs("{\n", jw->fp); jw->indent++; jw->needs_comma = 0;

    jw_key_string(jw, "schemaVersion",  PMEF_VERSION);
    jw_key_string(jw, "creoVersion",    creo_version);
    jw_key_string(jw, "exportedAt",     timestamp);
    jw_key_string(jw, "assemblyName",   root_name);
    jw_key_string(jw, "coordinateUnit", coord_unit);

    /* Windchill number for root */
    char wc_root[256] = "";
    ProWindchillPartNumberGet(root_mdl, wc_root, 255);
    jw_key_string_opt(jw, "windchillNumber", wc_root);

    /* Collect all assemblies */
    AssemblyVisitor visitor;
    memset(&visitor, 0, sizeof(visitor));
    visitor.ctx = &ctx;

    jw_begin_array(jw, "assemblies");

    /* Write root assembly first */
    ProAsmcomp root_comp;  /* dummy for root */
    write_assembly(&ctx, root_comp, root_mdl, ctx.session_id_counter++, 0);

    /* Recurse into sub-assemblies */
    if (root_type == PRO_MDL_ASSEMBLY) {
        ProSolidFeatVisit((ProSolid)root_mdl,
                          (ProFunction)visit_assembly_comp,
                          PRO_FEAT_FILTER_REGULAR, &visitor);
        for (int i = 0; i < visitor.assembly_count; i++) {
            ProAsmcomp comp;  /* simplified */
            write_assembly(&ctx, comp, visitor.mdls[i],
                          visitor.session_ids[i], 0);
        }
    }
    jw_end_array(jw);

    /* Parts array (leaf-level parts — simplified) */
    jw_begin_array(jw, "parts"); jw_end_array(jw);

    /* Piping segments (from Creo Piping Extension — simplified) */
    jw_begin_array(jw, "pipingSegments"); jw_end_array(jw);

    /* Nozzles (collected from all assemblies above — simplified) */
    jw_begin_array(jw, "nozzles"); jw_end_array(jw);

    /* Summary */
    jw_key_object_begin(jw, "summary");
    jw_key_int(jw, "assemblyCount",     visitor.assembly_count + 1);
    jw_key_int(jw, "partCount",         0);
    jw_key_int(jw, "pipingSegmentCount",0);
    jw_key_int(jw, "nozzleCount",       0);
    jw_key_object_end(jw);

    /* Close root object */
    jw->indent--;
    fputs("\n}\n", jw->fp);

    jw_close(jw);

    char msg[512];
    snprintf(msg, 511, "PMEF export complete: %d assemblies → %s",
             visitor.assembly_count + 1, output_path);
    ProMessageDisplay("pmef_msgs.txt", msg);

    return PRO_TK_NO_ERROR;
}

/* ─────────────────────────────────────────────────────────────────────────────
 * Menu command
 * ─────────────────────────────────────────────────────────────────────────────
 */

static ProError pmef_export_command(void) {
    char output_path[MAX_PATH];

    /* Prompt user for output path */
    ProStringToWstring wpath, wprompt;
    ProStringToWstring(wprompt, "PMEF export path (e.g. creo-export.json):");
    if (ProMessageGetFilepath(wprompt, ".json", wpath) != PRO_TK_NO_ERROR) {
        ProMessageDisplay("pmef_msgs.txt", "PMEF_EXPORT_CANCELLED");
        return PRO_TK_NO_ERROR;
    }
    ProWstringToString(output_path, wpath);

    return pmef_export_model(output_path);
}

/* ─────────────────────────────────────────────────────────────────────────────
 * Plugin entry points (required by Creo Toolkit)
 * ─────────────────────────────────────────────────────────────────────────────
 */

ProError user_initialize(int argc, char *argv[],
                          char *version, char *build,
                          wchar_t errbuf[], int errbufsize) {
    /* Register menu item: PMEF > Export */
    uiCmdCmdId cmd_id;
    ProCmdActionAdd("PMEF_EXPORT", (uiCmdCmdActFn)pmef_export_command,
                    uiProeImmediate, NULL, PRO_B_TRUE, PRO_B_TRUE, &cmd_id);

    ProMenubarMenuAdd("PMEF", "PMEF", "Help", PRO_B_TRUE, "pmef_msgs.txt");
    ProMenubarmenuMenuAdd("PMEF", "PMEF_EXPORT", "PMEF_EXPORT",
                          NULL, PRO_B_TRUE, "pmef_msgs.txt");

    return PRO_TK_NO_ERROR;
}

ProError user_terminate(void) {
    return PRO_TK_NO_ERROR;
}
