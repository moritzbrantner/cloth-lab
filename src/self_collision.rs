use core::fmt;
use std::collections::BTreeSet;

use spatial_kernels::{Aabb, Axis3, Body, BroadPhase, NaiveBroadPhase, SweepAndPruneBroadPhase};

use crate::Vec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelfCollisionConfig {
    pub thickness: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelfCollisionError {
    InvalidThickness,
    InvalidParticlePosition,
    InvalidInverseMass,
    InvalidTriangleIndex,
    DuplicateTriangleVertex,
    TopologyTooLarge,
    BroadPhaseRangeExceeded,
}

impl fmt::Display for SelfCollisionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidThickness => "self-collision thickness must be finite and positive",
            Self::InvalidParticlePosition => {
                "self-collision particle positions must contain only finite components"
            }
            Self::InvalidInverseMass => {
                "self-collision inverse masses must be finite and non-negative"
            }
            Self::InvalidTriangleIndex => "self-collision triangle index is outside the particle set",
            Self::DuplicateTriangleVertex => {
                "self-collision triangles must contain three distinct particle indices"
            }
            Self::TopologyTooLarge => "self-collision topology must fit into shared u32 collider IDs",
            Self::BroadPhaseRangeExceeded => {
                "self-collision bounds must fit into the shared f32 broad-phase representation"
            }
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for SelfCollisionError {}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelfCollisionParticle {
    pub position: Vec3,
    pub inverse_mass: f64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SelfCollisionReport {
    pub broad_phase_pairs: usize,
    pub vertex_triangle_candidates: usize,
    pub adjacency_exclusions: usize,
    pub narrow_phase_tests: usize,
    pub projections: usize,
}

#[derive(Clone, Copy, Debug)]
struct ClosestTrianglePoint {
    point: Vec3,
    barycentric: [f64; 3],
    distance_squared: f64,
}

pub fn solve_vertex_triangle_self_collision(
    particles: &mut [SelfCollisionParticle],
    triangles: &[[usize; 3]],
    config: SelfCollisionConfig,
) -> Result<SelfCollisionReport, SelfCollisionError> {
    validate_inputs(particles, triangles, config)?;

    let bodies = broad_phase_bodies(particles, triangles, config.thickness);
    let broad_phase = SweepAndPruneBroadPhase::new(Axis3::X);
    let broad_phase_result = broad_phase.detect(&bodies);
    let mesh_edges = mesh_edges(triangles);
    let particle_count = particles.len();
    let mut report = SelfCollisionReport {
        broad_phase_pairs: broad_phase_result.pairs.len(),
        ..SelfCollisionReport::default()
    };

    for pair in broad_phase_result.pairs {
        let Some((particle_index, triangle_index)) = decode_vertex_triangle_pair(
            pair.a as usize,
            pair.b as usize,
            particle_count,
            triangles.len(),
        ) else {
            continue;
        };
        report.vertex_triangle_candidates += 1;
        let triangle = triangles[triangle_index];
        if is_adjacent_feature(particle_index, triangle, &mesh_edges) {
            report.adjacency_exclusions += 1;
            continue;
        }
        report.narrow_phase_tests += 1;
        report.projections += usize::from(project_vertex_triangle(
            particles,
            particle_index,
            triangle,
            config.thickness,
            triangle_index,
        ));
    }

    Ok(report)
}

fn validate_inputs(
    particles: &[SelfCollisionParticle],
    triangles: &[[usize; 3]],
    config: SelfCollisionConfig,
) -> Result<(), SelfCollisionError> {
    if !config.thickness.is_finite() || config.thickness <= 0.0 {
        return Err(SelfCollisionError::InvalidThickness);
    }
    let total = particles
        .len()
        .checked_add(triangles.len())
        .ok_or(SelfCollisionError::TopologyTooLarge)?;
    if total > u32::MAX as usize {
        return Err(SelfCollisionError::TopologyTooLarge);
    }

    let broad_phase_limit = f64::from(f32::MAX);
    for particle in particles {
        if !particle.position.is_finite() {
            return Err(SelfCollisionError::InvalidParticlePosition);
        }
        if !particle.inverse_mass.is_finite() || particle.inverse_mass < 0.0 {
            return Err(SelfCollisionError::InvalidInverseMass);
        }
        for coordinate in [particle.position.x, particle.position.y, particle.position.z] {
            if coordinate.abs() + config.thickness > broad_phase_limit {
                return Err(SelfCollisionError::BroadPhaseRangeExceeded);
            }
        }
    }

    for &[a, b, c] in triangles {
        if a >= particles.len() || b >= particles.len() || c >= particles.len() {
            return Err(SelfCollisionError::InvalidTriangleIndex);
        }
        if a == b || b == c || c == a {
            return Err(SelfCollisionError::DuplicateTriangleVertex);
        }
    }
    Ok(())
}

fn broad_phase_bodies(
    particles: &[SelfCollisionParticle],
    triangles: &[[usize; 3]],
    thickness: f64,
) -> Vec<Body> {
    let mut bodies = Vec::with_capacity(particles.len() + triangles.len());
    for (index, particle) in particles.iter().enumerate() {
        let min = particle.position - Vec3::new(thickness, thickness, thickness);
        let max = particle.position + Vec3::new(thickness, thickness, thickness);
        bodies.push(Body::new(index as u32, conservative_aabb(min, max)));
    }
    let triangle_offset = particles.len();
    for (index, &[a, b, c]) in triangles.iter().enumerate() {
        let pa = particles[a].position;
        let pb = particles[b].position;
        let pc = particles[c].position;
        let min = Vec3::new(
            pa.x.min(pb.x).min(pc.x),
            pa.y.min(pb.y).min(pc.y),
            pa.z.min(pb.z).min(pc.z),
        );
        let max = Vec3::new(
            pa.x.max(pb.x).max(pc.x),
            pa.y.max(pb.y).max(pc.y),
            pa.z.max(pb.z).max(pc.z),
        );
        bodies.push(Body::new(
            (triangle_offset + index) as u32,
            conservative_aabb(min, max),
        ));
    }
    bodies
}

fn conservative_aabb(min: Vec3, max: Vec3) -> Aabb {
    Aabb::new(
        [lower_f32(min.x), lower_f32(min.y), lower_f32(min.z)],
        [upper_f32(max.x), upper_f32(max.y), upper_f32(max.z)],
    )
}

fn lower_f32(value: f64) -> f32 {
    let rounded = value as f32;
    if f64::from(rounded) > value {
        next_down(rounded)
    } else {
        rounded
    }
}

fn upper_f32(value: f64) -> f32 {
    let rounded = value as f32;
    if f64::from(rounded) < value {
        next_up(rounded)
    } else {
        rounded
    }
}

fn next_down(value: f32) -> f32 {
    if value == 0.0 {
        return -f32::from_bits(1);
    }
    if value > 0.0 {
        f32::from_bits(value.to_bits() - 1)
    } else {
        f32::from_bits(value.to_bits() + 1)
    }
}

fn next_up(value: f32) -> f32 {
    if value == 0.0 {
        return f32::from_bits(1);
    }
    if value > 0.0 {
        f32::from_bits(value.to_bits() + 1)
    } else {
        f32::from_bits(value.to_bits() - 1)
    }
}

fn decode_vertex_triangle_pair(
    first: usize,
    second: usize,
    particle_count: usize,
    triangle_count: usize,
) -> Option<(usize, usize)> {
    let triangle_end = particle_count + triangle_count;
    match (first < particle_count, second < particle_count) {
        (true, false) if second < triangle_end => Some((first, second - particle_count)),
        (false, true) if first < triangle_end => Some((second, first - particle_count)),
        _ => None,
    }
}

fn mesh_edges(triangles: &[[usize; 3]]) -> BTreeSet<(usize, usize)> {
    let mut edges = BTreeSet::new();
    for &[a, b, c] in triangles {
        for (left, right) in [(a, b), (b, c), (c, a)] {
            edges.insert(ordered_pair(left, right));
        }
    }
    edges
}

fn ordered_pair(left: usize, right: usize) -> (usize, usize) {
    if left < right {
        (left, right)
    } else {
        (right, left)
    }
}

fn is_adjacent_feature(
    particle: usize,
    triangle: [usize; 3],
    mesh_edges: &BTreeSet<(usize, usize)>,
) -> bool {
    triangle.contains(&particle)
        || triangle
            .into_iter()
            .any(|vertex| mesh_edges.contains(&ordered_pair(particle, vertex)))
}

fn project_vertex_triangle(
    particles: &mut [SelfCollisionParticle],
    particle_index: usize,
    triangle: [usize; 3],
    thickness: f64,
    triangle_index: usize,
) -> bool {
    let [a, b, c] = triangle;
    let point = particles[particle_index].position;
    let pa = particles[a].position;
    let pb = particles[b].position;
    let pc = particles[c].position;
    let closest = closest_point_on_triangle(point, pa, pb, pc);
    if closest.distance_squared >= thickness * thickness {
        return false;
    }

    let distance = closest.distance_squared.sqrt();
    let normal = if distance > f64::EPSILON {
        (point - closest.point) / distance
    } else {
        deterministic_triangle_normal(pa, pb, pc, particle_index, triangle_index)
    };
    let [wa, wb, wc] = closest.barycentric;
    let point_mass = particles[particle_index].inverse_mass;
    let mass_a = particles[a].inverse_mass;
    let mass_b = particles[b].inverse_mass;
    let mass_c = particles[c].inverse_mass;
    let denominator = point_mass + mass_a * wa * wa + mass_b * wb * wb + mass_c * wc * wc;
    if denominator <= f64::EPSILON {
        return false;
    }

    let lambda = (thickness - distance) / denominator;
    particles[particle_index].position += normal * (point_mass * lambda);
    particles[a].position -= normal * (mass_a * wa * lambda);
    particles[b].position -= normal * (mass_b * wb * lambda);
    particles[c].position -= normal * (mass_c * wc * lambda);
    true
}

fn closest_point_on_triangle(point: Vec3, a: Vec3, b: Vec3, c: Vec3) -> ClosestTrianglePoint {
    let ab = b - a;
    let ac = c - a;
    if cross(ab, ac).length_squared() <= f64::EPSILON {
        return closest_point_on_degenerate_triangle(point, a, b, c);
    }

    let ap = point - a;
    let d1 = dot(ab, ap);
    let d2 = dot(ac, ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return closest_result(point, a, [1.0, 0.0, 0.0]);
    }

    let bp = point - b;
    let d3 = dot(ab, bp);
    let d4 = dot(ac, bp);
    if d3 >= 0.0 && d4 <= d3 {
        return closest_result(point, b, [0.0, 1.0, 0.0]);
    }

    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return closest_result(point, a + ab * v, [1.0 - v, v, 0.0]);
    }

    let cp = point - c;
    let d5 = dot(ab, cp);
    let d6 = dot(ac, cp);
    if d6 >= 0.0 && d5 <= d6 {
        return closest_result(point, c, [0.0, 0.0, 1.0]);
    }

    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return closest_result(point, a + ac * w, [1.0 - w, 0.0, w]);
    }

    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let edge = c - b;
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return closest_result(point, b + edge * w, [0.0, 1.0 - w, w]);
    }

    let denominator = 1.0 / (va + vb + vc);
    let v = vb * denominator;
    let w = vc * denominator;
    let u = 1.0 - v - w;
    closest_result(point, a * u + b * v + c * w, [u, v, w])
}

