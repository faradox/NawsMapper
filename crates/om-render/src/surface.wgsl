// NawsMapper surface shader.
//
// One draw call per surface. The vertex stage emits the surface quad from
// four corners in canvas pixels; the fragment stage maps each pixel back to
// the unit square with the inverse homography, which gives perspective-
// correct texturing for any four-corner shape.

struct SurfaceUniforms {
    corners0: vec4<f32>,   // p0.xy, p1.xy
    corners1: vec4<f32>,   // p2.xy, p3.xy
    inv_h0: vec4<f32>,     // canvas px -> surface uv (mat3 columns)
    inv_h1: vec4<f32>,
    inv_h2: vec4<f32>,
    uv_m0: vec4<f32>,      // base uv -> texture uv (mat3 columns)
    uv_m1: vec4<f32>,
    uv_m2: vec4<f32>,
    canvas: vec4<f32>,     // width, height, local time, mapping (0 surface, 1 canvas)
    tint: vec4<f32>,
    color_a: vec4<f32>,
    color_b: vec4<f32>,
    gen: vec4<f32>,        // source kind, scale, speed, angle (radians)
    misc: vec4<f32>,       // opacity, hue shift (turns), feather, mask
    pal: vec4<f32>,        // stop count, cyclic, detail, surface aspect (w / h)
    stops: array<vec4<f32>, 6>,
    shape: vec4<f32>,      // outline point count, backdrop blend mode
    points: array<vec4<f32>, 64>, // outline in surface uv, two points per vec4
};

@group(0) @binding(0) var<uniform> u: SurfaceUniforms;
@group(1) @binding(0) var src_tex: texture_2d<f32>;
@group(1) @binding(1) var src_samp: sampler;
// What is already drawn under this surface (only for the backdrop blend modes).
@group(2) @binding(0) var backdrop: texture_2d<f32>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) canvas_px: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    var order = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);
    let i = order[vi];
    var p: vec2<f32>;
    switch i {
        case 0u: { p = u.corners0.xy; }
        case 1u: { p = u.corners0.zw; }
        case 2u: { p = u.corners1.xy; }
        default: { p = u.corners1.zw; }
    }
    var out: VsOut;
    out.pos = vec4<f32>(p.x / u.canvas.x * 2.0 - 1.0, 1.0 - p.y / u.canvas.y * 2.0, 0.0, 1.0);
    out.canvas_px = p;
    return out;
}

const TAU: f32 = 6.28318530718;

fn rot2(a: f32) -> mat2x2<f32> {
    let c = cos(a);
    let s = sin(a);
    return mat2x2<f32>(c, s, -s, c);
}

fn hash2(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2<f32>(123.34, 456.21));
    let r = q + dot(q, q + 45.32);
    return fract(r.x * r.y);
}

fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let w = f * f * (3.0 - 2.0 * f);
    let a = hash2(i);
    let b = hash2(i + vec2<f32>(1.0, 0.0));
    let c = hash2(i + vec2<f32>(0.0, 1.0));
    let d = hash2(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, w.x), mix(c, d, w.x), w.y);
}

fn fbm(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var amp = 0.5;
    var q = p;
    for (var i = 0; i < 5; i++) {
        v += amp * value_noise(q);
        q = q * 2.03 + vec2<f32>(17.1, 9.2);
        amp *= 0.5;
    }
    return v;
}

// Antialiased square wave with duty 0.5; `x` in cycles.
fn aa_square(x: f32) -> f32 {
    let w = max(fwidth(x), 1e-4);
    let f = fract(x);
    return clamp((min(f, 1.0 - f) - 0.25) / w + 0.5, 0.0, 1.0);
}

// Like `aa_square` with an explicit filter width (for angles, whose
// derivative jumps at the atan2 seam).
fn aa_square_w(x: f32, w: f32) -> f32 {
    let f = fract(x);
    return clamp((min(f, 1.0 - f) - 0.25) / max(w, 1e-4) + 0.5, 0.0, 1.0);
}

fn hash22(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(hash2(p), hash2(p + vec2<f32>(19.19, 7.31)));
}

// Distance (in cycles) to the nearest integer, antialiased into a line mask.
fn aa_line(x: f32, thickness: f32) -> f32 {
    let w = max(fwidth(x), 1e-4);
    let d = abs(fract(x + 0.5) - 0.5);
    return 1.0 - smoothstep(thickness * w, thickness * w + w, d);
}

// Color at position `x` of the source's palette. Cyclic palettes wrap;
// the others run first -> last -> first, so any `x` is seamless.
fn palette(x: f32) -> vec4<f32> {
    let n = clamp(u32(u.pal.x), 1u, 6u);
    var pos: f32;
    var j: u32;
    if u.pal.y > 0.5 {
        pos = fract(x) * f32(n);
    } else {
        pos = (1.0 - abs(1.0 - 2.0 * fract(x * 0.5))) * f32(n - 1u);
    }
    let i = min(u32(pos), n - 1u);
    if u.pal.y > 0.5 {
        j = (i + 1u) % n;
    } else {
        j = min(i + 1u, n - 1u);
    }
    return mix(u.stops[i], u.stops[j], pos - f32(i));
}

fn generator(kind: u32, uv: vec2<f32>, t: f32) -> vec4<f32> {
    if kind == 8u {
        return test_grid(uv);
    }
    let f = field(kind, uv, t);
    // `x` picks the palette color, `y` fades it in over the background (color A).
    return mix(u.color_a, palette(f.x), clamp(f.y, 0.0, 1.0));
}

