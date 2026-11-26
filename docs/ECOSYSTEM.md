# Ökosystem: Verantwortung je Repository

Stand: 2026-11-26 · Freigegeben von Marcel · Gilt identisch in allen acht Repositories.

Dieses Dokument legt fest, welches Repository welche Funktion **besitzt**. Ziel ist, dass keine
Funktion in zwei Repositories parallel entsteht. Vor jeder neuen Funktion gilt: Gehört sie laut
Tabelle einem anderen Repository, wird sie dort gebaut (oder dort angefragt), nicht hier.

## Schichten und Abhängigkeitsrichtung

```text
Plattformen   ProDocAimAi (CDE/AIM)            ProcessHubAi (BPM)  --API-->  ProDocAimAi
                   |  WASM-Pakete, REST-API
Engines       DEXPI-Manager   PlantView   cad-engine   EngineeringAI   eng-calc*   electrical*
                   |  Cargo-Crates (Git-Tags)
Kern          PMEF (Informationsmodell, Kataloge)    cad-kernel (exakte Geometrie)
```

`*` = beschlossen, Repository noch anzulegen (Entscheidungen vom 2026-11-26, Nr. 2 und 4).

- Abhängigkeiten zeigen **nur nach unten**. Keine Schicht ruft eine höhere auf.
- Engines kennen einander nur über Daten (PMEF, DEXPI) mit stabilen `@id`s, nicht über interne Typen.
- Rust-Crates werden per Git-Abhängigkeit mit festem Tag eingebunden (Muster: PlantView → PMEF).
- Plattformen binden Engines als vendorte WASM-Pakete ein (Muster: ProDocAimAi ADR 0042).

## Verantwortungsmatrix

