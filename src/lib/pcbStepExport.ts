// ===============================================================
// STEP (ISO 10303-21) Export — Board body geometry
// ===============================================================

import type { PcbData, PcbPoint, PcbFootprint } from "@/types/pcb";

// --- STEP entity ID allocator ---

class StepIdAllocator {
  private next = 1;

  /** Allocate the next available entity ID. */
  alloc(): number {
    return this.next++;
  }

  /** Return current count of allocated IDs. */
  get count(): number {
    return this.next - 1;
  }
}

// --- Coordinate formatting ---

/** Format a number for STEP output (6 decimal places). */
function sf(v: number): string {
  return v.toFixed(6);
}

/** Format a 3D point tuple for STEP. */
function pt3d(x: number, y: number, z: number): string {
  return `(${sf(x)},${sf(y)},${sf(z)})`;
}

/** Format a direction tuple for STEP. */
function dir3d(x: number, y: number, z: number): string {
  return `(${sf(x)},${sf(y)},${sf(z)})`;
}

// --- Board outline utilities ---

/** Ensure the outline polygon winds counter-clockwise (for STEP convention). */
function ensureCCW(points: PcbPoint[]): PcbPoint[] {
  // Compute signed area using the shoelace formula
  let area = 0;
  for (let i = 0; i < points.length; i++) {
    const j = (i + 1) % points.length;
    area += points[i].x * points[j].y;
    area -= points[j].x * points[i].y;
  }
  // If area is negative, the polygon is clockwise — reverse it
  if (area < 0) return [...points].reverse();
  return points;
}

/**
 * Generate a minimal STEP AP214 file containing the PCB board body.
 *
 * The board outline is extruded from z=0 to z=thickness as a BREP solid.
 * Component placements are recorded as PRODUCT_DEFINITION_PLACEMENT entries
 * referencing their positions and rotations (actual 3D models are not embedded
 * since those require external STEP model files).
 */
