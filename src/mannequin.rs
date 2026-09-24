use core::fmt;

use three_d_animation::retarget::{HumanoidBinding, HumanoidBone, HumanoidRig};
use three_d_animation::{
    AnimationClip, AnimationTrack, Interpolation, Keyframe, KeyframeTrack, LoopMode, Mat4, Quat,
    Transform, TransformNode, world_matrices,
};
use three_d_core::Vec3 as AnimationVec3;

use crate::{CapsuleCollider, ClothCollider, SphereCollider, Vec3};

const COLLISION_THICKNESS: f64 = 0.025;

const HIPS: usize = 0;
const SPINE: usize = 1;
const CHEST: usize = 2;
const NECK: usize = 3;
const HEAD: usize = 4;
const LEFT_UPPER_ARM: usize = 5;
const LEFT_LOWER_ARM: usize = 6;
const RIGHT_UPPER_ARM: usize = 7;
const RIGHT_LOWER_ARM: usize = 8;
const LEFT_UPPER_LEG: usize = 9;
const LEFT_LOWER_LEG: usize = 10;
const RIGHT_UPPER_LEG: usize = 11;
const RIGHT_LOWER_LEG: usize = 12;
const JOINT_COUNT: usize = 13;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MannequinAnimation {
    Rest,
    Walk,
    Wave,
}

