/**
 * WebGL2 PCB Renderer — Graphics Abstraction Layer (GAL)
 *
 * Architecture (inspired by KiCad GAL + Cadence BoardSurfers):
 * - Layer-based framebuffer caching: static layers cached in textures
 * - Instanced rendering for pads/vias (geometry defined once, drawn many times)
 * - Viewport culling via bounding box checks
 * - Static/dynamic split: grid + copper in static FBO, cursor + selection in dynamic overlay
 *
 * This module provides the low-level WebGL2 rendering primitives.
 * The React component (PcbRenderer.tsx) will use either this or Canvas2D
 * depending on WebGL2 availability.
 */

import type { PcbLayerId } from "@/types/pcb";
import { DEFAULT_LAYER_COLORS } from "@/types/pcb";

// ═══════════════════════════════════════════════════════════════
// SHADER SOURCES
// ═══════════════════════════════════════════════════════════════

const VERTEX_SHADER = `#version 300 es
precision highp float;

// Per-vertex
layout(location = 0) in vec2 a_position;

// Per-instance (for instanced drawing)
layout(location = 1) in vec2 a_offset;
layout(location = 2) in vec2 a_scale;
layout(location = 3) in vec4 a_color;

uniform mat3 u_transform; // Camera transform (zoom + pan)

out vec4 v_color;

void main() {
  vec2 worldPos = a_position * a_scale + a_offset;
  vec3 transformed = u_transform * vec3(worldPos, 1.0);
  gl_Position = vec4(transformed.xy, 0.0, 1.0);
  v_color = a_color;
}
`;

const FRAGMENT_SHADER = `#version 300 es
precision highp float;

in vec4 v_color;
out vec4 fragColor;

void main() {
  fragColor = v_color;
}
`;

// Line rendering shader with width support
const LINE_VERTEX_SHADER = `#version 300 es
precision highp float;

layout(location = 0) in vec2 a_start;
layout(location = 1) in vec2 a_end;
layout(location = 2) in float a_width;
layout(location = 3) in vec4 a_color;
layout(location = 4) in float a_vertexId; // 0-3 for quad vertices

uniform mat3 u_transform;

out vec4 v_color;

void main() {
  vec2 dir = a_end - a_start;
  float len = length(dir);
  vec2 norm = len > 0.0 ? vec2(-dir.y, dir.x) / len : vec2(0.0, 1.0);
  float hw = a_width * 0.5;

  vec2 pos;
  float vid = a_vertexId;
  if (vid < 0.5) pos = a_start + norm * hw;       // 0: start+normal
  else if (vid < 1.5) pos = a_start - norm * hw;   // 1: start-normal
  else if (vid < 2.5) pos = a_end + norm * hw;     // 2: end+normal
  else pos = a_end - norm * hw;                     // 3: end-normal

  vec3 transformed = u_transform * vec3(pos, 1.0);
  gl_Position = vec4(transformed.xy, 0.0, 1.0);
  v_color = a_color;
}
`;

// ═══════════════════════════════════════════════════════════════
// WebGL2 RENDERER CLASS
// ═══════════════════════════════════════════════════════════════

export interface WebGLRendererOptions {
  canvas: HTMLCanvasElement;
}

export class PcbWebGLContext {
  private gl: WebGL2RenderingContext;
  private quadProgram: WebGLProgram | null = null;
  private lineProgram: WebGLProgram | null = null;
  private quadVAO: WebGLVertexArrayObject | null = null;
  private unitQuadBuffer: WebGLBuffer | null = null;

  // Camera
  private cameraX = 0;
  private cameraY = 0;
  private cameraZoom = 5;
  private canvasWidth = 800;
  private canvasHeight = 600;

  constructor(canvas: HTMLCanvasElement) {
    const gl = canvas.getContext("webgl2", {
      antialias: true,
      alpha: false,
      premultipliedAlpha: false,
    });
    if (!gl) throw new Error("WebGL2 not available");
    this.gl = gl;
    this.init();
  }