export function generateStepFile(data: PcbData): string {
  const ids = new StepIdAllocator();
  const entities: string[] = [];

  const outline = data.board.outline.length >= 3
    ? ensureCCW(data.board.outline)
    : [{ x: 0, y: 0 }, { x: 100, y: 0 }, { x: 100, y: 80 }, { x: 0, y: 80 }];

  const thickness = data.board.thickness || 1.6;
  const boardName = data.board.generator || "board";
  const dateStr = new Date().toISOString().slice(0, 10);

  // =============================================================
  // Foundation entities: application context, units, etc.
  // =============================================================

  const idAppCtx = ids.alloc();
  entities.push(`#${idAppCtx}=APPLICATION_CONTEXT('automotive design');`);

  const idAppProto = ids.alloc();
  entities.push(`#${idAppProto}=APPLICATION_PROTOCOL_DEFINITION('international standard','automotive_design',2010,#${idAppCtx});`);

  const idProdCtx = ids.alloc();
  entities.push(`#${idProdCtx}=PRODUCT_CONTEXT('',#${idAppCtx},'mechanical');`);

  const idProdDefCtx = ids.alloc();
  entities.push(`#${idProdDefCtx}=PRODUCT_DEFINITION_CONTEXT('detailed design',#${idAppCtx},'design');`);

  // --- Units ---

  const idLenUnit = ids.alloc();
  entities.push(`#${idLenUnit}=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.));`);

  const idAngUnit = ids.alloc();
  entities.push(`#${idAngUnit}=(NAMED_UNIT(*)PLANE_ANGLE_UNIT()SI_UNIT($,.RADIAN.));`);

  const idSolidAngUnit = ids.alloc();
  entities.push(`#${idSolidAngUnit}=(NAMED_UNIT(*)SI_UNIT($,.STERADIAN.)SOLID_ANGLE_UNIT());`);

  const idUncertaintyMeasure = ids.alloc();
  entities.push(`#${idUncertaintyMeasure}=UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.E-06),#${idLenUnit},'DISTANCE_ACCURACY_VALUE','');`);

  const idGeoRepCtx = ids.alloc();
  entities.push(
    `#${idGeoRepCtx}=(GEOMETRIC_REPRESENTATION_CONTEXT(3)` +
    `GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#${idUncertaintyMeasure}))` +
    `GLOBAL_UNIT_ASSIGNED_CONTEXT((#${idLenUnit},#${idAngUnit},#${idSolidAngUnit}))` +
    `REPRESENTATION_CONTEXT('Context3D','3D Context'));`
  );

  // =============================================================
  // Product definition
  // =============================================================

  const idProduct = ids.alloc();
  entities.push(`#${idProduct}=PRODUCT('${boardName}','${boardName} PCB','',(#${idProdCtx}));`);

  const idProdDefForm = ids.alloc();
  entities.push(`#${idProdDefForm}=PRODUCT_DEFINITION_FORMATION('','',#${idProduct});`);

  const idProdDef = ids.alloc();
  entities.push(`#${idProdDef}=PRODUCT_DEFINITION('design','',#${idProdDefForm},#${idProdDefCtx});`);

  // =============================================================
  // Axis placements (shared)
  // =============================================================

  const idOrigin = ids.alloc();
  entities.push(`#${idOrigin}=CARTESIAN_POINT('Origin',${pt3d(0, 0, 0)});`);

  const idDirZ = ids.alloc();
  entities.push(`#${idDirZ}=DIRECTION('Z',${dir3d(0, 0, 1)});`);

  const idDirX = ids.alloc();
  entities.push(`#${idDirX}=DIRECTION('X',${dir3d(1, 0, 0)});`);

  const idDirNegZ = ids.alloc();
  entities.push(`#${idDirNegZ}=DIRECTION('-Z',${dir3d(0, 0, -1)});`);

  const idDirY = ids.alloc();
  entities.push(`#${idDirY}=DIRECTION('Y',${dir3d(0, 1, 0)});`);

  const idAxis2Top = ids.alloc();
  entities.push(`#${idAxis2Top}=AXIS2_PLACEMENT_3D('',#${idOrigin},#${idDirZ},#${idDirX});`);

  // Top face placement (at z = thickness)
  const idOriginTop = ids.alloc();
  entities.push(`#${idOriginTop}=CARTESIAN_POINT('TopOrigin',${pt3d(0, 0, thickness)});`);

  const idAxis2TopFace = ids.alloc();
  entities.push(`#${idAxis2TopFace}=AXIS2_PLACEMENT_3D('',#${idOriginTop},#${idDirZ},#${idDirX});`);

  // Bottom face placement (at z = 0, normal pointing down)
  const idAxis2BotFace = ids.alloc();
  entities.push(`#${idAxis2BotFace}=AXIS2_PLACEMENT_3D('',#${idOrigin},#${idDirNegZ},#${idDirX});`);

  // =============================================================
  // Board outline geometry — vertices, edges, edge loop
  // =============================================================

  const n = outline.length;

  // Create cartesian points for bottom face vertices
  const botPtIds: number[] = [];
  for (let i = 0; i < n; i++) {
    const id = ids.alloc();
    entities.push(`#${id}=CARTESIAN_POINT('v${i}_bot',${pt3d(outline[i].x, outline[i].y, 0)});`);
    botPtIds.push(id);
  }

  // Create cartesian points for top face vertices
  const topPtIds: number[] = [];
  for (let i = 0; i < n; i++) {
    const id = ids.alloc();
    entities.push(`#${id}=CARTESIAN_POINT('v${i}_top',${pt3d(outline[i].x, outline[i].y, thickness)});`);
    topPtIds.push(id);
  }

  // Vertex points
  const botVertexIds: number[] = [];
  for (let i = 0; i < n; i++) {
    const id = ids.alloc();
    entities.push(`#${id}=VERTEX_POINT('',#${botPtIds[i]});`);
    botVertexIds.push(id);
  }

  const topVertexIds: number[] = [];
  for (let i = 0; i < n; i++) {
    const id = ids.alloc();
    entities.push(`#${id}=VERTEX_POINT('',#${topPtIds[i]});`);
    topVertexIds.push(id);
  }

  // --- Bottom face edge loop (outline polygon at z=0) ---

  const botEdgeCurveIds: number[] = [];
  const botOrientedEdgeIds: number[] = [];

  for (let i = 0; i < n; i++) {
    const j = (i + 1) % n;

    // Line geometry
    const idLine = ids.alloc();
    const dx = outline[j].x - outline[i].x;
    const dy = outline[j].y - outline[i].y;
    const len = Math.hypot(dx, dy) || 1;
    const idLineDir = ids.alloc();
    entities.push(`#${idLineDir}=DIRECTION('',${dir3d(dx / len, dy / len, 0)});`);
    const idLineVec = ids.alloc();
    entities.push(`#${idLineVec}=VECTOR('',#${idLineDir},${sf(len)});`);
    entities.push(`#${idLine}=LINE('',#${botPtIds[i]},#${idLineVec});`);

    // Edge curve
    const idEdge = ids.alloc();
    entities.push(`#${idEdge}=EDGE_CURVE('',#${botVertexIds[i]},#${botVertexIds[j]},#${idLine},.T.);`);
    botEdgeCurveIds.push(idEdge);

    // Oriented edge
    const idOrientedEdge = ids.alloc();
    entities.push(`#${idOrientedEdge}=ORIENTED_EDGE('',*,*,#${idEdge},.T.);`);
    botOrientedEdgeIds.push(idOrientedEdge);
  }

  const idBotEdgeLoop = ids.alloc();
  entities.push(`#${idBotEdgeLoop}=EDGE_LOOP('',(${botOrientedEdgeIds.map((id) => `#${id}`).join(",")}));`);

  // --- Top face edge loop (outline polygon at z=thickness) ---

  const topEdgeCurveIds: number[] = [];
  const topOrientedEdgeIds: number[] = [];

  for (let i = 0; i < n; i++) {
    const j = (i + 1) % n;

    const idLine = ids.alloc();
    const dx = outline[j].x - outline[i].x;
    const dy = outline[j].y - outline[i].y;
    const len = Math.hypot(dx, dy) || 1;
    const idLineDir = ids.alloc();
    entities.push(`#${idLineDir}=DIRECTION('',${dir3d(dx / len, dy / len, 0)});`);
    const idLineVec = ids.alloc();
    entities.push(`#${idLineVec}=VECTOR('',#${idLineDir},${sf(len)});`);
    entities.push(`#${idLine}=LINE('',#${topPtIds[i]},#${idLineVec});`);

    const idEdge = ids.alloc();
    entities.push(`#${idEdge}=EDGE_CURVE('',#${topVertexIds[i]},#${topVertexIds[j]},#${idLine},.T.);`);
    topEdgeCurveIds.push(idEdge);

    const idOrientedEdge = ids.alloc();
    entities.push(`#${idOrientedEdge}=ORIENTED_EDGE('',*,*,#${idEdge},.T.);`);
    topOrientedEdgeIds.push(idOrientedEdge);
  }

  const idTopEdgeLoop = ids.alloc();
  entities.push(`#${idTopEdgeLoop}=EDGE_LOOP('',(${topOrientedEdgeIds.map((id) => `#${id}`).join(",")}));`);

  // --- Vertical edges (connecting bottom to top vertices) ---

  const vertEdgeCurveIds: number[] = [];
  for (let i = 0; i < n; i++) {
    const idLine = ids.alloc();
    const idLineDir = ids.alloc();
    entities.push(`#${idLineDir}=DIRECTION('',${dir3d(0, 0, 1)});`);
    const idLineVec = ids.alloc();
    entities.push(`#${idLineVec}=VECTOR('',#${idLineDir},${sf(thickness)});`);
    entities.push(`#${idLine}=LINE('',#${botPtIds[i]},#${idLineVec});`);

    const idEdge = ids.alloc();
    entities.push(`#${idEdge}=EDGE_CURVE('',#${botVertexIds[i]},#${topVertexIds[i]},#${idLine},.T.);`);
    vertEdgeCurveIds.push(idEdge);
  }

  // =============================================================
  // Faces: bottom, top, and side faces
  // =============================================================

  const faceIds: number[] = [];

  // --- Bottom face (plane at z=0, normal pointing down) ---
  {
    const idPlane = ids.alloc();
    entities.push(`#${idPlane}=PLANE('BottomPlane',#${idAxis2BotFace});`);

    // Bottom face uses reversed edge loop (face normal is -Z)
    const botReversedEdgeIds: number[] = [];
    for (let i = n - 1; i >= 0; i--) {
      const idOE = ids.alloc();
      entities.push(`#${idOE}=ORIENTED_EDGE('',*,*,#${botEdgeCurveIds[i]},.F.);`);
      botReversedEdgeIds.push(idOE);
    }
    const idBotRevLoop = ids.alloc();
    entities.push(`#${idBotRevLoop}=EDGE_LOOP('',(${botReversedEdgeIds.map((id) => `#${id}`).join(",")}));`);

    const idFaceBound = ids.alloc();
    entities.push(`#${idFaceBound}=FACE_OUTER_BOUND('',#${idBotRevLoop},.T.);`);

    const idFace = ids.alloc();
    entities.push(`#${idFace}=ADVANCED_FACE('BottomFace',(#${idFaceBound}),#${idPlane},.T.);`);
    faceIds.push(idFace);
  }

  // --- Top face (plane at z=thickness, normal pointing up) ---
  {
    const idPlane = ids.alloc();
    entities.push(`#${idPlane}=PLANE('TopPlane',#${idAxis2TopFace});`);

    const idFaceBound = ids.alloc();
    entities.push(`#${idFaceBound}=FACE_OUTER_BOUND('',#${idTopEdgeLoop},.T.);`);

    const idFace = ids.alloc();
    entities.push(`#${idFace}=ADVANCED_FACE('TopFace',(#${idFaceBound}),#${idPlane},.T.);`);
    faceIds.push(idFace);
  }

  // --- Side faces (one per edge of the outline polygon) ---
  for (let i = 0; i < n; i++) {
    const j = (i + 1) % n;

    // Compute the outward normal for this edge
    const dx = outline[j].x - outline[i].x;
    const dy = outline[j].y - outline[i].y;
    const len = Math.hypot(dx, dy) || 1;
    // Outward normal (perpendicular, pointing outward for CCW polygon)
    const nx = dy / len;
    const ny = -dx / len;

    const idSideNormal = ids.alloc();
    entities.push(`#${idSideNormal}=DIRECTION('',${dir3d(nx, ny, 0)});`);

    const idSidePlacePt = ids.alloc();
    entities.push(`#${idSidePlacePt}=CARTESIAN_POINT('',${pt3d(outline[i].x, outline[i].y, 0)});`);

    const idSideRefDir = ids.alloc();
    entities.push(`#${idSideRefDir}=DIRECTION('',${dir3d(dx / len, dy / len, 0)});`);

    const idSideAxis = ids.alloc();
    entities.push(`#${idSideAxis}=AXIS2_PLACEMENT_3D('',#${idSidePlacePt},#${idSideNormal},#${idSideRefDir});`);

    const idSidePlane = ids.alloc();
    entities.push(`#${idSidePlane}=PLANE('SidePlane${i}',#${idSideAxis});`);

    // Edge loop: bot_edge[i] -> vert_edge[j] -> top_edge[i](reversed) -> vert_edge[i](reversed)
    const oe1 = ids.alloc();
    entities.push(`#${oe1}=ORIENTED_EDGE('',*,*,#${botEdgeCurveIds[i]},.T.);`);
    const oe2 = ids.alloc();
    entities.push(`#${oe2}=ORIENTED_EDGE('',*,*,#${vertEdgeCurveIds[j]},.T.);`);
    const oe3 = ids.alloc();
    entities.push(`#${oe3}=ORIENTED_EDGE('',*,*,#${topEdgeCurveIds[i]},.F.);`);
    const oe4 = ids.alloc();
    entities.push(`#${oe4}=ORIENTED_EDGE('',*,*,#${vertEdgeCurveIds[i]},.F.);`);

    const idSideLoop = ids.alloc();
    entities.push(`#${idSideLoop}=EDGE_LOOP('',(#${oe1},#${oe2},#${oe3},#${oe4}));`);

    const idSideBound = ids.alloc();
    entities.push(`#${idSideBound}=FACE_OUTER_BOUND('',#${idSideLoop},.T.);`);

    const idSideFace = ids.alloc();
    entities.push(`#${idSideFace}=ADVANCED_FACE('SideFace${i}',(#${idSideBound}),#${idSidePlane},.T.);`);
    faceIds.push(idSideFace);
  }

  // =============================================================
  // Closed shell and manifold solid
  // =============================================================

  const idClosedShell = ids.alloc();
  entities.push(`#${idClosedShell}=CLOSED_SHELL('BoardShell',(${faceIds.map((id) => `#${id}`).join(",")}));`);

  const idManifoldSolid = ids.alloc();
  entities.push(`#${idManifoldSolid}=MANIFOLD_SOLID_BREP('BoardBody',#${idClosedShell});`);

  // =============================================================
  // Shape representation and product association
  // =============================================================

  const idShapeRep = ids.alloc();
  entities.push(`#${idShapeRep}=ADVANCED_BREP_SHAPE_REPRESENTATION('BoardShape',(#${idManifoldSolid},#${idAxis2Top}),#${idGeoRepCtx});`);

  const idShapeDefRep = ids.alloc();
  entities.push(`#${idShapeDefRep}=SHAPE_DEFINITION_REPRESENTATION(#${idShapeDefRep + 1},#${idShapeRep});`);

  const idProdDefShape = ids.alloc();
  entities.push(`#${idProdDefShape}=PRODUCT_DEFINITION_SHAPE('','',#${idProdDef});`);

  // =============================================================
  // Component placements (positional markers, no 3D model data)
  // =============================================================

  for (const fp of data.footprints) {
    emitComponentPlacement(entities, ids, fp, idProdDefCtx, idProdCtx, idAppCtx);
  }

  // =============================================================
  // Assemble the final STEP file
  // =============================================================

  const header = [
    "ISO-10303-21;",
    "HEADER;",
    `FILE_DESCRIPTION(('Signex EDA PCB Export'),'2;1');`,
    `FILE_NAME('${boardName}.step','${dateStr}T00:00:00',('Signex EDA'),(''),'Signex EDA 1.0','Signex','');`,
    "FILE_SCHEMA(('AUTOMOTIVE_DESIGN'));",
    "ENDSEC;",
    "DATA;",
  ];

  const footer = [
    "ENDSEC;",
    "END-ISO-10303-21;",
  ];

  return [...header, ...entities, ...footer].join("\n") + "\n";
}

/**
 * Emit STEP entities for a component placement.
 * Records position/rotation as PRODUCT_DEFINITION_PLACEMENT.
 * Without an external STEP model the placement is a positional marker only.
 */
function emitComponentPlacement(
  entities: string[],
  ids: StepIdAllocator,
  fp: PcbFootprint,
  _prodDefCtxId: number,
  prodCtxId: number,
  appCtxId: number,
): void {
  // Component product
  const idCompProduct = ids.alloc();
  entities.push(`#${idCompProduct}=PRODUCT('${escapeStep(fp.reference)}','${escapeStep(fp.value)}','',(#${prodCtxId}));`);

  const idCompProdDefForm = ids.alloc();
  entities.push(`#${idCompProdDefForm}=PRODUCT_DEFINITION_FORMATION('','',#${idCompProduct});`);

  const idCompProdDefCtx = ids.alloc();
  entities.push(`#${idCompProdDefCtx}=PRODUCT_DEFINITION_CONTEXT('part definition',#${appCtxId},'design');`);

  const idCompProdDef = ids.alloc();
  entities.push(`#${idCompProdDef}=PRODUCT_DEFINITION('design','',#${idCompProdDefForm},#${idCompProdDefCtx});`);

  // Placement: position and rotation
  const z = fp.layer === "B.Cu" ? 0 : (fp as { position: PcbPoint }).position.y !== undefined ? 0 : 0;
  const idPos = ids.alloc();
  entities.push(`#${idPos}=CARTESIAN_POINT('',${pt3d(fp.position.x, fp.position.y, z)});`);

  const rad = (fp.rotation * Math.PI) / 180;
  const cosR = Math.cos(rad);
  const sinR = Math.sin(rad);

  const idRefDir = ids.alloc();
  entities.push(`#${idRefDir}=DIRECTION('',${dir3d(cosR, sinR, 0)});`);

  const idAxisDir = ids.alloc();
  if (fp.layer === "B.Cu") {
    // Bottom-side components face downward
    entities.push(`#${idAxisDir}=DIRECTION('',${dir3d(0, 0, -1)});`);
  } else {
    entities.push(`#${idAxisDir}=DIRECTION('',${dir3d(0, 0, 1)});`);
  }

  const idPlacement = ids.alloc();
  entities.push(`#${idPlacement}=AXIS2_PLACEMENT_3D('${escapeStep(fp.reference)}',#${idPos},#${idAxisDir},#${idRefDir});`);

  // Record the placement as a comment (NEXT_ASSEMBLY_USAGE_OCCURRENCE for real assemblies)
  const idNAUO = ids.alloc();
  entities.push(
    `#${idNAUO}=NEXT_ASSEMBLY_USAGE_OCCURRENCE('${escapeStep(fp.reference)}','','',` +
    `#${idCompProdDef},#${idCompProdDef},'${escapeStep(fp.footprintId)}');`
  );
}

/** Escape a string for STEP — replace single quotes with double-single. */
function escapeStep(s: string): string {
  return s.replace(/'/g, "''");
}
