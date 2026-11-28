#!/usr/bin/env python3
"""Writes tests/plant3d/pds-synthetic.dcf: a tiny synthetic Plant 3D project database (CC0).

Same table and column names as a Plant 3D `Piping.dcf`, invented content: one line with a
pipe, a butt weld, an elbow and a pump nozzle, plus one steel column and a grating.
Used by the `pds` tests in pmef-adapter-plant3d. Run from the repository root.
"""
import os
import sqlite3
import uuid

OUT = os.path.join(os.path.dirname(__file__), "pds-synthetic.dcf")
if os.path.exists(OUT):
    os.remove(OUT)
c = sqlite3.connect(OUT)
x = c.execute
x("PRAGMA page_size=1024")  # small pages: the long text below spans overflow pages

x('CREATE TABLE "PnPTables"("TableName" VarChar (255),"BaseTable" VarChar (255))')
for t, b in [
    ("PnPBase", ""), ("PnPDrawings", "PnPBase"), ("PnPDataLinks", "PnPBase"), ("PnPProject", "PnPBase"),
    ("EngineeringItems", "PnPBase"), ("PipeRunComponent", "EngineeringItems"), ("Pipe", "PipeRunComponent"),
    ("Elbow", "PipeRunComponent"), ("Nozzle", "PipeRunComponent"), ("Fasteners", "EngineeringItems"),
    ("Buttweld", "Fasteners"), ("Equipment", "EngineeringItems"), ("Pump", "Equipment"),
    ("Port", "PnPBase"), ("P3dConnector", "PnPBase"), ("P3dLineGroup", "PnPBase"),
    ("SteelStructure", "PnPBase"), ("StructureMember", "SteelStructure"), ("StructurePlate", "SteelStructure"),
    ("StructureGrating", "StructurePlate"),
]:
    x("INSERT INTO PnPTables VALUES (?,?)", (t, b))

x('CREATE TABLE "PnPBase"("PnPID" integer,"PnPClassName" VarChar (255),"PnPGuid" GUID,Primary key ("PnPID"))')
x('CREATE TABLE "PnPProject"("PnPID" integer,"Project_Name" VarChar (255),"Project_Number" VarChar (255),Primary key ("PnPID"))')
x('CREATE TABLE "PnPDrawings"("PnPID" integer,"Dwg Name" VarChar (255),"PnPRelativePath" VarChar (255),"PnPType" VarChar (255),Primary key ("PnPID"))')
x('CREATE TABLE "PnPDataLinks"("RowId" int,"DwgId" int,"DwgHandleLow" int,"DwgHandleHigh" int,"DwgSubIndex" int,"RowClassName" VarChar (255),"PnPID" integer,Primary key ("PnPID"))')
x('CREATE TABLE "EngineeringItems"("PnPID" integer,"Tag" VarChar (255),"PartFamilyLongDesc" VarChar (255),"Spec" VarChar (255),'
  '"Weight" float,"WeightUnit" VarChar (255),"ContentIsoSymbolDefinition" VarChar (255),"PartSizeLongDesc" VarChar (4000),"PortName" VarChar (255),"NominalDiameter" float,"NominalUnit" VarChar (255),"EndType" VarChar (255),"Position X" float,"Position Y" float,"Position Z" float,"SizeRecordId" GUID,Primary key ("PnPID"))')
x('CREATE TABLE "PipeRunComponent"("PnPID" integer,"LineNumberTag" VarChar (255),"Shop_Field" VarChar (255),Primary key ("PnPID"))')
x('CREATE TABLE "Pipe"("PnPID" integer,"Length" float,"CutLength" float,Primary key ("PnPID"))')
x('CREATE TABLE "Elbow"("PnPID" integer,"PathAngle" float,"CurveRadius" float,Primary key ("PnPID"))')
x('CREATE TABLE "Nozzle"("PnPID" integer,"Number" VarChar (255),Primary key ("PnPID"))')
x('CREATE TABLE "Buttweld"("PnPID" integer,Primary key ("PnPID"))')
x('CREATE TABLE "Equipment"("PnPID" integer,"Tag" VarChar (255),Primary key ("PnPID"))')
x('CREATE TABLE "Port"("PnPID" integer,"PortName" VarChar (255),"NominalDiameter" float,"NominalUnit" VarChar (255),"EndType" VarChar (255),Primary key ("PnPID"))')
x('CREATE TABLE "PartPort"("PnPID" integer,"Part" int,"Port" int,"Name" VarChar (4000),Primary key ("PnPID"))')
x('CREATE TABLE "P3dPartConnection"("PnPID" integer,"Part1" int,"Part2" int,"Port1" VarChar (4000),"Port2" VarChar (4000),Primary key ("PnPID"))')
x('CREATE TABLE "P3dLineGroup"("PnPID" integer,"Tag" VarChar (255),"Service" VarChar (255),Primary key ("PnPID"))')
x('CREATE TABLE "P3dLineGroupPartRelationship"("PnPID" integer,"LineGroup" int,"Part" int,Primary key ("PnPID"))')
x('CREATE TABLE "StructureMember"("PnPID" integer,"Size" VarChar (255),"Length" float,"Materialcode" VarChar (255),"MaterialStandard" VarChar (255),'
  '"Start X" float,"Start Y" float,"Start Z" float,"End X" float,"End Y" float,"End Z" float,Primary key ("PnPID"))')

