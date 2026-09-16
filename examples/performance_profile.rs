use std::hint::black_box;
use std::time::{Duration, Instant};

use cloth_lab::{
    ClothCollider, ContactConfig, FixedStepConfig, SelfCollisionConfig, SphereCollider,
    TriangleMeshCloth, TriangleMeshClothConfig, Vec3,
};

const SAMPLE_COUNT: usize = 9;
const STEPS_PER_SAMPLE: usize = 30;
const SMOKE_SAMPLE_COUNT: usize = 2;
const SMOKE_STEPS_PER_SAMPLE: usize = 2;
const REFERENCE_COLUMNS: usize = 28;
const REFERENCE_ROWS: usize = 20;
const LAYER_COLUMNS: usize = 18;
const LAYER_ROWS: usize = 14;
const SPACING: f64 = 0.055;
const SELF_COLLISION_THICKNESS: f64 = 0.04;

#[derive(Clone, Copy)]
enum BenchmarkMode {
    ReferenceOneIteration,
    ReferenceEightIterations,
    RigidCollision,
    LayeredWithoutSelfCollision,
    LayeredWithSelfCollision,
}

#[derive(Clone, Copy)]
struct ProfileConfig {
    sample_count: usize,
    steps_per_sample: usize,
}

struct SampleResult {
    elapsed: Duration,
    fingerprint: u64,
}

fn main() {
    let config = profile_config();
    println!("cloth-lab deterministic reference performance profile");
    println!(
        "samples={} steps/sample={}",
        config.sample_count, config.steps_per_sample
    );
    println!();
    println!(
        "| lane | vertices | triangles | solver iterations | median ns/step | min ns/step | max ns/step | fingerprint |"
    );
    println!("| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |");

    profile_lane(
        "reference-1-iteration",
        BenchmarkMode::ReferenceOneIteration,
        config,
    );
    profile_lane(
        "reference-8-iterations",
        BenchmarkMode::ReferenceEightIterations,
        config,
    );
    profile_lane("rigid-collision", BenchmarkMode::RigidCollision, config);
    profile_lane(
        "layered-no-self-collision",
        BenchmarkMode::LayeredWithoutSelfCollision,
        config,
    );
    profile_lane(
        "layered-self-collision",
        BenchmarkMode::LayeredWithSelfCollision,
        config,
    );
}

fn profile_config() -> ProfileConfig {
    let smoke = std::env::args().skip(1).any(|argument| argument == "--smoke");
    if smoke {
        ProfileConfig {
            sample_count: SMOKE_SAMPLE_COUNT,
            steps_per_sample: SMOKE_STEPS_PER_SAMPLE,
        }
    } else {
        ProfileConfig {
            sample_count: SAMPLE_COUNT,
            steps_per_sample: STEPS_PER_SAMPLE,
        }
    }
}

fn profile_lane(name: &str, mode: BenchmarkMode, config: ProfileConfig) {
    let warmup = run_sample(mode, config.steps_per_sample);
    black_box(warmup.fingerprint);

    let mut samples = (0..config.sample_count)
        .map(|_| run_sample(mode, config.steps_per_sample))
        .collect::<Vec<_>>();
    let expected_fingerprint = samples[0].fingerprint;
    assert!(
        samples
            .iter()
            .all(|sample| sample.fingerprint == expected_fingerprint),
        "benchmark lane {name} did not replay deterministically"
    );

    samples.sort_by_key(|sample| sample.elapsed);
    let median = samples[config.sample_count / 2].elapsed;
    let minimum = samples[0].elapsed;
    let maximum = samples[config.sample_count - 1].elapsed;
    let cloth = fixture_for(mode);
    let solver_iterations = step_config_for(mode).solver_iterations;

    println!(
        "| {name} | {} | {} | {solver_iterations} | {} | {} | {} | {expected_fingerprint:016x} |",
        cloth.particles().len(),
        cloth.triangles().len(),
        nanoseconds_per_step(median, config.steps_per_sample),
        nanoseconds_per_step(minimum, config.steps_per_sample),
        nanoseconds_per_step(maximum, config.steps_per_sample),
    );
}