fn closest_point_on_degenerate_triangle(
    point: Vec3,
    a: Vec3,
    b: Vec3,
    c: Vec3,
) -> ClosestTrianglePoint {
    let candidates = [
        closest_point_on_edge(point, a, b, [0, 1]),
        closest_point_on_edge(point, b, c, [1, 2]),
        closest_point_on_edge(point, c, a, [2, 0]),
    ];
    candidates
        .into_iter()
        .min_by(|left, right| left.distance_squared.total_cmp(&right.distance_squared))
        .expect("three degenerate-triangle edge candidates always exist")
}

fn closest_point_on_edge(
    point: Vec3,
    start: Vec3,
    end: Vec3,
    barycentric_indices: [usize; 2],
) -> ClosestTrianglePoint {
    let edge = end - start;
    let length_squared = edge.length_squared();
    let fraction = if length_squared <= f64::EPSILON {
        0.0
    } else {
        (dot(point - start, edge) / length_squared).clamp(0.0, 1.0)
    };
    let mut barycentric = [0.0; 3];
    barycentric[barycentric_indices[0]] = 1.0 - fraction;
    barycentric[barycentric_indices[1]] = fraction;
    closest_result(point, start + edge * fraction, barycentric)
}

fn closest_result(point: Vec3, closest: Vec3, barycentric: [f64; 3]) -> ClosestTrianglePoint {
    ClosestTrianglePoint {
        point: closest,
        barycentric,
        distance_squared: (point - closest).length_squared(),
    }
}

