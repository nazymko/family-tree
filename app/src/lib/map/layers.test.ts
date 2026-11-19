// Placement math of old maps: corners to the transform, the mesh, defaults and moves. Invented coordinates only.
import { describe, expect, it } from 'vitest';
import type { Camera } from './projection';
import { mercator } from './projection';
import {
  affine,
  cornersToWorld,
  defaultCorners,
  homography,
  mesh,
  meshSize,
  moveAll,
  moveCorner,
  parseDegrees,
  project,
  reordered,
  screenQuad,
  skew,
  wantsMore,
  worldToCorners,
  type Pt,
  type Quad,
} from './layers';

const near = (a: number, b: number, eps = 1e-6) => expect(Math.abs(a - b)).toBeLessThan(eps);
const unit: Pt[] = [
  [0, 0],
  [1, 0],
  [1, 1],
  [0, 1],
];

describe('map layer placement', () => {
  it('corners_map_to_the_four_screen_points_and_the_middle_follows_the_perspective', () => {
    const q: Quad = [
      [100, 50],
      [500, 80],
      [460, 400],
      [90, 380],
    ];
    const h = homography(unit, q)!;
    for (let i = 0; i < 4; i++) {
      const [x, y] = project(h, unit[i][0], unit[i][1]);
      near(x, q[i][0]);
      near(y, q[i][1]);
    }
    // The middle of the picture is where the diagonals cross, not the average of the corners.
    const [mx, my] = project(h, 0.5, 0.5);
    const cross = (a: Pt, b: Pt, c: Pt, d: Pt): Pt => {
      const t = ((c[0] - a[0]) * (d[1] - c[1]) - (c[1] - a[1]) * (d[0] - c[0])) / ((b[0] - a[0]) * (d[1] - c[1]) - (b[1] - a[1]) * (d[0] - c[0]));
      return [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])];
    };
    const [cx, cy] = cross(q[0], q[2], q[1], q[3]);
    near(mx, cx, 1e-6);
    near(my, cy, 1e-6);
  });

  it('a_flat_quad_has_no_transform', () => {
    expect(
      homography(unit, [
        [0, 0],
        [1, 0],
        [2, 0],
        [3, 0],
      ]),
    ).toBeNull();
    expect(
      affine(
        [
          [0, 0],
          [1, 1],
          [2, 2],
        ],
        [
          [0, 0],
          [1, 0],
          [0, 1],
        ],
      ),
    ).toBeNull();
  });

  it('an_affine_piece_takes_each_picture_point_to_its_screen_point', () => {
    const s: [Pt, Pt, Pt] = [
      [0, 0],
      [200, 0],
      [0, 100],
    ];
    const d: [Pt, Pt, Pt] = [
      [10, 20],
      [210, 60],
      [-10, 130],
    ];
    const [a, b, c, dd, e, f] = affine(s, d)!;
    for (let i = 0; i < 3; i++) {
      near(a * s[i][0] + c * s[i][1] + e, d[i][0]);
      near(b * s[i][0] + dd * s[i][1] + f, d[i][1]);
    }
  });

  it('a_parallelogram_is_one_piece_and_a_trapezoid_a_finer_mesh', () => {
    const par: Quad = [
      [0, 0],
      [400, 20],
      [420, 320],
      [20, 300],
    ];
    expect(skew(par)).toBeLessThan(1e-9);
    expect(meshSize(par)).toBe(1);
    const trap: Quad = [
      [0, 0],
      [400, 0],
      [350, 300],
      [50, 300],
    ];
    const n = meshSize(trap);
    expect(n).toBeGreaterThan(1);
    expect(n).toBeLessThanOrEqual(16);
    const pieces = mesh(trap, 2000, 1500, n);
    expect(pieces).toHaveLength(2 * n * n);
    // The pieces cover the picture: their picture corners run from (0, 0) to (2000, 1500).
    const xs = pieces.flatMap((p) => p.src.map((s) => s[0]));
    const ys = pieces.flatMap((p) => p.src.map((s) => s[1]));
    expect(Math.min(...xs)).toBe(0);
    expect(Math.max(...xs)).toBe(2000);
    expect(Math.max(...ys)).toBe(1500);
  });

  it('corners_and_world_points_go_both_ways', () => {
    const c = [
      { latE6: 50_600_000, lonE6: 30_000_000 },
      { latE6: 50_600_000, lonE6: 31_200_000 },
      { latE6: 49_900_000, lonE6: 31_200_000 },
      { latE6: 49_900_000, lonE6: 30_000_000 },
    ];
    const back = worldToCorners(cornersToWorld(c));
    back.forEach((p, i) => {
      expect(Math.abs(p.latE6 - c[i].latE6)).toBeLessThanOrEqual(1);
      expect(Math.abs(p.lonE6 - c[i].lonE6)).toBeLessThanOrEqual(1);
    });
  });

  it('a_new_map_starts_in_the_middle_of_the_view_with_the_pictures_shape', () => {
    const w0 = mercator(50.3, 30.6);
    const cam: Camera = { cx: w0.x, cy: w0.y, zoom: 9 };
    const c = defaultCorners(cam, 1000, 600, 3000, 2000);
    const q = screenQuad(cornersToWorld(c), cam, 1000, 600);
    const width = q[1][0] - q[0][0];
    const height = q[3][1] - q[0][1];
    near(width / height, 1.5, 1e-3);
    near((q[0][0] + q[2][0]) / 2, 500, 0.5);
    near((q[0][1] + q[2][1]) / 2, 300, 0.5);
    expect(height).toBeLessThanOrEqual(360.5);
    // North stays up: the top corners have the larger latitude.
    expect(c[0].latE6).toBeGreaterThan(c[3].latE6);
  });

  it('dragging_moves_one_corner_or_all_of_them', () => {
    const w0 = mercator(50.3, 30.6);
    const cam: Camera = { cx: w0.x, cy: w0.y, zoom: 8 };
    const c = defaultCorners(cam, 800, 600, 4, 3);
    const one = moveCorner(c, 2, cam, 800, 600, 700, 550);
    const q = screenQuad(cornersToWorld(one), cam, 800, 600);
    near(q[2][0], 700, 0.05);
    near(q[2][1], 550, 0.05);
    expect(one[0]).toEqual(c[0]);
    const before = screenQuad(cornersToWorld(c), cam, 800, 600);
    const all = screenQuad(cornersToWorld(moveAll(c, cam, 800, 600, 30, -20)), cam, 800, 600);
    all.forEach((p, i) => {
      near(p[0] - before[i][0], 30, 0.05);
      near(p[1] - before[i][1], -20, 0.05);
    });
  });

  it('a_closer_view_asks_for_the_original_once_the_preview_is_too_small', () => {
    const small: Quad = [
      [0, 0],
      [800, 0],
      [800, 600],
      [0, 600],
    ];
    expect(wantsMore(small, 1, 1600, 8000)).toBe(false);
    const big: Quad = [
      [0, 0],
      [4000, 0],
      [4000, 3000],
      [0, 3000],
    ];
    expect(wantsMore(big, 1, 1600, 8000)).toBe(true);
    // Nothing more to get: the preview is the whole picture already.
    expect(wantsMore(big, 1, 1600, 1600)).toBe(false);
  });

  it('reorders_and_reads_typed_degrees', () => {
    expect(reordered(['a', 'b', 'c'], 'a', 1)).toEqual(['b', 'a', 'c']);
    expect(reordered(['a', 'b', 'c'], 'a', -1)).toEqual(['a', 'b', 'c']);
    expect(reordered(['a', 'b', 'c'], 'c', -1)).toEqual(['a', 'c', 'b']);
    expect(parseDegrees('50,45', 90)).toBe(50_450_000);
    expect(parseDegrees(' -30.5 ', 180)).toBe(-30_500_000);
    expect(parseDegrees('91', 90)).toBeNull();
    expect(parseDegrees('', 90)).toBeNull();
    expect(parseDegrees('north', 90)).toBeNull();
  });
});
