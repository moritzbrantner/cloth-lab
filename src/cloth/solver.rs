use super::geometry::dot;
use super::{BendingConstraint, DistanceConstraint, Particle};
use crate::Vec3;

pub(super) fn solve_distance_constraint(
    particles: &mut [Particle],
    constraint: &mut DistanceConstraint,
    delta_seconds: f64,
) {
    let (particle_a, particle_b) = two_mut(particles, constraint.particle_a, constraint.particle_b);
    let delta = particle_a.position - particle_b.position;
    let length = delta.length();
    if length <= f64::EPSILON {
        return;
    }
    let weight_sum = particle_a.inverse_mass + particle_b.inverse_mass;
    if weight_sum <= f64::EPSILON {
        return;
    }
    let alpha = constraint.compliance / (delta_seconds * delta_seconds);
    let constraint_error = length - constraint.rest_length;
    let delta_lambda = (-constraint_error - alpha * constraint.lambda) / (weight_sum + alpha);
    constraint.lambda += delta_lambda;
    let normal = delta / length;
    particle_a.position += normal * (particle_a.inverse_mass * delta_lambda);
    particle_b.position -= normal * (particle_b.inverse_mass * delta_lambda);
}

pub(super) fn solve_bending_constraint(
    particles: &mut [Particle],
    constraint: &mut BendingConstraint,
    delta_seconds: f64,
) {
    let indices = [
        constraint.edge_a,
        constraint.edge_b,
        constraint.opposite_a,
        constraint.opposite_b,
    ];
    let positions = indices.map(|index| particles[index].position);
    let inverse_masses = indices.map(|index| particles[index].inverse_mass);
    let energy = bending_energy(&positions, &constraint.q);

    let mut gradients = [Vec3::ZERO; 4];
    for (row, gradient) in gradients.iter_mut().enumerate() {
        for (column, position) in positions.iter().copied().enumerate() {
            *gradient += position * constraint.q[row][column];
        }
    }

    let alpha = constraint.compliance / (delta_seconds * delta_seconds);
    let denominator = gradients
        .iter()
        .zip(inverse_masses)
        .fold(alpha, |sum, (gradient, inverse_mass)| {
            sum + inverse_mass * gradient.length_squared()
        });
    if !denominator.is_finite() || denominator <= f64::EPSILON {
        return;
    }

    let delta_lambda = -(energy + alpha * constraint.lambda) / denominator;
    if !delta_lambda.is_finite() {
        return;
    }
    constraint.lambda += delta_lambda;

    for ((index, gradient), inverse_mass) in indices.into_iter().zip(gradients).zip(inverse_masses)
    {
        particles[index].position += gradient * (inverse_mass * delta_lambda);
    }
}

pub(super) fn max_constraint_error(
    particles: &[Particle],
    constraints: &[DistanceConstraint],
) -> f64 {
    constraints
        .iter()
        .map(|constraint| {
            let delta = particles[constraint.particle_a].position
                - particles[constraint.particle_b].position;
            (delta.length() - constraint.rest_length).abs()
        })
        .fold(0.0, f64::max)
}

pub(super) fn bending_constraint_value(
    particles: &[Particle],
    constraint: &BendingConstraint,
) -> f64 {
    let indices = [
        constraint.edge_a,
        constraint.edge_b,
        constraint.opposite_a,
        constraint.opposite_b,
    ];
    let positions = indices.map(|index| particles[index].position);
    bending_energy(&positions, &constraint.q)
}

fn bending_energy(positions: &[Vec3; 4], q: &[[f64; 4]; 4]) -> f64 {
    let mut energy = 0.0;
    for (row, q_row) in q.iter().enumerate() {
        for (column, coefficient) in q_row.iter().copied().enumerate() {
            energy += coefficient * dot(positions[column], positions[row]);
        }
    }
    energy * 0.5
}

fn two_mut<T>(slice: &mut [T], first: usize, second: usize) -> (&mut T, &mut T) {
    debug_assert_ne!(first, second);
    if first < second {
        let (left, right) = slice.split_at_mut(second);
        (&mut left[first], &mut right[0])
    } else {
        let (left, right) = slice.split_at_mut(first);
        (&mut right[0], &mut left[second])
    }
}