// Every pattern returns (palette position, intensity).
fn field(kind: u32, uv: vec2<f32>, t: f32) -> vec2<f32> {
    let scale = u.gen.y;
    let speed = u.gen.z;
    let angle = u.gen.w;
    let detail = u.pal.z;
    let aspect = u.pal.w;
    let tt = t * speed;
    let ruv = rot2(angle) * (uv - 0.5);
    // Centered and aspect-corrected, so circles stay round on any surface.
    let p = rot2(angle) * ((uv - 0.5) * vec2<f32>(aspect, 1.0));
    let r = length(p);
    // Angle in turns (-0.5..0.5) and its filter width, which unlike
    // fwidth(a) does not explode at the seam.
    let a = atan2(p.y, p.x) / TAU;
    let aw = length(fwidth(p)) / (TAU * max(r, 1e-4));

    switch kind {
        // Gradient: mirrored, so scrolling stays seamless.
        case 2u: {
            let x = ruv.x * max(scale * 0.25, 0.01) + 0.5 + tt;
            let f = 1.0 - abs(fract(x * 0.5) * 2.0 - 1.0);
            return vec2<f32>(smoothstep(0.0, 1.0, f), 1.0);
        }
        // Checker
        case 3u: {
            let q = ruv * scale + vec2<f32>(tt, 0.0);
            return vec2<f32>(abs(aa_square(q.x * 0.5) - aa_square(q.y * 0.5)), 1.0);
        }
        // Stripes
        case 4u: {
            return vec2<f32>(aa_square(ruv.x * scale - tt), 1.0);
        }
        // Rings
        case 5u: {
            return vec2<f32>(aa_square(length(uv - 0.5) * scale - tt), 1.0);
        }
        // Noise
        case 6u: {
            let n = fbm(ruv * scale + vec2<f32>(tt, tt * 0.37));
            return vec2<f32>(smoothstep(0.25, 0.75, n), 1.0);
        }
        // Plasma
        case 7u: {
            let q = ruv * scale;
            let ph = tt * TAU;
            var v = sin(q.x * 1.7 + ph) + sin(q.y * 1.3 - ph * 0.7)
                + sin((q.x + q.y) * 0.9 + ph * 0.5)
                + sin(length(q + vec2<f32>(sin(ph * 0.3), cos(ph * 0.4)) * 2.0) * 1.5);
            return vec2<f32>(0.5 + 0.5 * sin(v * 1.2), 1.0);
        }
        // Tunnel: checkered walls rushing past, fog in the distance.
        case 9u: {
            let rr = max(r, 1e-3);
            let depth = 0.35 / rr + tt;
            let sides = max(floor(scale), 1.0);
            let twist = (detail - 0.5) * 3.0;
            let tu = a * sides + twist * 0.35 / rr;
            let wu = aw * sides + abs(twist) * fwidth(0.35 / rr);
            let c = abs(aa_square_w(tu, wu) - aa_square_w(depth * 2.0, fwidth(depth * 2.0)));
            return vec2<f32>(depth * 0.25 + c * 0.45, smoothstep(0.0, 0.45, rr));
        }
        // Kaleidoscope: mirrored wedges of flowing noise.
        case 10u: {
            let seg = 1.0 / (3.0 + floor(detail * 9.0));
            let m = fract(a / seg) * seg;
            let fa = min(m, seg - m) * TAU;
            var q = vec2<f32>(cos(fa), sin(fa)) * r * scale;
            q = rot2(tt * 0.4) * q + vec2<f32>(tt * 0.6, tt * 0.25);
            let n = fbm(q * 0.8);
            let w = 0.5 + 0.5 * sin(q.x * 2.0 + n * 5.0 + tt);
            return vec2<f32>(n * 1.4 + r * 0.6 - tt * 0.15 + w * 0.3, 0.75 + 0.25 * w);
        }
        // Hypno spiral
        case 11u: {
            let arms = 1.0 + floor(detail * 6.0);
            let x = a * arms + r * scale - tt;
            return vec2<f32>(aa_square_w(x, aw * arms + fwidth(r) * scale), 1.0);
        }
        // Liquid: noise warped by noise warped by noise.
        case 12u: {
            let q0 = p * scale * 0.6;
            let warp = 1.0 + detail * 5.0;
            let q = vec2<f32>(fbm(q0 + vec2<f32>(0.0, tt * 0.2)), fbm(q0 + vec2<f32>(5.2, 1.3) - tt * 0.15));
            let s = vec2<f32>(
                fbm(q0 + warp * q + vec2<f32>(1.7, 9.2) + tt * 0.1),
                fbm(q0 + warp * q + vec2<f32>(8.3, 2.8) - tt * 0.12),
            );
            let f = fbm(q0 + warp * s);
            return vec2<f32>(f * 1.8 + length(q) * 0.5, 0.4 + 0.6 * smoothstep(0.05, 0.6, f));
        }
        // Cells: drifting Voronoi cells with dark edges.
        case 13u: {
            let g = p * scale;
            let ip = floor(g);
            let fp = fract(g);
            var d1 = 8.0;
            var d2 = 8.0;
            var id = vec2<f32>(0.0);
            for (var y = -1; y <= 1; y++) {
                for (var x = -1; x <= 1; x++) {
                    let o = vec2<f32>(f32(x), f32(y));
                    let pt = o + 0.5 + 0.4 * sin(tt + TAU * hash22(ip + o)) - fp;
                    let d = dot(pt, pt);
                    if d < d1 {
                        d2 = d1;
                        d1 = d;
                        id = ip + o;
                    } else if d < d2 {
                        d2 = d;
                    }
                }
            }
            let edge = sqrt(d2) - sqrt(d1);
            let lit = smoothstep(0.0, 0.03 + detail * 0.4, edge);
            return vec2<f32>(hash2(id) + tt * 0.05, lit * (1.0 - 0.5 * sqrt(d1)));
        }
        // Warp speed: layers of stars flying toward the viewer.
        case 14u: {
            var glow = 0.0;
            var hue = 0.0;
            for (var l = 0; l < 4; l++) {
                let lf = f32(l);
                let depth = fract(lf * 0.25 + tt * 0.15);
                let q = p * mix(scale * 4.0, scale * 0.25, depth) + vec2<f32>(lf * 13.7, lf * 7.3);
                let cell = floor(q);
                let h = hash2(cell);
                let d = length(fract(q) - 0.5 - (hash22(cell + 3.1) - 0.5) * 0.6);
                let size = (0.03 + detail * 0.1) * (0.5 + h);
                let star = 1.0 - smoothstep(0.0, size, d) + 0.25 * (1.0 - smoothstep(0.0, size * 4.0, d));
                let fade = smoothstep(0.0, 0.4, depth) * (1.0 - smoothstep(0.8, 1.0, depth));
                let s = star * fade * step(0.4, h);
                glow += s;
                hue += s * h;
            }
            return vec2<f32>(hue / max(glow, 1e-3), glow);
        }
        // Lava lamp: metaballs.
        case 15u: {
            let n = 3 + i32(detail * 7.0);
            let size = 4.0 / max(scale, 0.5);
            var f = 0.0;
            for (var k = 0; k < n; k++) {
                let kf = f32(k);
                let c = vec2<f32>(
                    sin(tt * (0.31 + 0.07 * kf) + kf * 1.7) * 0.42 * aspect,
                    cos(tt * (0.23 + 0.05 * kf) + kf * 2.3) * 0.42,
                );
                let rad = (0.1 + 0.06 * hash2(vec2<f32>(kf, 3.0))) * size;
                f += rad * rad / max(dot(p - c, p - c), 1e-5);
            }
            let blob = smoothstep(0.9, 1.1, f);
            return vec2<f32>(clamp(f * 0.4, 0.0, 1.0) * 0.7 + 0.3 * blob, mix(0.12, 1.0, blob));
        }
        // Truchet: quarter-circle tiles that keep flipping.
        case 16u: {
            let g = p * scale;
            let id = floor(g);
            var f = fract(g) - 0.5;
            let h = hash2(id);
            let flip = fract(h + floor(tt * 0.5 + h * 4.0) * 0.618) > 0.5;
            f.x = select(f.x, -f.x, flip);
            let d = min(abs(length(f - 0.5) - 0.5), abs(length(f + 0.5) - 0.5));
            let wdt = 0.04 + detail * 0.2;
            let fw = max(fwidth(d), 1e-4);
            let line = 1.0 - smoothstep(wdt - fw, wdt + fw, d);
            return vec2<f32>(0.6 + length(id) * 0.04 + h * 0.2 - tt * 0.1, mix(0.08, 1.0, line));
        }
        // Moiré: two ring systems drifting over each other.
        case 17u: {
            let drift = 0.1 + detail * 0.4;
            let c1 = vec2<f32>(sin(tt * 0.7), cos(tt * 0.5)) * drift;
            let c2 = vec2<f32>(cos(tt * 0.6 + 1.0), sin(tt * 0.8)) * drift;
            let k = scale * 3.0;
            return vec2<f32>(abs(aa_square(length(p - c1) * k) - aa_square(length(p - c2) * k)), 1.0);
        }
        // Julia set whose constant travels just inside the Mandelbrot
        // cardioid, where the shapes are most intricate.
        case 18u: {
            var z = p * (12.8 / max(scale, 0.5));
            let th = tt * 0.3;
            let c = (vec2<f32>(cos(th), sin(th)) * 0.5 - vec2<f32>(cos(2.0 * th), sin(2.0 * th)) * 0.25) * 0.98;
            let iters = 24 + i32(detail * 72.0);
            var esc = -1.0;
            for (var k = 0; k < iters; k++) {
                if esc < 0.0 {
                    z = vec2<f32>(z.x * z.x - z.y * z.y, 2.0 * z.x * z.y) + c;
                    let m2 = dot(z, z);
                    if m2 > 256.0 {
                        esc = f32(k) - log2(log2(m2)) + 4.0;
                    }
                }
            }
            let inside = esc < 0.0;
            // Bands of color outside, brightest along the edge of the set.
            let glow = mix(0.2, 1.0, pow(clamp(esc / 24.0, 0.0, 1.0), 0.7));
            return select(vec2<f32>(log2(esc + 1.0) * 0.45 + tt * 0.05, glow), vec2<f32>(0.0), inside);
        }
        // Aurora: soft curtains with a sharp lower edge.
        case 19u: {
            let q = ruv + 0.5;
            let sharp = mix(40.0, 8.0, detail);
            var glow = 0.0;
            var pos = 0.0;
            for (var l = 0; l < 5; l++) {
                let lf = f32(l);
                let x = q.x * scale * 0.5 + lf * 0.37;
                let y0 = 0.25 + 0.11 * lf + 0.1 * sin(x * 2.3 + tt * (0.5 + 0.13 * lf))
                    + 0.12 * (value_noise(vec2<f32>(x * 1.3, tt * 0.25 + lf * 3.0)) - 0.5);
                let dy = q.y - y0;
                // y points down: glow fades slowly upward, quickly downward.
                let band = select(exp(-dy * sharp), exp(dy * sharp * 0.25), dy < 0.0);
                let rays = 0.55 + 0.45 * value_noise(vec2<f32>(x * 18.0, tt * 0.6 + lf));
                let g = band * rays * (0.35 + 0.15 * lf);
                glow += g;
                pos += g * lf * 0.25;
            }
            return vec2<f32>(pos / max(glow, 1e-3) + q.x * 0.2 + 0.2, glow);
        }
        // Fire rising from the bottom edge.
        case 20u: {
            let q = vec2<f32>(ruv.x, ruv.y) * scale * 0.7 + vec2<f32>(0.0, tt * 2.0);
            let n = fbm(q + vec2<f32>(fbm(q * 0.7 + vec2<f32>(0.0, tt * 0.5)) * 1.5, 0.0));
            let height = 0.5 - ruv.y;
            let heat = clamp(n * 1.6 - height * (2.2 - detail * 1.6) + 0.15, 0.0, 1.0);
            return vec2<f32>(heat, smoothstep(0.0, 0.3, heat));
        }
        // Ripples from several drops.
        case 21u: {
            let drops = 2 + i32(detail * 5.0);
            var v = 0.0;
            for (var k = 0; k < drops; k++) {
                let kf = f32(k);
                let c = vec2<f32>(sin(kf * 2.1 + tt * 0.21) * 0.35 * aspect, cos(kf * 1.7 + tt * 0.17) * 0.35);
                let d = length(p - c);
                v += sin(d * scale * 12.0 - tt * 4.0) / (1.0 + d * 4.0);
            }
            return vec2<f32>(0.5 + 0.5 * clamp(v / (f32(drops) * 0.4), -1.0, 1.0), 1.0);
        }
        // Hexagons lit by a traveling wave.
        case 22u: {
            let g = p * scale;
            let rv = vec2<f32>(1.0, 1.7320508);
            let hv = rv * 0.5;
            let ga = g - rv * floor(g / rv) - hv;
            let gb = (g - hv) - rv * floor((g - hv) / rv) - hv;
            let gv = select(gb, ga, dot(ga, ga) < dot(gb, gb));
            let id = g - gv;
            let ag = abs(gv);
            let hexd = max(dot(ag, normalize(rv)), ag.x);
            let wave = 0.5 + 0.5 * sin(length(id) * mix(1.5, 0.15, detail) - tt * 3.0);
            let fill = 1.0 - smoothstep(0.40, 0.46, hexd);
            return vec2<f32>(wave * 0.7 + hash2(id) * 0.15, fill * mix(0.15, 1.0, wave));
        }
        // Vortex: arms wound tighter toward the center.
        case 23u: {
            let arms = max(floor(scale * 0.5), 1.0);
            let tw = (0.5 + detail * 3.0) / (r + 0.2);
            let ang = a * TAU + tw + tt;
            let swirl = 0.5 + 0.5 * sin(ang * arms + r * 4.0);
            // Angle terms in steps of 2 so mirrored palettes have no seam.
            return vec2<f32>(a * 2.0 + (tw + tt) / TAU * 2.0 + swirl * 0.3, smoothstep(0.0, 0.12, r) * (0.55 + 0.45 * swirl));
        }
        // Sunburst
        case 24u: {
            let rays = max(floor(scale * 3.0), 2.0);
            let ray = aa_square_w(a * rays + tt, aw * rays);
            return vec2<f32>(ray * 0.5 + r * detail * 3.0 - tt * 0.1, 1.0);
        }
        // Op art: warped stripes.
        case 25u: {
            let amp = 0.2 + detail * 1.5;
            let x = ruv.x * scale * 3.0 + sin(ruv.y * 7.0 + tt) * amp + sin(length(ruv) * 9.0 - tt * 0.7) * amp * 0.6;
            return vec2<f32>(aa_square(x), 1.0);
        }
        // Digital rain: falling columns of glowing blocks.
        case 26u: {
            let q = ruv + 0.5;
            let cols = max(floor(scale * 8.0), 2.0);
            let rows = cols / max(aspect, 0.1);
            let cx = q.x * cols;
            let col = floor(cx);
            let h = hash2(vec2<f32>(col, 1.7));
            let s = fract((q.y - tt * (0.3 + h * 0.7)) * 0.7 - h * 7.0);
            let trail = pow(s, mix(12.0, 1.5, detail));
            let cy = q.y * rows;
            let glyph = step(0.35, hash2(vec2<f32>(col, floor(cy) + floor(tt * 6.0 * (0.5 + h)))));
            let fx = fract(cx);
            let fy = fract(cy);
            let cell = step(0.15, fx) * step(fx, 0.85) * step(0.1, fy) * step(fy, 0.9);
            let head = smoothstep(0.96, 1.0, s);
            return vec2<f32>(1.0, (trail * mix(0.35, 1.0, glyph) + head) * cell);
        }
        // Melt: space folded over itself by layers of sine waves.
        case 27u: {
            var q = p * scale;
            let layers = 3 + i32(detail * 7.0);
            for (var k = 1; k <= 10; k++) {
                if k <= layers {
                    let kf = f32(k);
                    q += vec2<f32>(
                        sin(q.y * kf * 0.9 + tt + kf * 1.3),
                        cos(q.x * kf * 0.9 - tt * 0.8 + kf * 0.7),
                    ) * (0.7 / kf);
                }
            }
            let v = 0.5 + 0.5 * sin(q.x * 0.8 + q.y * 0.6);
            return vec2<f32>(v, 0.8 + 0.2 * sin(q.x - q.y));
        }
        // Fractal lace: the "Kali" fractal, abs-folded and inverted, so it
        // mirrors itself into lace that keeps rearranging.
        case 28u: {
            var z = rot2(tt * 0.15) * p * (2.5 / max(scale, 0.3));
            let c = vec2<f32>(0.75 + 0.12 * sin(tt * 0.31), 0.6 + 0.12 * cos(tt * 0.23));
            let iters = 5 + i32(detail * 10.0);
            var trap = 1e3;
            for (var k = 0; k < 15; k++) {
                if k < iters {
                    z = abs(z) / max(dot(z, z), 1e-4) - c;
                    trap = min(trap, abs(z.x * z.y));
                }
            }
            let glow = exp(-trap * 6.0);
            return vec2<f32>(log(1.0 + length(z)) * 0.5 + glow * 0.4, mix(0.08, 1.0, glow));
        }
        // Quasicrystal: plane waves in n directions, never quite repeating.
        case 29u: {
            let n = 3 + i32(detail * 8.0);
            let q = p * scale * 16.0;
            var v = 0.0;
            for (var k = 0; k < 11; k++) {
                if k < n {
                    let th = f32(k) * TAU * 0.5 / f32(n);
                    v += cos(dot(q, vec2<f32>(cos(th), sin(th))) + tt * TAU * 0.5);
                }
            }
            v /= f32(n);
            return vec2<f32>(v * 1.2 + 0.5 + tt * 0.05, 1.0);
        }
        // Caustics: light focused by a moving water surface.
        case 30u: {
            let q = p * scale * 2.0 - 250.0;
            var i = q;
            var c = 1.0;
            let inten = 0.005;
            for (var n = 0; n < 5; n++) {
                let t2 = tt * (1.0 - 3.5 / f32(n + 1));
                i = q + vec2<f32>(cos(t2 - i.x) + sin(t2 + i.y), sin(t2 - i.y) + cos(t2 + i.x));
                c += 1.0 / length(vec2<f32>(q.x / (sin(i.x + t2) / inten), q.y / (cos(i.y + t2) / inten)));
            }
            c = 1.17 - pow(c / 5.0, 1.4);
            let v = clamp(pow(abs(c), mix(4.0, 14.0, detail)), 0.0, 1.0);
            return vec2<f32>(0.25 + 0.75 * v, 1.0);
        }
        // Retro grid: synthwave floor racing toward a banded sun.
        case 31u: {
            let q = ruv + 0.5;
            let horizon = 0.58;
            let dy = q.y - horizon;
            let z = 0.3 / max(dy, 1e-3);
            let gx = (q.x - 0.5) * aspect * z * scale * 2.0;
            let gz = z * scale + tt * 2.0;
            let lines = max(aa_line(gx, 1.5), aa_line(gz, 1.5)) * smoothstep(0.0, 0.2, dy);
            let ground = vec2<f32>(mix(0.15, 0.55, lines), mix(0.2, 1.0, lines));

            let sun_r = 0.12 + detail * 0.2;
            let sc = vec2<f32>(0.5, horizon - sun_r * 0.55);
            let sd = length((q - sc) * vec2<f32>(aspect, 1.0)) / sun_r;
            let sy = (q.y - sc.y) / sun_r;
            // Gaps that widen toward the bottom of the sun.
            let gap_x = fract(sy * 7.0 - tt * 0.4);
            let gap_w = clamp(sy, 0.0, 1.0) * 0.6;
            let gfw = max(fwidth(sy * 7.0), 1e-4);
            let solid = smoothstep(gap_w - gfw, gap_w + gfw, gap_x);
            let sfw = max(fwidth(sd), 1e-4);
            let disk = (1.0 - smoothstep(1.0 - sfw, 1.0 + sfw, sd)) * select(1.0, solid, sy > 0.0);
            let sky = vec2<f32>(0.25, 0.15 + 0.6 * exp(dy * 6.0) + 0.3 * exp(-sd * 1.5));
            let sun = vec2<f32>(mix(1.0, 0.55, clamp(sy * 0.5 + 0.5, 0.0, 1.0)), 1.0);
            return select(mix(sky, sun, disk), ground, dy > 0.0);
        }
        // Galaxy: spiral arms of dust around a bright core, with stars.
        case 32u: {
            let zr = r * 4.0 / max(scale, 0.5);
            let arms = 2.0 + floor(detail * 4.0);
            let spiral = a * TAU * arms - log(max(zr, 1e-3)) * 4.0 + tt;
            let arm = pow(0.5 + 0.5 * cos(spiral), 3.0);
            let dust = fbm(rot2(-tt / arms) * p * 10.0 / max(scale, 0.5));
            let disk = exp(-zr * 1.4);
            let core = exp(-zr * 9.0);
            let sg = p * 90.0;
            let cell = floor(sg);
            let h = hash2(cell);
            let star = (1.0 - smoothstep(0.0, 0.25, length(fract(sg) - 0.5)))
                * step(0.96, h) * (0.6 + 0.4 * sin(tt * 3.0 + h * 60.0));
            let density = disk * (0.15 + 1.1 * arm) * (0.4 + dust) + core * 1.5 + star;
            return vec2<f32>(0.2 + arm * disk * 0.5 + core * 0.8 + star * 0.3, density);
        }
        // Electric: crackling lines along the contours of moving noise.
        case 33u: {
            let q = p * scale * 0.8;
            let n1 = fbm(q + vec2<f32>(tt * 0.3, -tt * 0.2));
            let n = fbm(q * 1.3 + vec2<f32>(n1 * 2.0, tt * 0.5));
            let line = abs(fract(n * 3.0) - 0.5);
            let glow = clamp(pow((0.01 + detail * 0.04) / max(line, 1e-3), 1.3), 0.0, 1.5);
            return vec2<f32>(0.3 + 0.7 * min(glow, 1.0), glow);
        }
        // Bubbles: iridescent soap bubbles rising in three layers.
        case 34u: {
            var glow = 0.0;
            var pos = 0.0;
            for (var l = 0; l < 3; l++) {
                let lf = f32(l);
                var g = p * scale * (1.0 + lf * 0.6) + vec2<f32>(lf * 7.1, tt * (0.6 + lf * 0.25));
                g.x += sin(g.y * 0.9 + lf + tt * 0.5) * 0.15;
                let id = floor(g);
                let h = hash2(id + lf * 17.0);
                // Radius + offset stay below half a cell, so no bubble is cut off.
                let f = fract(g) - 0.5 - (hash22(id + 4.0) - 0.5) * 0.36;
                let rad = mix(0.14, 0.3, hash2(id + 9.0));
                let d = length(f) / rad;
                // fwidth(d) jumps at cell borders; g is continuous.
                let aa = max(length(fwidth(g)) / rad, 1e-3);
                let inside = 1.0 - smoothstep(1.0 - aa, 1.0 + aa, d);
                let rim = smoothstep(0.7, 1.0, d) * inside;
                let shine = 1.0 - smoothstep(0.0, 0.2, length(f / rad + vec2<f32>(0.35)));
                let b = (rim + inside * 0.15 + shine * 0.8) * step(0.9 - detail * 0.8, h) * (0.8 + lf * 0.2);
                glow += b;
                pos += b * (h * 0.5 + d * 0.7 + lf * 0.15);
            }
            return vec2<f32>(pos / max(glow, 1e-3), glow);
        }
        // Mandala: rings of petals turning in alternating directions.
        case 35u: {
            let petals = 4.0 + floor(detail * 12.0);
            let rr = r * scale;
            let ring = floor(rr);
            let fr = fract(rr);
            let dir = select(-1.0, 1.0, ring - 2.0 * floor(ring * 0.5) < 0.5);
            let k = petals * (1.0 + floor(ring * 0.5));
            let m = abs(fract((a + tt * 0.04 * dir) * k) - 0.5) * 2.0;
            let edge = 0.1 + 0.85 * cos(m * TAU * 0.25);
            let d = fr - edge;
            let w = fwidth(rr) + 2.7 * k * aw;
            let fill = 1.0 - smoothstep(-w, w, d);
            let outline = 1.0 - smoothstep(0.0, w * 1.5, abs(d) - 0.015);
            let inner = 1.0 - smoothstep(0.0, w * 1.5, abs(fr - edge * 0.55) - 0.01);
            let dots = 1.0 - smoothstep(0.0, w * 1.5, length(vec2<f32>((1.0 - m) * 0.3, fr - 0.92)) - 0.04);
            let lines = max(max(outline, inner), dots);
            return vec2<f32>(ring * 0.2 + fill * 0.25 + m * 0.15 - tt * 0.05, max(fill * 0.45, lines) + 0.1);
        }
        // Ink blot: a mirrored Rorschach shape that keeps changing.
        case 36u: {
            let q = vec2<f32>(abs(p.x), p.y) * scale * 0.5;
            let warp = vec2<f32>(fbm(q + vec2<f32>(0.0, tt * 0.15)), fbm(q + vec2<f32>(4.7, 1.9) - tt * 0.1));
            let f = fbm(q * 1.4 + warp * (1.0 + detail * 2.5) + vec2<f32>(0.0, tt * 0.05));
            let v = f + 0.3 - length(p * vec2<f32>(0.8, 1.0)) * 0.5;
            let fw = max(fwidth(v), 1e-4);
            let blot = smoothstep(0.45 - fw, 0.45 + fw, v);
            let inner = smoothstep(0.5, 0.75, v);
            return vec2<f32>(0.3 + inner * 0.6 + warp.x * 0.3, blot * (0.75 + 0.25 * inner));
        }
        // Silk: two ribbons of fine glowing strands.
        case 37u: {
            let q = ruv + 0.5;
            let strands = 4 + i32(detail * 20.0);
            let px = max(fwidth(q.y), 1e-4);
            let x = q.x * scale * 0.5;
            var glow = 0.0;
            var pos = 0.0;
            for (var k = 0; k < 24; k++) {
                if k < strands {
                    let kf = f32(k) / f32(strands);
                    let rib = f32(k % 2);
                    let y = 0.5 + (rib - 0.5) * 0.18
                        + 0.22 * sin(x * 2.0 + tt + kf * 1.5 + rib * 2.7) * cos(x * 0.7 - tt * 0.6 + kf * 0.8)
                        + (kf - 0.5) * 0.2 * sin(x * 1.3 + tt * 0.4 + rib);
                    let d = abs(q.y - y);
                    let g = (1.0 - smoothstep(0.0, px * 2.0, d)) * 0.7 + exp(-d / (px * 12.0)) * 0.12;
                    glow += g;
                    pos += g * (kf * 0.6 + rib * 0.4);
                }
            }
            return vec2<f32>(pos / max(glow, 1e-3) + q.x * 0.3, 1.0 - exp(-glow * 1.5));
        }
        // Glitch: blocks and slices of a broken signal.
        case 38u: {
            let q = ruv + 0.5;
            let tick = floor(tt * 8.0);
            let row = floor(q.y * scale * 6.0);
            let jump = hash2(vec2<f32>(row, tick)) > 1.0 - detail * 0.5;
            let x = q.x + select(0.0, (hash2(vec2<f32>(row, tick + 7.0)) - 0.5) * 0.4 * (0.3 + detail), jump);
            let blk = floor(vec2<f32>(x * scale * 2.0, q.y * scale));
            let hb = hash2(blk + vec2<f32>(floor(tt * 2.0) * 0.37, 0.0));
            let fine = hash2(floor(vec2<f32>(x * scale * 24.0, q.y * scale * 6.0)) + tick);
            let scan = 0.75 + 0.25 * sin(q.y * u.canvas.y * 1.5);
            let on = step(0.3, hb) * mix(0.6, 1.0, step(0.5, fine));
            return vec2<f32>(hb + select(0.0, 0.5, jump), (on + select(0.0, 0.5, jump)) * scan);
        }
        // Infinite zoom: twisted squares falling inward forever.
        case 39u: {
            let rr = max(r, 1e-4);
            let q = rot2(log(rr) * detail * 3.0 + tt * 0.3) * p;
            let sq = max(max(abs(q.x), abs(q.y)), 1e-4);
            let x = log(sq) * scale * 0.5 - tt;
            let s = aa_square_w(x, fwidth(x));
            return vec2<f32>(x * 0.15 + s * 0.1, mix(0.25, 1.0, s) * smoothstep(0.0, 0.02, r));
        }
        // Halftone: a dot screen whose dots swell with traveling waves.
        case 40u: {
            let k = scale * 3.0;
            let g = rot2(0.785398) * p * k;
            let id = floor(g) + 0.5;
            let f = g - id;
            let c = rot2(-0.785398) * id / k;
            let wave = 0.5 + 0.5 * sin(length(c) * 10.0 - tt * 3.0 + sin(c.x * 4.0 + tt) * 1.5);
            let rad = wave * (0.2 + detail * 0.5);
            let fw = length(fwidth(g)) * 0.7;
            let dot_ = 1.0 - smoothstep(-fw, fw, length(f) - rad);
            return vec2<f32>(wave * 0.8 + 0.1, dot_);
        }
        default: {
            return vec2<f32>(0.0, 1.0);
        }
    }
}

