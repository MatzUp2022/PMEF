# Third-Party Notices

This file documents the third-party standards, data sources, and trademarks
referenced by the PMEF project.

---

## Standards-Derived Catalog Data

The reference catalogs in the `catalogs/` directory contain factual dimensional
values (nominal sizes, wall thicknesses, flange dimensions, section properties,
etc.) derived from the following published industry standards. The data has been
reformatted into PMEF's JSON catalog schema for machine-readable
interoperability purposes.

| Catalog File | Source Standard(s) |
|---|---|
| `pipe-dimensions.json` | ASME B36.10M (Welded and Seamless Wrought Steel Pipe), ASME B36.19M (Stainless Steel Pipe) |
| `flange-dimensions.json` | ASME B16.5 (Pipe Flanges and Flanged Fittings) |
| `piping-class-a1a2.json` | ASME B16.5, ASME B16.9 (Factory-Made Wrought Buttwelding Fittings), ASME B16.11 (Forged Fittings, Socket-Welding and Threaded) |
| `piping-class-b3c1.json` | ASME B16.5, ASME B16.9 |
| `profiles-en.json` | EN 10034, EN 10055, EN 10056-1, EN 10058, EN 10210-2, EN 10219-2, EN 10279 |
| `profiles-aisc.json` | AISC Steel Construction Manual, 16th Edition |
| `rdl-uri-map.json` | ISO 15926-4 via PCA Reference Data Library (public SPARQL endpoint) |
| `materials-en.json` | EN 10025-2:2019 (Structural steels), EN 10028-2:2017 (Pressure vessel steels), EN 10028-7:2016 (Stainless steels), EN 10216-1/2:2013 (Seamless tubes), EN 10217-1:2019 (Welded tubes), EN 10222-2:2017 (Forgings) |
| `piping-class-en-p1a.json` | EN 1092-1:2018 (Flanges), EN 10253-2 (BW fittings), EN 10216-2 (Seamless tubes), EN 10220 (Pipe dimensions), EN 1514-2 (Gaskets), EN 1984 (Gate valves), EN 12334 (Check valves), EN 13789 (Globe valves), EN 17292 (Ball valves) |
| `caesarII-cii-mapping.json` | PMEF-internal mapping (field correspondences only) |

### Factual Data Statement

The dimensional data in these catalogs consists of individual factual values
(e.g., nominal diameters, wall thicknesses, flange outside diameters, section
moduli) that are not subject to copyright protection. The data has been
independently reformatted into PMEF's own JSON schema structure for the purpose
of engineering data interoperability.

### Disclaimer

**These catalog files are not a substitute for the official published
standards.** Users should always verify dimensional and design data against the
current editions of the applicable standards for engineering and construction
purposes. The PMEF project makes no warranty as to the accuracy or completeness
of the data contained in these files.

---

## Trademarks

The following names are trademarks or registered trademarks of their respective
owners and are used in this project solely for identification and
interoperability purposes:

| Name | Owner |
|---|---|
| CAESAR II | Hexagon AB |
| ROHR2 | SIGMA Ingenieurgesellschaft mbH |
| Autodesk, AutoCAD, Plant 3D, Inventor, Revit, Navisworks, Advance Steel | Autodesk, Inc. |
| Tekla Structures | Trimble Inc. |
| COMOS | Siemens AG |
| AVEVA E3D, AVEVA PDMS | AVEVA Group plc |
| Smart 3D, SmartPlant | Hexagon AB |
| CADMATIC | CADMATIC Oy |
| Bentley OpenPlant | Bentley Systems, Inc. |
| Creo | PTC Inc. |

## Non-Affiliation

PMEF is an independent, community-driven open specification. It is **not
affiliated with, endorsed by, or sponsored by** any of the standards bodies,
software vendors, or trademark holders listed above.

---

## Other References

| Resource | Usage |
|---|---|
| ISO 15926-14 (Industrial Data Ontology) | Upper ontology alignment for semantic grounding |
| DEXPI 2.0 | Extended alignment for P&ID data exchange |
| CFIHOS V2.0 | Property set alignment for handover specifications |
| IEC 81346 | Reference designation convention |
| Contributor Covenant v2.1 | Code of Conduct (adapted, see `CODE_OF_CONDUCT.md`) |