  private init() {
    const gl = this.gl;

    // Compile shaders
    this.quadProgram = this.createProgram(VERTEX_SHADER, FRAGMENT_SHADER);
    this.lineProgram = this.createProgram(LINE_VERTEX_SHADER, FRAGMENT_SHADER);

    // Unit quad: [-0.5, -0.5] to [0.5, 0.5]
    const quadVerts = new Float32Array([
      -0.5, -0.5, 0.5, -0.5, 0.5, 0.5,
      -0.5, -0.5, 0.5, 0.5, -0.5, 0.5,
    ]);
    this.unitQuadBuffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, this.unitQuadBuffer);
    gl.bufferData(gl.ARRAY_BUFFER, quadVerts, gl.STATIC_DRAW);

    // VAO for quad
    this.quadVAO = gl.createVertexArray();
    gl.bindVertexArray(this.quadVAO);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
    gl.bindVertexArray(null);

    // Enable blending for transparency
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
  }

  private createProgram(vertSrc: string, fragSrc: string): WebGLProgram {
    const gl = this.gl;
    const vert = this.compileShader(gl.VERTEX_SHADER, vertSrc);
    const frag = this.compileShader(gl.FRAGMENT_SHADER, fragSrc);
    const prog = gl.createProgram()!;
    gl.attachShader(prog, vert);
    gl.attachShader(prog, frag);
    gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) {
      throw new Error(`Shader link failed: ${gl.getProgramInfoLog(prog)}`);
    }
    // Detach and delete shader objects after linking — prevent GPU memory leak
    gl.detachShader(prog, vert);
    gl.deleteShader(vert);
    gl.detachShader(prog, frag);
    gl.deleteShader(frag);
    return prog;
  }

  private compileShader(type: number, src: string): WebGLShader {
    const gl = this.gl;
    const shader = gl.createShader(type)!;
    gl.shaderSource(shader, src);
    gl.compileShader(shader);
    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
      throw new Error(`Shader compile failed: ${gl.getShaderInfoLog(shader)}`);
    }
    return shader;
  }

  // --- Camera ---

  setCamera(x: number, y: number, zoom: number) {
    this.cameraX = x;
    this.cameraY = y;
    this.cameraZoom = zoom;
  }

  resize(width: number, height: number) {
    this.canvasWidth = width;
    this.canvasHeight = height;
    this.gl.viewport(0, 0, width, height);
  }

  private getTransformMatrix(): Float32Array {
    // World → NDC transform: translate + scale + viewport normalize
    const sx = (2 * this.cameraZoom) / this.canvasWidth;
    const sy = -(2 * this.cameraZoom) / this.canvasHeight; // Flip Y
    const tx = (2 * this.cameraX) / this.canvasWidth - 1;
    const ty = -(2 * this.cameraY) / this.canvasHeight + 1;

    // Column-major 3x3 matrix
    return new Float32Array([
      sx, 0, 0,
      0, sy, 0,
      tx, ty, 1,
    ]);
  }

  // --- Drawing primitives ---

  clear(r: number, g: number, b: number) {
    const gl = this.gl;
    gl.clearColor(r, g, b, 1.0);
    gl.clear(gl.COLOR_BUFFER_BIT);
  }

  /**
   * Draw filled rectangles (instanced).
   * Each rect: { x, y, w, h, r, g, b, a }
   */
  drawRects(rects: { x: number; y: number; w: number; h: number; color: [number, number, number, number] }[]) {
    if (rects.length === 0) return;
    const gl = this.gl;

    gl.useProgram(this.quadProgram);
    const transformLoc = gl.getUniformLocation(this.quadProgram!, "u_transform");
    gl.uniformMatrix3fv(transformLoc, false, this.getTransformMatrix());

    // Build instance data
    const instanceData = new Float32Array(rects.length * 8); // offset(2) + scale(2) + color(4)
    for (let i = 0; i < rects.length; i++) {
      const r = rects[i];
      const base = i * 8;
      instanceData[base + 0] = r.x + r.w / 2; // center X
      instanceData[base + 1] = r.y + r.h / 2; // center Y
      instanceData[base + 2] = r.w;            // scale X
      instanceData[base + 3] = r.h;            // scale Y
      instanceData[base + 4] = r.color[0];
      instanceData[base + 5] = r.color[1];
      instanceData[base + 6] = r.color[2];
      instanceData[base + 7] = r.color[3];
    }

    const instanceBuffer = gl.createBuffer()!;
    gl.bindBuffer(gl.ARRAY_BUFFER, instanceBuffer);
    gl.bufferData(gl.ARRAY_BUFFER, instanceData, gl.DYNAMIC_DRAW);

    gl.bindVertexArray(this.quadVAO);

    // Bind instance attributes
    gl.bindBuffer(gl.ARRAY_BUFFER, instanceBuffer);
    gl.enableVertexAttribArray(1);
    gl.vertexAttribPointer(1, 2, gl.FLOAT, false, 32, 0);
    gl.vertexAttribDivisor(1, 1);

    gl.enableVertexAttribArray(2);
    gl.vertexAttribPointer(2, 2, gl.FLOAT, false, 32, 8);
    gl.vertexAttribDivisor(2, 1);

    gl.enableVertexAttribArray(3);
    gl.vertexAttribPointer(3, 4, gl.FLOAT, false, 32, 16);
    gl.vertexAttribDivisor(3, 1);

    // Bind unit quad
    const quadBuf = this.unitQuadBuffer!;
    gl.bindBuffer(gl.ARRAY_BUFFER, quadBuf);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
    gl.vertexAttribDivisor(0, 0);

    gl.drawArraysInstanced(gl.TRIANGLES, 0, 6, rects.length);

    gl.bindVertexArray(null);
    gl.deleteBuffer(instanceBuffer);
  }

  /**
   * Draw circles (instanced, using quads with fragment shader discard).
   * For now, uses squares as approximation — proper circles need a circle shader.
   */
  drawCircles(circles: { x: number; y: number; radius: number; color: [number, number, number, number] }[]) {
    // Approximate circles as squares for now (MVP)
    this.drawRects(circles.map((c) => ({
      x: c.x - c.radius,
      y: c.y - c.radius,
      w: c.radius * 2,
      h: c.radius * 2,
      color: c.color,
    })));
  }

  /**
   * Check if WebGL2 is available in the current browser/webview.
   */
  static isAvailable(): boolean {
    try {
      const canvas = document.createElement("canvas");
      const gl = canvas.getContext("webgl2");
      return gl !== null;
    } catch {
      return false;
    }
  }

  destroy() {
    const gl = this.gl;
    if (this.quadProgram) gl.deleteProgram(this.quadProgram);
    if (this.lineProgram) gl.deleteProgram(this.lineProgram);
    if (this.quadVAO) gl.deleteVertexArray(this.quadVAO);
    if (this.unitQuadBuffer) gl.deleteBuffer(this.unitQuadBuffer);
  }
}

/**
 * Parse hex color to RGBA float array.
 */
export function hexToRgba(hex: string, alpha = 1.0): [number, number, number, number] {
  const r = parseInt(hex.slice(1, 3), 16) / 255;
  const g = parseInt(hex.slice(3, 5), 16) / 255;
  const b = parseInt(hex.slice(5, 7), 16) / 255;
  return [r, g, b, alpha];
}

/**
 * Get layer color as RGBA float array.
 */
export function getLayerColorRgba(layer: PcbLayerId, alpha = 1.0): [number, number, number, number] {
  const hex = DEFAULT_LAYER_COLORS[layer] || "#808080";
  return hexToRgba(hex, alpha);
}