// Calibration grid: background a, lines b, plus border, crosshair and circle.
fn test_grid(uv: vec2<f32>) -> vec4<f32> {
    let cells = max(u.gen.y, 1.0);
    let g = max(aa_line(uv.x * cells, 1.0), aa_line(uv.y * cells, 1.0));
    let sub = max(aa_line(uv.x * cells * 4.0, 0.5), aa_line(uv.y * cells * 4.0, 0.5)) * 0.3;
    let border = max(aa_line(uv.x, 4.0), aa_line(uv.y, 4.0));
    let cross = max(aa_line(uv.x - 0.5, 2.0), aa_line(uv.y - 0.5, 2.0));
    let circle = aa_line(length(uv - 0.5) * 2.0 - 0.8, 2.0);
    let diag = max(aa_line(uv.x - uv.y, 1.0), aa_line(uv.x + uv.y - 1.0, 1.0)) * 0.5;
    let m = clamp(max(max(max(g, sub), max(border, cross)), max(circle, diag)), 0.0, 1.0);
    var c = mix(u.color_a, u.color_b, m);
    // Colored quadrant markers so orientation is obvious on the wall.
    let q = step(vec2<f32>(0.5), uv);
    let qc = vec3<f32>(1.0 - q.x, q.x * (1.0 - q.y), q.y);
    let blob = 1.0 - smoothstep(0.035, 0.04, length(fract(uv * 2.0) - 0.5) * 0.5);
    c = vec4<f32>(mix(c.rgb, qc, blob * 0.9), c.a);
    return c;
}