fn run_sample(mode: BenchmarkMode, steps_per_sample: usize) -> SampleResult {
    let mut cloth = fixture_for(mode);
    let step = step_config_for(mode);
    let sphere = ClothCollider::Sphere(SphereCollider {
        center: Vec3::new(0.0, -0.04, 0.0),
        radius: 0.32,
        thickness: 0.02,
    });
    let colliders = [sphere];
    let self_collision = SelfCollisionConfig {
        thickness: SELF_COLLISION_THICKNESS,
    };

    let started = Instant::now();
    let mut fingerprint = cloth.state_fingerprint();
    for _ in 0..steps_per_sample {
        fingerprint = match mode {
            BenchmarkMode::ReferenceOneIteration
            | BenchmarkMode::ReferenceEightIterations
            | BenchmarkMode::LayeredWithoutSelfCollision => cloth
                .step(step)
                .expect("reference benchmark step must succeed")
                .state_fingerprint,
            BenchmarkMode::RigidCollision => cloth
                .step_with_contacts(step, &colliders, ContactConfig::default())
                .expect("rigid-collision benchmark step must succeed")
                .state_fingerprint,
            BenchmarkMode::LayeredWithSelfCollision => cloth
                .step_with_contacts_and_self_collision(
                    step,
                    &[],
                    ContactConfig::default(),
                    self_collision,
                )
                .expect("self-collision benchmark step must succeed")
                .solver
                .state_fingerprint,
        };
        black_box(fingerprint);
    }
    let elapsed = started.elapsed();

    SampleResult {
        elapsed,
        fingerprint,
    }
}

fn fixture_for(mode: BenchmarkMode) -> TriangleMeshCloth {
    match mode {
        BenchmarkMode::ReferenceOneIteration
        | BenchmarkMode::ReferenceEightIterations
        | BenchmarkMode::RigidCollision => reference_sheet(),
        BenchmarkMode::LayeredWithoutSelfCollision | BenchmarkMode::LayeredWithSelfCollision => {
            layered_sheet()
        }
    }
}

fn step_config_for(mode: BenchmarkMode) -> FixedStepConfig {
    FixedStepConfig {
        delta_seconds: 1.0 / 60.0,
        gravity: Vec3::new(0.0, -9.81, 0.0),
        solver_iterations: match mode {
            BenchmarkMode::ReferenceOneIteration => 1,
            _ => 8,
        },
        velocity_damping: 0.995,
    }
}

fn reference_sheet() -> TriangleMeshCloth {
    let (positions, triangles) = rectangular_grid(
        REFERENCE_COLUMNS,
        REFERENCE_ROWS,
        Vec3::new(
            -((REFERENCE_COLUMNS - 1) as f64 * SPACING) / 2.0,
            0.0,
            -((REFERENCE_ROWS - 1) as f64 * SPACING) / 2.0,
        ),
    );
    TriangleMeshCloth::new(&positions, &triangles, cloth_config())
        .expect("reference performance fixture must be valid")
}

fn layered_sheet() -> TriangleMeshCloth {
    let origin = Vec3::new(
        -((LAYER_COLUMNS - 1) as f64 * SPACING) / 2.0,
        0.0,
        -((LAYER_ROWS - 1) as f64 * SPACING) / 2.0,
    );
    let (mut positions, mut triangles) = rectangular_grid(LAYER_COLUMNS, LAYER_ROWS, origin);
    let first_layer_vertices = positions.len();
    let (upper_positions, upper_triangles) = rectangular_grid(
        LAYER_COLUMNS,
        LAYER_ROWS,
        origin + Vec3::new(0.0, SELF_COLLISION_THICKNESS * 0.45, 0.0),
    );
    positions.extend(upper_positions);
    triangles.extend(upper_triangles.into_iter().map(|triangle| {
        [
            triangle[0] + first_layer_vertices,
            triangle[1] + first_layer_vertices,
            triangle[2] + first_layer_vertices,
        ]
    }));

    TriangleMeshCloth::new(&positions, &triangles, cloth_config())
        .expect("layered performance fixture must be valid")
}

fn rectangular_grid(
    columns: usize,
    rows: usize,
    origin: Vec3,
) -> (Vec<Vec3>, Vec<[usize; 3]>) {
    let mut positions = Vec::with_capacity(columns * rows);
    for row in 0..rows {
        for column in 0..columns {
            positions.push(
                origin + Vec3::new(column as f64 * SPACING, 0.0, row as f64 * SPACING),
            );
        }
    }

    let mut triangles = Vec::with_capacity((columns - 1) * (rows - 1) * 2);
    for row in 0..rows - 1 {
        for column in 0..columns - 1 {
            let top_left = row * columns + column;
            let top_right = top_left + 1;
            let bottom_left = (row + 1) * columns + column;
            let bottom_right = bottom_left + 1;
            triangles.push([top_left, bottom_left, top_right]);
            triangles.push([top_right, bottom_left, bottom_right]);
        }
    }
    (positions, triangles)
}

fn cloth_config() -> TriangleMeshClothConfig {
    TriangleMeshClothConfig {
        particle_mass: 1.0,
        stretch_compliance: 1.0e-7,
        bending_compliance: 1.0e-3,
    }
}

fn nanoseconds_per_step(duration: Duration, steps_per_sample: usize) -> u128 {
    duration.as_nanos() / steps_per_sample as u128
}