| Repo | Besitzt | Baut nicht (Nicht-Scope) | Liefert nach außen |
| --- | --- | --- | --- |
| PMEF | Informationsmodell 3D-Objekte (Piping, Equipment, Steel, HVAC-Kanäle, Kabel und Kabeltrassen inkl. Formteile), JSON-Schemas, Kataloge (Rohrklassen, Flansche, Profile, Werkstoffe, Kanalmaße, Rohrhalterungen, Kabeltypen), NDJSON-I/O, Diff/Merge, Snapping, Auto-Routing-Kernel (`pmef-route`: Rohrleitungen im Graph/Voxel-Raster nach Rohrklassen-Regeln, später Kabel), Adapter zu AVEVA E3D und AutoCAD Plant 3D | Viewer, Datenbank/Server, Workflows, P&ID-Topologie (→ DEXPI-Manager), Konstruktions-UI (→ cad-engine `cad-studio`), Berechnungs-Solver (→ eng-calc), Schaltplan-Modell und Elektro-Berechnung (→ electrical) | Crates `pmef-core`, `pmef-io`, `pmef-validate`, `pmef-geom`, `pmef-catalogs`, `pmef-route`; CLI `pmef route`; Schemas; Katalog-JSON |
| cad-kernel | Exakte Geometrie: NURBS, B-Rep-Topologie, Schnitte, Booleans, Tessellierung | Szene, Rendering, Fachobjekte, Dateiformate | Crate `kernel-api` (Rust + WASM) |
| DEXPI-Manager | DEXPI-Parsing/-Validierung/-Export, P&ID-/PFD-Graph, Regel-Engine, Safety (HAZOP, SIL, LOPA), P/T-Berechnung für Rohrklassen, Adapter zu P&ID-Werkzeugen | Dokumentenmanagement, Transmittals, Revisions-Workflows, Schweißen, Prüfung, Kabel/Elektro, Bau/IBN (→ ProDocAimAi); eigene Rohrklassen-/Maßtabellen (→ PMEF-Kataloge); 3D-Tool-Adapter (→ PMEF); 2D-Ableitung (→ PlantView); Erkennungs-KI (→ EngineeringAI) | Crates `dexpi-core`, `pid-graph`, `rule-engine`, `pipe-class-engine`; WASM `wasm-bindings` |
| PlantView | Viewer-Formate (glTF, IFC, STEP, OBJ/STL, PCF, `.mvcache`), Konvertierung, Streaming, Desktop-/Web-Viewer, 2D-Ableitung (HLR, DXF, PDF, SVG), BIM-Prüfungen (IDS, BCF, bSDD) | Fach-Datenmodelle je Disziplin (→ PMEF), eigener DEXPI-Parser (→ `dexpi-core`), NURBS-/B-Rep-Auswertung (→ cad-kernel), Lifecycle-Module (→ ProDocAimAi), Erkennungs-KI (→ EngineeringAI) | CLI, WASM `mvcache-web`, Geometriequelle für `geom-api` |
| cad-engine | Natives Echtzeit-Rendering großer Modelle (wgpu), Physik, Montage-, Kranhub- und Kinematiksimulation; Konstruktionsprogramm `cad-studio` (interaktives Verlegen und Bearbeiten von Rohrleitungen, Halterungen, Stahl, Kanälen, Trassen; speichert PMEF) | Formatimport außer PMEF/`.mvcache` (→ PlantView), Fachdatenmodelle und Regeln (→ PMEF, `cad-studio` nutzt `pmef-core`/`pmef-route`), exakte Geometrie (→ cad-kernel) | `geom-api`, `cad-render`, `cad-physics`, App `cad-studio` |
| EngineeringAI | Erkennung aus 2D-Zeichnungen, Isometrien, PDF/DWG/DXF/DGN und Punktwolken; Ausgabe PMEF bzw. DEXPI mit Provenienz, Konfidenz, Review-Status | LLM-Chat-/Copilot-Funktionen der Plattformen, eigenes Datenmodell | Erkennungs-Pipeline (CLI/Service) |
| ProDocAimAi | CDE/EDMS, Revisionen, Workflows, Transmittals, Tags/Equipment-Lifecycle, Klassenbibliothek, RDS-/81346-Strukturen, ITP, Handover, AAS/CFIHOS/SAP-Export | Geometrie, Formatparser, P&ID-Logik (kommen als WASM aus den Engines) | Öffentliche REST-API |
| ProcessHubAi | Prozessmanagement, Workflow-Ausführung, Engineering Edition (Deliverable-Abhängigkeiten); Prozessseite der Material-/Logistikfluss-Simulation, Anbindung externer Simulationswerkzeuge per Konnektor | CDE-Funktionen, Dokumentablage, MDR/VDRL, Schedule/EVM (→ ProDocAimAi); eigener Materialfluss-Simulator | REST-API, Konnektor zu ProDocAimAi |
| eng-calc* | Berechnungs-Solver: Rohrstatik (ASME B31.3, EN 13480), Stabwerk (EC3); liest PMEF, schreibt Ergebnisse als PMEF-Eigenschaften zurück | Datenmodell (→ PMEF), Viewer, 1D-Hydraulik (→ DEXPI-Manager), CFD | Crate(s), CLI |
| electrical* | Schaltplan-Modell (Einlinienschaltbild), Kabelberechnung (Querschnitt, Spannungsfall, Kurzschluss nach IEC 60364/60909), Klemmenpläne | Kabel- und Trassenobjekte im 3D-Modell (→ PMEF), Kabelliste/-verfolgung im Projekt (→ ProDocAimAi) | Crate(s), WASM |

## Getroffene Entscheidungen (2026-11-21)

1. **DEXPI-Manager** ist Fachbibliothek plus WASM, nicht weiter eigenständiges Serverprodukt. Der
   Axum-Server bleibt lauffähig, wird aber nicht weiter ausgebaut.
2. **EngineeringAI** wird Heimat der Erkennungs-KI (2D/PDF/DWG/Scans → PMEF/DEXPI).
3. **Nativer Renderer** ist `cad-engine`. PlantView konzentriert sich auf Formate, Streaming und Web.
4. **Rust-Verteilung** zunächst per Git-Abhängigkeit mit Tags; private Registry erst ab mehr als
   drei Konsumenten.
5. **ProDocAimAi ↔ ProcessHubAi** teilen Code weiterhin per Kopie (ProcessHubAi CLAUDE.md §7a); ein
   gemeinsames Paket erst, wenn ein drittes TS-Produkt es braucht.