// ---- Blend modes (backdrop `cb`, source `cs`, both straight sRGB) ---------

fn screen(cb: vec3<f32>, cs: vec3<f32>) -> vec3<f32> {
    return cb + cs - cb * cs;
}

fn hard_light(cb: vec3<f32>, cs: vec3<f32>) -> vec3<f32> {
    return select(screen(cb, 2.0 * cs - 1.0), cb * 2.0 * cs, cs <= vec3<f32>(0.5));
}

fn color_dodge(cb: vec3<f32>, cs: vec3<f32>) -> vec3<f32> {
    let d = select(min(vec3<f32>(1.0), cb / max(1.0 - cs, vec3<f32>(1e-5))), vec3<f32>(1.0), cs >= vec3<f32>(1.0));
    return select(d, vec3<f32>(0.0), cb <= vec3<f32>(0.0));
}

fn color_burn(cb: vec3<f32>, cs: vec3<f32>) -> vec3<f32> {
    let d = select(1.0 - min(vec3<f32>(1.0), (1.0 - cb) / max(cs, vec3<f32>(1e-5))), vec3<f32>(0.0), cs <= vec3<f32>(0.0));
    return select(d, vec3<f32>(1.0), cb >= vec3<f32>(1.0));
}

fn soft_light(cb: vec3<f32>, cs: vec3<f32>) -> vec3<f32> {
    let d = select(sqrt(cb), ((16.0 * cb - 12.0) * cb + 4.0) * cb, cb <= vec3<f32>(0.25));
    return select(cb + (2.0 * cs - 1.0) * (d - cb), cb - (1.0 - 2.0 * cs) * cb * (1.0 - cb), cs <= vec3<f32>(0.5));
}