def base(i, cls):
    x("INSERT INTO PnPBase VALUES (?,?,?)", (i, cls, uuid.UUID(int=i).bytes_le))

x("INSERT INTO PnPProject VALUES (1,'Synthetic demo','SYN-1')")
x("INSERT INTO PnPDrawings VALUES (2,'Area A','Models\\Area A','FOLDER')")
x("INSERT INTO PnPDrawings VALUES (3,'pipe.dwg','Models\\Area A\\pipe.dwg','DWG')")
x("INSERT INTO PnPDrawings VALUES (4,'steel.dwg','Models\\Area A\\steel.dwg','DWG')")

# A part is its own first port (EngineeringItems.PortName …); PartPort lists the others.
# Line, pipe 10 – connector 20 (weld 21) – elbow 30 – connector 40 – pump 50 nozzle 51.
base(5, "P3dLineGroup"); x("INSERT INTO P3dLineGroup VALUES (5,'L-100','CW')")
for i, cls in [(10, "Pipe"), (20, "P3dConnector"), (21, "Buttweld"), (30, "Elbow"), (40, "P3dConnector"),
               (50, "Pump"), (51, "Nozzle"), (60, "StructureMember"), (70, "StructureGrating")]:
    base(i, cls)
# The pipe and the elbow point at their part sizes in tests/plant3d/spec-synthetic.pspc.
x("INSERT INTO EngineeringItems VALUES (10,NULL,'Pipe DIN','16HC01',30.0,'kg','TYPE=PIPE',?,?,?,?,?,500,0,0,?)", ("Pipe 219.1x6.3 " * 200, "S1", 200, "mm", "BV", uuid.UUID(int=0x9001).bytes_le))
x("INSERT INTO EngineeringItems VALUES (21,NULL,'Butt weld','16HC01',NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL)")
x("INSERT INTO EngineeringItems VALUES (30,NULL,'Bend 90','16HC01',8.5,'kg','SKEY=BEBW,TYPE=BEND',NULL,'S1',8,'in','BV',1305,0,0,?)", (uuid.UUID(int=0x9002).bytes_le,))
x("INSERT INTO EngineeringItems VALUES (50,NULL,'Centrifugal pump',NULL,250,'kg',NULL,NULL,NULL,NULL,NULL,NULL,1305,800,0,NULL)")
x("INSERT INTO EngineeringItems VALUES (51,NULL,'Nozzle',NULL,NULL,NULL,NULL,NULL,'N1',200,'mm','FL',1305,305,0,NULL)")
x("INSERT INTO PipeRunComponent VALUES (10,'L-100','SHOP')")
x("INSERT INTO PipeRunComponent VALUES (30,'L-100','SHOP')")
x("INSERT INTO Pipe VALUES (10,1000,1000)")
x("INSERT INTO Elbow VALUES (30,90,305)")
x("INSERT INTO Nozzle VALUES (51,'1')")
x("INSERT INTO Equipment VALUES (50,'P-100')")
x("INSERT INTO StructureMember VALUES (60,'HEA-200',3000,'S235JR','EN 10025',0,0,0,0,0,3000)")

pid = 100
for part, name, et in [(10, "S2", "BV"), (30, "S2", "FL")]:
    pid += 1
    base(pid, "Port"); x("INSERT INTO Port VALUES (?,?,200,'mm',?)", (pid, name, et))
    x("INSERT INTO PartPort VALUES (?,?,?,?)", (pid + 1000, part, pid, name))
for k, (a, pa, b, pb) in enumerate([(10, "S2", 20, "S1"), (30, "S1", 20, "S2"), (30, "S2", 40, "S1"), (51, "N1", 40, "S2")]):
    x("INSERT INTO P3dPartConnection VALUES (?,?,?,?,?)", (200 + k, a, b, pa, pb))
for k, part in enumerate([10, 20, 21, 30, 40]):
    x("INSERT INTO P3dLineGroupPartRelationship VALUES (?,5,?)", (300 + k, part))
for k, (row, dwg, h, s, cls) in enumerate([(10, 3, 0x1A0, 0, "Pipe"), (20, 3, 0x1A1, 0, "P3dConnector"), (21, 3, 0x1A1, 1, "Buttweld"),
                                          (30, 3, 0x1A2, 0, "Elbow"), (40, 3, 0x1A3, 0, "P3dConnector"), (50, 3, 0x1A4, 0, "Pump"),
                                          (51, 3, 0x1A4, 1, "Nozzle"), (60, 4, 0x200, 0, "StructureMember"), (70, 4, 0x201, 0, "StructureGrating")]):
    x("INSERT INTO PnPDataLinks VALUES (?,?,?,0,?,?,?)", (row, dwg, h, s, cls, 400 + k))
c.commit()
x("VACUUM")
c.close()
print("wrote", OUT)