6. **Auto-Routing** (2026-11-25): Der Graph- und Regel-Kernel für automatisches Routing von
   Rohrleitungen (später Kabel) ist das Crate `pmef-route` in PMEF, weil er Rohrklassen, Rohrmaße und
   das PMEF-Modell braucht. Engines und Plattformen binden ihn wie `pmef-core` ein; Viewer zeigen
   das Ergebnis, bauen aber keinen eigenen Router.

## Getroffene Entscheidungen (2026-11-26)

1. **Konstruktionsprogramm** ist die neue App `cad-studio` in **cad-engine** (nativer Renderer und
   Picking sind dort, Geometrie aus cad-kernel, Daten und Routing aus PMEF). Kein neues Repository.
2. **Berechnungs-Solver** (Rohrstatik, Stabwerk) entstehen im neuen Engine-Repository **eng-calc**.
   Bis dahin Export nach CAESAR II bzw. RFEM/RSTAB aus PMEF. 1D-Hydraulik bleibt in DEXPI-Manager.
3. **Material-/Logistikfluss-Simulation** wird nicht selbst gebaut: Prozessseite in ProcessHubAi,
   Simulation über Anbindung eines Fremdwerkzeugs per Konnektor.
4. **Elektro-Engine** (Einlinienschaltbild, Kabelberechnung, Klemmenpläne) entsteht im neuen
   Engine-Repository **electrical**. Kabel- und Trassenobjekte bleiben in PMEF. Die eingefrorenen
   Elektro-Module in PlantView und DEXPI-Manager werden danach abgelöst (Freigabe je Modul).

## Eingefroren (nur Bugfixes, kein Ausbau)

- DEXPI-Manager: `adapters/document_engine`, `revision_workflow`, `inspection_engine`,
  `welding_engine`, `construction_engine`, `electrical_engine`, `isometric_drawing`, `dxf_writer`,
  `step_ifc_import`, `native_cad_import`, `pdf_extraction`, `pdf_pipeline`, `legacy_pdf_dwg`;
  `dexpi-core` ML/ONNX-Module; neue Server-Endpunkte außerhalb P&ID/PFD.
- PlantView: Disziplin-Module in `mvcache-core` (u. a. `commissioning`, `construction`, `handover`,
  `quality`, `review`, `document`, `cable`, `telecom`, `hse`, `cost`); `mvcache-ai`; Erweiterungen
  der NURBS-/B-Rep-Auswertung im STEP-Import; `mvcache-render` (wird von `cad-render` abgelöst).

Entfernt wird eingefrorener Code erst, wenn das zuständige Repo die Funktion abdeckt, und nur nach
Freigabe je Modul.

## Konsolidierungs-Roadmap

1. **Stufe 0 – Scope-Governance:** dieses Dokument + Nicht-Scope-Abschnitte in jeder `CLAUDE.md`.
2. **Stufe 1 – Ein DEXPI-Parser:** PlantView importiert DEXPI über `dexpi-core`; vorher Differenztest
   beider Parser auf den DEXPI-Fixtures.
3. **Stufe 2 – PMEF als Datenmodell-Hub:** Mapping `mvcache-core` ↔ PMEF (Piping, Equipment, Steel);
   `pipe-class-engine` liest PMEF-Kataloge; DEXPI ↔ PMEF-Mapping über stabile `@id`s.
4. **Stufe 3 – Gemeinsame Testvektoren:** RDS/81346-Kennzeichen, DEXPI-Dateien, PMEF-Beispiele, gegen
   die TS- und Rust-Implementierungen laufen; WASM-Sync in ProDocAimAi per CI.
5. **Stufe 4 – Geometrie und Rendering bündeln:** nach cad-kernel Schicht 2 läuft PlantView-STEP über
   cad-kernel; `cad-render` löst `mvcache-render` ab.
6. **Stufe 5 – Erkennungs-KI in EngineeringAI** (Umzug aus DEXPI-Manager und `mvcache-ai`).
7. **Stufe 6 – Lifecycle in ProDocAimAi bündeln;** eingefrorene Module entfernen (Freigabe je Modul).

## Pflege

Änderungen an der Matrix werden in allen acht Repositories gleichzeitig vorgenommen. Bei Konflikt
zwischen diesem Dokument und einer repo-lokalen Planung gilt dieses Dokument; den Konflikt im PR
benennen.