fn lum(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.3, 0.59, 0.11));
}

fn set_lum(c: vec3<f32>, l: f32) -> vec3<f32> {
    let o = c + (l - lum(c));
    let ol = lum(o);
    let n = min(min(o.r, o.g), o.b);
    let x = max(max(o.r, o.g), o.b);
    var r = o;
    if n < 0.0 {
        r = ol + (r - ol) * ol / max(ol - n, 1e-5);
    }
    if x > 1.0 {
        r = ol + (r - ol) * (1.0 - ol) / max(x - ol, 1e-5);
    }
    return r;
}

fn sat(c: vec3<f32>) -> f32 {
    return max(max(c.r, c.g), c.b) - min(min(c.r, c.g), c.b);
}

fn set_sat(c: vec3<f32>, s: f32) -> vec3<f32> {
    let n = min(min(c.r, c.g), c.b);
    let range = sat(c);
    return select(vec3<f32>(0.0), (c - n) * s / max(range, 1e-5), range > 1e-5);
}

// Mode numbers come from `backdrop_index` in lib.rs.
fn blend(mode: u32, cb: vec3<f32>, cs: vec3<f32>) -> vec3<f32> {
    switch mode {
        case 1u: { return min(cb, cs); }
        case 2u: { return max(cb, cs); }
        case 3u: { return color_burn(cb, cs); }
        case 4u: { return max(cb + cs - 1.0, vec3<f32>(0.0)); }
        case 5u: { return color_dodge(cb, cs); }
        case 6u: { return hard_light(cs, cb); }
        case 7u: { return soft_light(cb, cs); }
        case 8u: { return hard_light(cb, cs); }
        case 9u: {
            return select(color_dodge(cb, 2.0 * cs - 1.0), color_burn(cb, 2.0 * cs), cs <= vec3<f32>(0.5));
        }
        case 10u: { return cb + 2.0 * cs - 1.0; }
        case 11u: { return select(max(cb, 2.0 * cs - 1.0), min(cb, 2.0 * cs), cs <= vec3<f32>(0.5)); }
        case 12u: { return select(vec3<f32>(0.0), vec3<f32>(1.0), cb + cs >= vec3<f32>(1.0)); }
        case 13u: { return abs(cb - cs); }
        case 14u: { return cb / max(cs, vec3<f32>(1e-5)); }
        case 15u: { return set_lum(set_sat(cs, sat(cb)), lum(cb)); }
        case 16u: { return set_lum(set_sat(cb, sat(cs)), lum(cb)); }
        case 17u: { return set_lum(cs, lum(cb)); }
        case 18u: { return set_lum(cb, lum(cs)); }
        default: { return cs; }
    }
}

