use super::geometry::dot;
use super::{CapsuleCollider, ClothCollider, ContactConfig, Particle, SphereCollider};
use crate::Vec3;

#[derive(Clone, Copy, Debug)]
pub(super) struct CollisionProjection {
    pub(super) normal: Vec3,
    pub(super) normal_correction: f64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct CollisionSolveReport {
    pub(super) projections: usize,
    pub(super) friction_corrections: usize,
}

pub(super) fn solve_collisions(
    particles: &mut [Particle],
    colliders: &[ClothCollider],
    contact: ContactConfig,
) -> CollisionSolveReport {
    let mut report = CollisionSolveReport::default();
    for particle in particles {
        if particle.is_pinned() {
            continue;
        }
        for collider in colliders {
            let projection = match collider {
                ClothCollider::Sphere(sphere) => solve_sphere_collision(particle, *sphere),
                ClothCollider::Capsule(capsule) => solve_capsule_collision(particle, *capsule),
            };
            if let Some(projection) = projection {
                report.projections += 1;
                report.friction_corrections += usize::from(apply_contact_friction(
                    particle,
                    projection,
                    contact.friction_coefficient,
                ));
            }
        }
    }
    report
}

pub(super) fn solve_sphere_collision(
    particle: &mut Particle,
    collider: SphereCollider,
) -> Option<CollisionProjection> {
    let effective_radius = collider.effective_radius();
    let current_delta = particle.position - collider.center;
    let current_distance = current_delta.length();

    if current_distance < effective_radius {
        let previous_delta = particle.previous_position - collider.center;
        let normal = collision_normal(current_delta, previous_delta);
        return Some(project_particle(
            particle,
            collider.center + normal * effective_radius,
            normal,
        ));
    }

    let movement = particle.position - particle.previous_position;
    let movement_squared = movement.length_squared();
    if movement_squared <= f64::EPSILON {
        return None;
    }

    let start_delta = particle.previous_position - collider.center;
    let radius_squared = effective_radius * effective_radius;
    let start_distance_squared = start_delta.length_squared();
    if start_distance_squared <= radius_squared {
        return None;
    }

    let projection = dot(start_delta, movement);
    let constant = start_distance_squared - radius_squared;
    let discriminant = projection * projection - movement_squared * constant;
    if discriminant < 0.0 {
        return None;
    }

    let hit_fraction = (-projection - discriminant.sqrt()) / movement_squared;
    if !(0.0..=1.0).contains(&hit_fraction) {
        return None;
    }

    let hit = particle.previous_position + movement * hit_fraction;
    let hit_delta = hit - collider.center;
    let normal = collision_normal(hit_delta, start_delta);
    Some(project_particle(
        particle,
        collider.center + normal * effective_radius,
        normal,
    ))
}

pub(super) fn solve_capsule_collision(
    particle: &mut Particle,
    collider: CapsuleCollider,
) -> Option<CollisionProjection> {
    let effective_radius = collider.effective_radius();
    let radius_squared = effective_radius * effective_radius;
    let axis = collider.end - collider.start;
    let current_axis_point =
        closest_point_on_segment(particle.position, collider.start, collider.end);
    let current_delta = particle.position - current_axis_point;

    if current_delta.length_squared() < radius_squared {
        let previous_axis_point =
            closest_point_on_segment(particle.previous_position, collider.start, collider.end);
        let previous_delta = particle.previous_position - previous_axis_point;
        let normal = capsule_collision_normal(current_delta, previous_delta, axis);
        return Some(project_particle(
            particle,
            current_axis_point + normal * effective_radius,
            normal,
        ));
    }

    let movement = particle.position - particle.previous_position;
    if movement.length_squared() <= f64::EPSILON {
        return None;
    }

    let start_axis_point =
        closest_point_on_segment(particle.previous_position, collider.start, collider.end);
    let start_delta = particle.previous_position - start_axis_point;
    if start_delta.length_squared() <= radius_squared {
        return None;
    }

    let (closest_fraction, closest_distance_squared) = closest_path_fraction_to_segment(
        particle.previous_position,
        particle.position,
        collider.start,
        collider.end,
    );
    if closest_distance_squared > radius_squared || closest_fraction <= f64::EPSILON {
        return None;
    }

    let mut outside_fraction = 0.0;
    let mut inside_fraction = closest_fraction;
    for _ in 0..48 {
        let candidate_fraction = (outside_fraction + inside_fraction) * 0.5;
        let candidate = particle.previous_position + movement * candidate_fraction;
        let candidate_axis_point =
            closest_point_on_segment(candidate, collider.start, collider.end);
        let distance_squared = (candidate - candidate_axis_point).length_squared();
        if distance_squared > radius_squared {
            outside_fraction = candidate_fraction;
        } else {
            inside_fraction = candidate_fraction;
        }
    }

    let hit = particle.previous_position + movement * inside_fraction;
    let hit_axis_point = closest_point_on_segment(hit, collider.start, collider.end);
    let hit_delta = hit - hit_axis_point;
    let normal = capsule_collision_normal(hit_delta, start_delta, axis);
    Some(project_particle(
        particle,
        hit_axis_point + normal * effective_radius,
        normal,
    ))
}

fn project_particle(
    particle: &mut Particle,
    projected_position: Vec3,
    normal: Vec3,
) -> CollisionProjection {
    let correction = projected_position - particle.position;
    particle.position = projected_position;
    CollisionProjection {
        normal,
        normal_correction: dot(correction, normal).abs(),
    }
}

pub(super) fn apply_contact_friction(
    particle: &mut Particle,
    projection: CollisionProjection,
    friction_coefficient: f64,
) -> bool {
    if friction_coefficient <= f64::EPSILON || projection.normal_correction <= f64::EPSILON {
        return false;
    }

    let displacement = particle.position - particle.previous_position;
    let normal_displacement = projection.normal * dot(displacement, projection.normal);
    let tangential_displacement = displacement - normal_displacement;
    let tangential_length = tangential_displacement.length();
    if tangential_length <= f64::EPSILON {
        return false;
    }

    let correction_length =
        (friction_coefficient * projection.normal_correction).min(tangential_length);
    if correction_length <= f64::EPSILON {
        return false;
    }

    particle.previous_position += tangential_displacement * (correction_length / tangential_length);
    true
}

pub(super) fn closest_point_on_segment(point: Vec3, start: Vec3, end: Vec3) -> Vec3 {
    let segment = end - start;
    let segment_length_squared = segment.length_squared();
    debug_assert!(segment_length_squared > f64::EPSILON);
    let fraction = (dot(point - start, segment) / segment_length_squared).clamp(0.0, 1.0);
    start + segment * fraction
}

fn closest_path_fraction_to_segment(
    path_start: Vec3,
    path_end: Vec3,
    segment_start: Vec3,
    segment_end: Vec3,
) -> (f64, f64) {
    let path = path_end - path_start;
    let segment = segment_end - segment_start;
    let offset = path_start - segment_start;
    let path_length_squared = dot(path, path);
    let segment_projection = dot(path, segment);
    let segment_length_squared = dot(segment, segment);
    let path_offset_projection = dot(path, offset);
    let segment_offset_projection = dot(segment, offset);
    let denominator =
        path_length_squared * segment_length_squared - segment_projection * segment_projection;

    let mut path_numerator;
    let mut path_denominator = denominator;
    let mut segment_numerator;
    let mut segment_denominator = denominator;

    if denominator <= f64::EPSILON {
        path_numerator = 0.0;
        path_denominator = 1.0;
        segment_numerator = segment_offset_projection;
        segment_denominator = segment_length_squared;
    } else {
        path_numerator = segment_projection * segment_offset_projection
            - segment_length_squared * path_offset_projection;
        segment_numerator = path_length_squared * segment_offset_projection
            - segment_projection * path_offset_projection;

        if path_numerator < 0.0 {
            path_numerator = 0.0;
            segment_numerator = segment_offset_projection;
            segment_denominator = segment_length_squared;
        } else if path_numerator > path_denominator {
            path_numerator = path_denominator;
            segment_numerator = segment_offset_projection + segment_projection;
            segment_denominator = segment_length_squared;
        }
    }

    if segment_numerator < 0.0 {
        segment_numerator = 0.0;
        if -path_offset_projection < 0.0 {
            path_numerator = 0.0;
        } else if -path_offset_projection > path_length_squared {
            path_numerator = path_denominator;
        } else {
            path_numerator = -path_offset_projection;
            path_denominator = path_length_squared;
        }
    } else if segment_numerator > segment_denominator {
        segment_numerator = segment_denominator;
        let end_projection = -path_offset_projection + segment_projection;
        if end_projection < 0.0 {
            path_numerator = 0.0;
        } else if end_projection > path_length_squared {
            path_numerator = path_denominator;
        } else {
            path_numerator = end_projection;
            path_denominator = path_length_squared;
        }
    }

    let path_fraction = if path_numerator.abs() <= f64::EPSILON {
        0.0
    } else {
        path_numerator / path_denominator
    };
    let segment_fraction = if segment_numerator.abs() <= f64::EPSILON {
        0.0
    } else {
        segment_numerator / segment_denominator
    };
    let separation = offset + path * path_fraction - segment * segment_fraction;
    (path_fraction, separation.length_squared())
}

fn collision_normal(primary: Vec3, fallback: Vec3) -> Vec3 {
    let primary_length = primary.length();
    if primary_length > f64::EPSILON {
        return primary / primary_length;
    }
    let fallback_length = fallback.length();
    if fallback_length > f64::EPSILON {
        return fallback / fallback_length;
    }
    Vec3::new(0.0, 1.0, 0.0)
}

fn capsule_collision_normal(primary: Vec3, fallback: Vec3, axis: Vec3) -> Vec3 {
    let primary_length = primary.length();
    if primary_length > f64::EPSILON {
        return primary / primary_length;
    }
    let fallback_length = fallback.length();
    if fallback_length > f64::EPSILON {
        return fallback / fallback_length;
    }
    deterministic_perpendicular(axis)
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
