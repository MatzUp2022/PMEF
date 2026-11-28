#!/usr/bin/env python3
"""Writes tests/plant3d/spec-synthetic.pspc: a tiny synthetic Plant 3D spec sheet (CC0).

Same table and column names as a Plant 3D `<spec>.pspc`, invented content: spec 16HC01 with a
DN 200 pipe and elbow (the size records the parts of pds-synthetic.dcf point at), a reducer
DN 200 x 150, a flange, and a valve actuator (not a spec entry).
Used by the `spec` tests in pmef-adapter-plant3d. Run from the repository root.
"""
import os
import sqlite3
import uuid

OUT = os.path.join(os.path.dirname(__file__), "spec-synthetic.pspc")
if os.path.exists(OUT):
    os.remove(OUT)
c = sqlite3.connect(OUT)
x = c.execute
x("PRAGMA page_size=1024")

x('CREATE TABLE "PnPTables"("TableName" VarChar (255),"BaseTable" VarChar (255))')
for t, b in [
    ("PnPBase", ""), ("RepositoryDescriptor", "PnPBase"), ("EngineeringItems", "PnPBase"),
    ("PipeRunComponent", "EngineeringItems"), ("Pipe", "PipeRunComponent"), ("Elbow", "PipeRunComponent"),
    ("Reducer", "PipeRunComponent"), ("Flange", "PipeRunComponent"), ("ValveActuator", "PipeRunComponent"),
    ("Port", "PnPBase"),
]:
    x("INSERT INTO PnPTables VALUES (?,?)", (t, b))

x('CREATE TABLE "PnPBase"("PnPID" integer,"PnPClassName" VarChar (255),"PnPGuid" GUID,Primary key ("PnPID"))')
x('CREATE TABLE "RepositoryDescriptor"("PnPID" integer,"Name" VarChar (255),"Description" VarChar (255),"Version" VarChar (255),Primary key ("PnPID"))')
x('CREATE TABLE "EngineeringItems"("PnPID" integer,"PartFamilyLongDesc" VarChar (255),"PartSizeLongDesc" VarChar (255),'
  '"CompatibleStandard" VarChar (255),"Material" VarChar (255),"Weight" float,"WeightUnit" VarChar (255),"SizeRecordId" GUID,'
  '"PortName" VarChar (255),"NominalDiameter" float,"NominalUnit" VarChar (255),"MatchingPipeOd" float,"EndType" VarChar (255),'
  '"Facing" VarChar (255),"PressureClass" VarChar (255),"WallThickness" float,"ContentGeometryParamDefinition" VarChar (255),'
  '"ContentIsoSymbolDefinition" VarChar (255),Primary key ("PnPID"))')
x('CREATE TABLE "PipeRunComponent"("PnPID" integer,Primary key ("PnPID"))')
x('CREATE TABLE "Pipe"("PnPID" integer,"Length" float,Primary key ("PnPID"))')
x('CREATE TABLE "Elbow"("PnPID" integer,"PathAngle" float,"CurveRadius" float,Primary key ("PnPID"))')
x('CREATE TABLE "Port"("PnPID" integer,"PortName" VarChar (255),"NominalDiameter" float,"NominalUnit" VarChar (255),"MatchingPipeOd" float,"EndType" VarChar (255),Primary key ("PnPID"))')
x('CREATE TABLE "PartPort"("PnPID" integer,"Part" int,"Port" int,"Name" VarChar (4000),Primary key ("PnPID"))')


def base(i, cls):
    x("INSERT INTO PnPBase VALUES (?,?,?)", (i, cls, uuid.UUID(int=i).bytes_le))


def size(n):
    return uuid.UUID(int=n).bytes_le


base(1, "RepositoryDescriptor")
x("INSERT INTO RepositoryDescriptor VALUES (1,'16HC01','Carbon steel, PN 16 (synthetic)','5_0')")

parts = [
    (10, "Pipe", "Pipe EN 10216", "Pipe 219.1x6.3", "EN 10216-2", "P235GH", 0x9001, "S1", 200, 219.1, "BV", None, None, 6.3, "D=219.1,L=-1", "TYPE=PIPE"),
    (11, "Elbow", "Bend 90 3D", "Bend 90 219.1x6.3", "EN 10253-2", "P235GH", 0x9002, "S1", 200, 219.1, "BV", None, None, 6.3, "A=90,D=219.1,R=305", "SKEY=BEBW,TYPE=BEND"),
    (12, "Reducer", "Reducer concentric", "Reducer 219.1x168.3", "EN 10253-2", "P235GH", 0x9003, "S1", 200, 219.1, "BV", None, None, 6.3, None, "SKEY=RCBW,TYPE=REDUCER"),
    (13, "Flange", "Weld neck flange", "Flange WN DN200 PN16", "EN 1092-1", "P250GH", 0x9004, "S1", 200, 219.1, "FL", "RF", "PN16", None, None, "SKEY=FLWN,TYPE=FLANGE"),
    (14, "ValveActuator", "Hand wheel", "Hand wheel 200", None, None, 0x9005, None, None, None, None, None, None, None, None, None),
]
for (i, cls, fam, desc, std, mat, sid, port, dn, od, et, face, pn, wt, geo, iso) in parts:
    base(i, cls)
    x("INSERT INTO EngineeringItems VALUES (?,?,?,?,?,?,'kg',?,?,?,'mm',?,?,?,?,?,?,?)",
      (i, fam, desc, std, mat, 10.0 + i, size(sid), port, dn, od, et, face, pn, wt, geo, iso))
    if cls not in ("Flange", "ValveActuator"):
        x("INSERT INTO PipeRunComponent VALUES (?)", (i,))
x("INSERT INTO Pipe VALUES (10,0)")
x("INSERT INTO Elbow VALUES (11,90,305)")

pid = 100
for part, name, dn, od, et in [(10, "S2", 200, 219.1, "BV"), (11, "S2", 200, 219.1, "BV"), (12, "S2", 150, 168.3, "BV"),
                               (13, "S2", 200, 219.1, "BV")]:
    pid += 1
    base(pid, "Port")
    x("INSERT INTO Port VALUES (?,?,?,'mm',?,?)", (pid, name, dn, od, et))
    x("INSERT INTO PartPort VALUES (?,?,?,?)", (pid + 1000, part, pid, name))
c.commit()
c.close()
print("wrote", OUT)