fn deterministic_triangle_normal(
    a: Vec3,
    b: Vec3,
    c: Vec3,
    particle_index: usize,
    triangle_index: usize,
) -> Vec3 {
    let triangle_normal = cross(b - a, c - a);
    if triangle_normal.length_squared() > f64::EPSILON {
        return canonicalize_normal(triangle_normal / triangle_normal.length());
    }

    let edges = [b - a, c - b, a - c];
    let longest = edges
        .into_iter()
        .max_by(|left, right| left.length_squared().total_cmp(&right.length_squared()))
        .unwrap_or(Vec3::ZERO);
    if longest.length_squared() > f64::EPSILON {
        return canonicalize_normal(deterministic_perpendicular(longest));
    }

    if (particle_index + triangle_index).is_multiple_of(2) {
        Vec3::new(0.0, 1.0, 0.0)
    } else {
        Vec3::new(0.0, -1.0, 0.0)
    }
}

fn deterministic_perpendicular(axis: Vec3) -> Vec3 {
    let absolute_x = axis.x.abs();
    let absolute_y = axis.y.abs();
    let absolute_z = axis.z.abs();
    let perpendicular = if absolute_x <= absolute_y && absolute_x <= absolute_z {
        Vec3::new(0.0, -axis.z, axis.y)
    } else if absolute_y <= absolute_z {
        Vec3::new(-axis.z, 0.0, axis.x)
    } else {
        Vec3::new(-axis.y, axis.x, 0.0)
    };
    perpendicular / perpendicular.length()
}