impl MannequinAnimation {
    pub const ALL: [Self; 3] = [Self::Rest, Self::Walk, Self::Wave];

    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Rest => "rest",
            Self::Walk => "walk",
            Self::Wave => "wave",
        }
    }

    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Rest => "Rest",
            Self::Walk => "Walk",
            Self::Wave => "Wave",
        }
    }

    #[must_use]
    pub fn from_key(value: &str) -> Option<Self> {
        match value {
            "rest" => Some(Self::Rest),
            "walk" => Some(Self::Walk),
            "wave" => Some(Self::Wave),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MannequinAttachment {
    pub particle_index: usize,
    pub bone: HumanoidBone,
    pub local_offset: [f32; 3],
}

impl MannequinAttachment {
    pub const fn new(
        particle_index: usize,
        bone: HumanoidBone,
        local_offset: [f32; 3],
    ) -> Self {
        Self {
            particle_index,
            bone,
            local_offset,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MannequinAnimationError {
    InvalidSpeed,
    InvalidDeltaSeconds,
}

impl fmt::Display for MannequinAnimationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSpeed => {
                formatter.write_str("mannequin animation speed must be finite and non-negative")
            }
            Self::InvalidDeltaSeconds => formatter
                .write_str("mannequin animation timestep must be finite and non-negative"),
        }
    }
}

impl std::error::Error for MannequinAnimationError {}

pub(crate) struct MannequinAnimator {
    rig: HumanoidRig,
    nodes: Vec<TransformNode>,
    pose: Vec<Transform>,
    world: Vec<Mat4>,
    walk: AnimationClip,
    wave: AnimationClip,
    animation: MannequinAnimation,
    time_seconds: f32,
    speed: f32,
    colliders: Vec<ClothCollider>,
}

impl MannequinAnimator {
    #[must_use]
    pub fn new() -> Self {
        let rest_pose = rest_pose();
        let rig = HumanoidRig::new(rest_pose.clone(), &humanoid_bindings(), 3.0)
            .expect("built-in mannequin humanoid mapping must be valid");
        let nodes = hierarchy(&rest_pose);
        let world = world_matrices(&nodes).expect("built-in mannequin hierarchy must be valid");
        let mut animator = Self {
            rig,
            nodes,
            pose: rest_pose,
            world,
            walk: walk_clip(),
            wave: wave_clip(),
            animation: MannequinAnimation::Rest,
            time_seconds: 0.0,
            speed: 1.0,
            colliders: Vec::with_capacity(12),
        };
        animator.refresh_colliders();
        animator
    }

    #[must_use]
    pub const fn animation(&self) -> MannequinAnimation {
        self.animation
    }

    #[must_use]
    pub const fn time_seconds(&self) -> f32 {
        self.time_seconds
    }

    #[must_use]
    pub const fn speed(&self) -> f32 {
        self.speed
    }

    #[must_use]
    pub fn joint_count(&self) -> usize {
        self.pose.len()
    }

    #[must_use]
    pub fn colliders(&self) -> &[ClothCollider] {
        &self.colliders
    }

    pub fn set_animation(&mut self, animation: MannequinAnimation) {
        if self.animation == animation {
            return;
        }
        self.animation = animation;
        self.time_seconds = 0.0;
        self.sample_current_pose();
    }

    pub fn set_speed(&mut self, speed: f32) -> Result<(), MannequinAnimationError> {
        if !speed.is_finite() || speed < 0.0 {
            return Err(MannequinAnimationError::InvalidSpeed);
        }
        self.speed = speed;
        Ok(())
    }

    pub fn advance(&mut self, delta_seconds: f64) -> Result<(), MannequinAnimationError> {
        if !delta_seconds.is_finite() || delta_seconds < 0.0 {
            return Err(MannequinAnimationError::InvalidDeltaSeconds);
        }
        self.time_seconds += delta_seconds as f32 * self.speed;
        self.sample_current_pose();
        Ok(())
    }

    pub fn reset(&mut self) {
        self.time_seconds = 0.0;
        self.sample_current_pose();
    }

    #[must_use]
    pub fn attachment_target(&self, attachment: MannequinAttachment) -> Vec3 {
        let node = self
            .rig
            .node(attachment.bone)
            .expect("built-in attachment bone must be mapped");
        let [x, y, z] = attachment.local_offset;
        to_cloth_vec3(self.world[node].transform_point(AnimationVec3::new(x, y, z)))
    }

    fn sample_current_pose(&mut self) {
        self.pose.copy_from_slice(self.rig.rest_pose());
        let clip = match self.animation {
            MannequinAnimation::Rest => None,
            MannequinAnimation::Walk => Some(&self.walk),
            MannequinAnimation::Wave => Some(&self.wave),
        };
        if let Some(clip) = clip {
            clip.sample(self.time_seconds, &mut self.pose)
                .expect("built-in clip targets must fit the mannequin pose");
        }
        for (node, pose) in self.nodes.iter_mut().zip(&self.pose) {
            node.local = *pose;
        }
        self.world = world_matrices(&self.nodes).expect("built-in mannequin hierarchy must stay valid");
        self.refresh_colliders();
    }

    fn refresh_colliders(&mut self) {
        self.colliders.clear();

        let head_center = self.point(HEAD, [0.0, 0.06, 0.0]);
        self.colliders.push(ClothCollider::Sphere(SphereCollider {
            center: head_center,
            radius: 0.26,
            thickness: COLLISION_THICKNESS,
        }));

        self.push_capsule(
            self.point(LEFT_UPPER_ARM, [0.0, 0.0, 0.0]),
            self.point(RIGHT_UPPER_ARM, [0.0, 0.0, 0.0]),
            0.20,
        );
        self.push_capsule(
            self.point(HIPS, [0.0, 0.12, 0.0]),
            self.point(CHEST, [0.0, 0.10, 0.0]),
            0.38,
        );

        self.push_capsule(
            self.point(LEFT_UPPER_ARM, [0.0, 0.0, 0.0]),
            self.point(LEFT_LOWER_ARM, [0.0, 0.0, 0.0]),
            0.16,
        );
        self.push_capsule(
            self.point(LEFT_LOWER_ARM, [0.0, 0.0, 0.0]),
            self.point(LEFT_LOWER_ARM, [-0.38, -0.24, 0.0]),
            0.13,
        );
        self.push_capsule(
            self.point(RIGHT_UPPER_ARM, [0.0, 0.0, 0.0]),
            self.point(RIGHT_LOWER_ARM, [0.0, 0.0, 0.0]),
            0.16,
        );
        self.push_capsule(
            self.point(RIGHT_LOWER_ARM, [0.0, 0.0, 0.0]),
            self.point(RIGHT_LOWER_ARM, [0.38, -0.24, 0.0]),
            0.13,
        );

        self.push_capsule(
            self.point(HIPS, [-0.38, 0.0, 0.0]),
            self.point(HIPS, [0.38, 0.0, 0.0]),
            0.24,
        );

        self.push_capsule(
            self.point(LEFT_UPPER_LEG, [0.0, 0.0, 0.0]),
            self.point(LEFT_LOWER_LEG, [0.0, 0.0, 0.0]),
            0.17,
        );
        self.push_capsule(
            self.point(LEFT_LOWER_LEG, [0.0, 0.0, 0.0]),
            self.point(LEFT_LOWER_LEG, [0.0, -0.68, 0.0]),
            0.15,
        );
        self.push_capsule(
            self.point(RIGHT_UPPER_LEG, [0.0, 0.0, 0.0]),
            self.point(RIGHT_LOWER_LEG, [0.0, 0.0, 0.0]),
            0.17,
        );
        self.push_capsule(
            self.point(RIGHT_LOWER_LEG, [0.0, 0.0, 0.0]),
            self.point(RIGHT_LOWER_LEG, [0.0, -0.68, 0.0]),
            0.15,
        );
    }

    fn point(&self, node: usize, local_offset: [f32; 3]) -> Vec3 {
        let [x, y, z] = local_offset;
        to_cloth_vec3(
            self.world[node].transform_point(AnimationVec3::new(x, y, z)),
        )
    }

    fn push_capsule(&mut self, start: Vec3, end: Vec3, radius: f64) {
        self.colliders.push(ClothCollider::Capsule(CapsuleCollider {
            start,
            end,
            radius,
            thickness: COLLISION_THICKNESS,
        }));
    }
}

fn rest_pose() -> Vec<Transform> {
    vec![
        transform(0.0, -0.18, 0.0),
        transform(0.0, 0.55, 0.0),
        transform(0.0, 0.55, 0.0),
        transform(0.0, 0.38, 0.0),
        transform(0.0, 0.28, 0.0),
        transform(-0.55, 0.34, 0.0),
        transform(-0.45, -0.28, 0.0),
        transform(0.55, 0.34, 0.0),
        transform(0.45, -0.28, 0.0),
        transform(-0.22, -0.10, 0.0),
        transform(0.0, -0.75, 0.0),
        transform(0.22, -0.10, 0.0),
        transform(0.0, -0.75, 0.0),
    ]
}

fn hierarchy(rest_pose: &[Transform]) -> Vec<TransformNode> {
    const PARENTS: [Option<usize>; JOINT_COUNT] = [
        None,
        Some(HIPS),
        Some(SPINE),
        Some(CHEST),
        Some(NECK),
        Some(CHEST),
        Some(LEFT_UPPER_ARM),
        Some(CHEST),
        Some(RIGHT_UPPER_ARM),
        Some(HIPS),
        Some(LEFT_UPPER_LEG),
        Some(HIPS),
        Some(RIGHT_UPPER_LEG),
    ];
    rest_pose
        .iter()
        .copied()
        .zip(PARENTS)
        .map(|(local, parent)| TransformNode { parent, local })
        .collect()
}

fn humanoid_bindings() -> [HumanoidBinding; JOINT_COUNT] {
    [
        binding(HumanoidBone::Hips, HIPS),
        binding(HumanoidBone::Spine, SPINE),
        binding(HumanoidBone::Chest, CHEST),
        binding(HumanoidBone::Neck, NECK),
        binding(HumanoidBone::Head, HEAD),
        binding(HumanoidBone::LeftUpperArm, LEFT_UPPER_ARM),
        binding(HumanoidBone::LeftLowerArm, LEFT_LOWER_ARM),
        binding(HumanoidBone::RightUpperArm, RIGHT_UPPER_ARM),
        binding(HumanoidBone::RightLowerArm, RIGHT_LOWER_ARM),
        binding(HumanoidBone::LeftUpperLeg, LEFT_UPPER_LEG),
        binding(HumanoidBone::LeftLowerLeg, LEFT_LOWER_LEG),
        binding(HumanoidBone::RightUpperLeg, RIGHT_UPPER_LEG),
        binding(HumanoidBone::RightLowerLeg, RIGHT_LOWER_LEG),
    ]
}

const fn binding(bone: HumanoidBone, node: usize) -> HumanoidBinding {
    HumanoidBinding { bone, node }
}

const fn transform(x: f32, y: f32, z: f32) -> Transform {
    Transform {
        translation: AnimationVec3::new(x, y, z),
        rotation: Quat::IDENTITY,
        scale: AnimationVec3::new(1.0, 1.0, 1.0),
    }
}

fn walk_clip() -> AnimationClip {
    AnimationClip::new(
        "walk",
        vec![
            rotation_track(
                LEFT_UPPER_ARM,
                [(0.0, 0.34), (0.5, -0.34), (1.0, 0.34)],
                Axis::X,
            ),
            rotation_track(
                RIGHT_UPPER_ARM,
                [(0.0, -0.34), (0.5, 0.34), (1.0, -0.34)],
                Axis::X,
            ),
            rotation_track(
                LEFT_UPPER_LEG,
                [(0.0, -0.38), (0.5, 0.38), (1.0, -0.38)],
                Axis::X,
            ),
            rotation_track(
                RIGHT_UPPER_LEG,
                [(0.0, 0.38), (0.5, -0.38), (1.0, 0.38)],
                Axis::X,
            ),
            rotation_track(
                LEFT_LOWER_LEG,
                [(0.0, 0.12), (0.5, 0.42), (1.0, 0.12)],
                Axis::X,
            ),
            rotation_track(
                RIGHT_LOWER_LEG,
                [(0.0, 0.42), (0.5, 0.12), (1.0, 0.42)],
                Axis::X,
            ),
            AnimationTrack::Translation {
                node: HIPS,
                track: KeyframeTrack::new(
                    vec![
                        Keyframe {
                            time: 0.0,
                            value: AnimationVec3::new(0.0, -0.18, 0.0),
                        },
                        Keyframe {
                            time: 0.25,
                            value: AnimationVec3::new(0.0, -0.15, 0.0),
                        },
                        Keyframe {
                            time: 0.5,
                            value: AnimationVec3::new(0.0, -0.18, 0.0),
                        },
                        Keyframe {
                            time: 0.75,
                            value: AnimationVec3::new(0.0, -0.15, 0.0),
                        },
                        Keyframe {
                            time: 1.0,
                            value: AnimationVec3::new(0.0, -0.18, 0.0),
                        },
                    ],
                    Interpolation::SmoothStep,
                )
                .expect("built-in walk translation track must be valid"),
            },
        ],
    )
    .expect("built-in walk clip must be valid")
    .with_loop_mode(LoopMode::Repeat)
}

fn wave_clip() -> AnimationClip {
    AnimationClip::new(
        "wave",
        vec![
            rotation_track(
                LEFT_UPPER_ARM,
                [(0.0, 0.92), (0.5, 1.04), (1.0, 0.92)],
                Axis::Z,
            ),
            rotation_track(
                LEFT_LOWER_ARM,
                [(0.0, -0.18), (0.25, 0.42), (0.5, -0.18), (0.75, 0.42), (1.0, -0.18)],
                Axis::Z,
            ),
            rotation_track(
                CHEST,
                [(0.0, -0.05), (0.5, 0.05), (1.0, -0.05)],
                Axis::Z,
            ),
        ],
    )
    .expect("built-in wave clip must be valid")
    .with_loop_mode(LoopMode::Repeat)
}

#[derive(Clone, Copy)]
enum Axis {
    X,
    Z,
}

fn rotation_track<const N: usize>(
    node: usize,
    samples: [(f32, f32); N],
    axis: Axis,
) -> AnimationTrack {
    let frames = samples
        .into_iter()
        .map(|(time, angle)| Keyframe {
            time,
            value: match axis {
                Axis::X => Quat::from_euler_xyz(angle, 0.0, 0.0),
                Axis::Z => Quat::from_euler_xyz(0.0, 0.0, angle),
            },
        })
        .collect();
    AnimationTrack::Rotation {
        node,
        track: KeyframeTrack::new(frames, Interpolation::SmoothStep)
            .expect("built-in rotation track must be valid"),
    }
}

fn to_cloth_vec3(value: AnimationVec3) -> Vec3 {
    Vec3::new(f64::from(value.x), f64::from(value.y), f64::from(value.z))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collider_signature(colliders: &[ClothCollider]) -> Vec<f64> {
        let mut values = Vec::new();
        for collider in colliders {
            match collider {
                ClothCollider::Sphere(sphere) => {
                    values.extend_from_slice(&[
                        sphere.center.x,
                        sphere.center.y,
                        sphere.center.z,
                        sphere.radius,
                    ]);
                }
                ClothCollider::Capsule(capsule) => {
                    values.extend_from_slice(&[
                        capsule.start.x,
                        capsule.start.y,
                        capsule.start.z,
                        capsule.end.x,
                        capsule.end.y,
                        capsule.end.z,
                        capsule.radius,
                    ]);
                }
            }
        }
        values
    }

    #[test]
    fn rest_pose_builds_finite_full_body_colliders() {
        let animator = MannequinAnimator::new();

        assert_eq!(animator.joint_count(), JOINT_COUNT);
        assert_eq!(animator.colliders().len(), 12);
        assert!(
            collider_signature(animator.colliders())
                .into_iter()
                .all(f64::is_finite)
        );
    }

    #[test]
    fn walk_replay_is_deterministic_and_moves_the_collision_rig() {
        let mut first = MannequinAnimator::new();
        let rest = collider_signature(first.colliders());
        first.set_animation(MannequinAnimation::Walk);
        for _ in 0..30 {
            first.advance(1.0 / 60.0).unwrap();
        }
        let first_result = collider_signature(first.colliders());

        let mut second = MannequinAnimator::new();
        second.set_animation(MannequinAnimation::Walk);
        for _ in 0..30 {
            second.advance(1.0 / 60.0).unwrap();
        }

        assert_ne!(first_result, rest);
        assert_eq!(first_result, collider_signature(second.colliders()));
        assert_eq!(first.time_seconds(), second.time_seconds());
    }

    #[test]
    fn reset_restores_the_first_sample_of_the_active_clip() {
        let mut animator = MannequinAnimator::new();
        animator.set_animation(MannequinAnimation::Wave);
        let initial = collider_signature(animator.colliders());
        animator.advance(0.37).unwrap();
        assert_ne!(collider_signature(animator.colliders()), initial);

        animator.reset();

        assert_eq!(animator.time_seconds(), 0.0);
        assert_eq!(collider_signature(animator.colliders()), initial);
    }

    #[test]
    fn animated_bone_attachment_tracks_the_sampled_pose() {
        let mut animator = MannequinAnimator::new();
        let attachment = MannequinAttachment::new(
            7,
            HumanoidBone::LeftLowerArm,
            [-0.36, -0.22, 0.0],
        );
        animator.set_animation(MannequinAnimation::Wave);
        let initial = animator.attachment_target(attachment);
        animator.advance(0.25).unwrap();

        assert_ne!(animator.attachment_target(attachment), initial);
    }

    #[test]
    fn animation_speed_fails_closed() {
        let mut animator = MannequinAnimator::new();

        assert_eq!(
            animator.set_speed(f32::NAN),
            Err(MannequinAnimationError::InvalidSpeed)
        );
        assert_eq!(
            animator.advance(f64::INFINITY),
            Err(MannequinAnimationError::InvalidDeltaSeconds)
        );
    }
}