fn hue_rotate(c: vec3<f32>, turns: f32) -> vec3<f32> {
    let k = vec3<f32>(0.57735027);
    let ang = turns * TAU;
    let ca = cos(ang);
    return c * ca + cross(k, c) * sin(ang) + k * dot(k, c) * (1.0 - ca);
}

// Signed coverage distance (in uv units, > 0 inside) for the surface mask.
fn mask_distance(suv: vec2<f32>, mask: u32) -> f32 {
    switch mask {
        case 1u: {
            return 0.5 - length(suv - 0.5);
        }
        case 2u: {
            // Apex at top-center, base along the bottom edge.
            let l = (suv.x - (0.5 - 0.5 * suv.y));
            let r = ((0.5 + 0.5 * suv.y) - suv.x);
            return min(min(l, r) * 0.894, 1.0 - suv.y);
        }
        case 3u: {
            return polygon_distance(suv);
        }
        default: {
            return min(min(suv.x, 1.0 - suv.x), min(suv.y, 1.0 - suv.y));
        }
    }
}

fn outline_point(i: u32) -> vec2<f32> {
    let v = u.points[i / 2u];
    return select(v.xy, v.zw, (i & 1u) == 1u);
}

// Signed distance to the outline polygon, > 0 inside (even-odd rule).
fn polygon_distance(p: vec2<f32>) -> f32 {
    let n = u32(u.shape.x);
    if n < 3u {
        return -1.0;
    }
    var d = 1e9;
    var s = 1.0;
    var j = n - 1u;
    for (var i = 0u; i < n; i++) {
        let vi = outline_point(i);
        let vj = outline_point(j);
        let e = vj - vi;
        let w = p - vi;
        let b = w - e * clamp(dot(w, e) / max(dot(e, e), 1e-12), 0.0, 1.0);
        d = min(d, dot(b, b));
        let c1 = p.y >= vi.y;
        let c2 = p.y < vj.y;
        let c3 = e.x * w.y > e.y * w.x;
        if (c1 && c2 && c3) || (!c1 && !c2 && !c3) {
            s = -s;
        }
        j = i;
    }
    return -s * sqrt(d);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let inv_h = mat3x3<f32>(u.inv_h0.xyz, u.inv_h1.xyz, u.inv_h2.xyz);
    let hp = inv_h * vec3<f32>(in.canvas_px, 1.0);
    let suv = hp.xy / hp.z;

    let mapping = u.canvas.w;
    var base_uv = suv;
    if mapping > 0.5 {
        base_uv = in.canvas_px / u.canvas.xy;
    }
    let uv_m = mat3x3<f32>(u.uv_m0.xyz, u.uv_m1.xyz, u.uv_m2.xyz);
    let tuv = (uv_m * vec3<f32>(base_uv, 1.0)).xy;

    // Coverage: always at least ~1px of antialiasing, more with feather.
    let d = mask_distance(suv, u32(u.misc.w));
    let aa = max(fwidth(d), 1e-5);
    let coverage = smoothstep(0.0, max(u.misc.z, aa), d);

    let kind = u32(u.gen.x);
    let tex_color = textureSample(src_tex, src_samp, tuv);
    var color: vec4<f32>;
    if kind == 0u {
        color = tex_color;
    } else if kind == 1u {
        color = u.color_a;
    } else {
        color = generator(kind, tuv, u.canvas.z);
    }

    var rgb = clamp(hue_rotate(color.rgb, u.misc.y), vec3<f32>(0.0), vec3<f32>(1.0)) * u.tint.rgb;
    let alpha = color.a * u.tint.a * u.misc.x * coverage;
    // Backdrop blend modes (W3C compositing): blend with what's below, then
    // composite normally. Fixed-function modes have mode 0.
    let mode = u32(u.shape.y);
    if mode > 0u {
        let bd = textureLoad(backdrop, vec2<i32>(in.pos.xy), 0);
        let cb = bd.rgb / max(bd.a, 1e-5);
        rgb = mix(rgb, clamp(blend(mode, cb, rgb), vec3<f32>(0.0), vec3<f32>(1.0)), bd.a);
    }
    return vec4<f32>(rgb * alpha, alpha);
}