fn canonicalize_normal(normal: Vec3) -> Vec3 {
    let sign = if normal.x.abs() > f64::EPSILON {
        normal.x
    } else if normal.y.abs() > f64::EPSILON {
        normal.y
    } else {
        normal.z
    };
    if sign < 0.0 { normal * -1.0 } else { normal }
}

fn cross(left: Vec3, right: Vec3) -> Vec3 {
    Vec3::new(
        left.y * right.z - left.z * right.y,
        left.z * right.x - left.x * right.z,
        left.x * right.y - left.y * right.x,
    )
}

fn dot(left: Vec3, right: Vec3) -> f64 {
    left.x * right.x + left.y * right.y + left.z * right.z
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (Vec<SelfCollisionParticle>, Vec<[usize; 3]>) {
        (
            vec![
                SelfCollisionParticle {
                    position: Vec3::new(-0.3, 0.0, 0.0),
                    inverse_mass: 0.0,
                },
                SelfCollisionParticle {
                    position: Vec3::new(0.3, 0.0, 0.0),
                    inverse_mass: 0.0,
                },
                SelfCollisionParticle {
                    position: Vec3::new(0.0, 0.0, 0.3),
                    inverse_mass: 0.0,
                },
                SelfCollisionParticle {
                    position: Vec3::new(0.0, 0.01, 0.1),
                    inverse_mass: 1.0,
                },
            ],
            vec![[0, 1, 2]],
        )
    }

    #[test]
    fn separates_non_adjacent_vertex_from_triangle_and_excludes_local_features() {
        let (mut particles, triangles) = fixture();
        let report = solve_vertex_triangle_self_collision(
            &mut particles,
            &triangles,
            SelfCollisionConfig { thickness: 0.08 },
        )
        .unwrap();
        assert_eq!(report.projections, 1);
        assert!(report.adjacency_exclusions >= 3);
        assert!((particles[3].position.y - 0.08).abs() < 1.0e-12);
    }

    #[test]
    fn replay_is_deterministic() {
        let (mut first, triangles) = fixture();
        let mut second = first.clone();
        let config = SelfCollisionConfig { thickness: 0.08 };
        let first_report =
            solve_vertex_triangle_self_collision(&mut first, &triangles, config).unwrap();
        let second_report =
            solve_vertex_triangle_self_collision(&mut second, &triangles, config).unwrap();
        assert_eq!(first_report, second_report);
        assert_eq!(first, second);
    }

    #[test]
    fn shared_sweep_and_prune_matches_naive_reference() {
        let (particles, triangles) = fixture();
        let bodies = broad_phase_bodies(&particles, &triangles, 0.08);
        let sweep = SweepAndPruneBroadPhase::new(Axis3::X).detect(&bodies);
        let naive = NaiveBroadPhase.detect(&bodies);
        assert_eq!(sweep.pairs, naive.pairs);
    }

    #[test]
    fn invalid_input_fails_before_mutating_particles() {
        let (mut particles, triangles) = fixture();
        let before = particles.clone();
        let error = solve_vertex_triangle_self_collision(
            &mut particles,
            &triangles,
            SelfCollisionConfig { thickness: -0.1 },
        )
        .unwrap_err();
        assert_eq!(error, SelfCollisionError::InvalidThickness);
        assert_eq!(particles, before);
    }

    #[test]
    fn degenerate_triangle_uses_deterministic_edge_fallback() {
        let mut particles = vec![
            SelfCollisionParticle {
                position: Vec3::new(-0.2, 0.0, 0.0),
                inverse_mass: 0.0,
            },
            SelfCollisionParticle {
                position: Vec3::new(0.0, 0.0, 0.0),
                inverse_mass: 0.0,
            },
            SelfCollisionParticle {
                position: Vec3::new(0.2, 0.0, 0.0),
                inverse_mass: 0.0,
            },
            SelfCollisionParticle {
                position: Vec3::new(0.05, 0.02, 0.0),
                inverse_mass: 1.0,
            },
        ];
        let report = solve_vertex_triangle_self_collision(
            &mut particles,
            &[[0, 1, 2]],
            SelfCollisionConfig { thickness: 0.05 },
        )
        .unwrap();
        assert_eq!(report.projections, 1);
        assert!((particles[3].position.y.abs() - 0.05).abs() < 1.0e-12);
    }
}
