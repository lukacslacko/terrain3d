use crate::state::{Rail, State};
use bevy::prelude::*;
use std::sync::atomic::Ordering;

#[derive(Component)]
pub struct Train {
    pub transforms: Vec<(Transform, Rail)>,
    pub idx: usize,
    pub next_idx: usize,
    pub forward: bool,
    pub seconds_spent_within_segment: f32,
    pub segment_duration: Option<f32>,
}

#[derive(Component)]
pub struct SelectedTrain;

impl Train {
    pub fn new(transforms: Vec<(Transform, Rail)>) -> Option<Self> {
        if transforms.len() < 2 {
            return None;
        }
        Some(Self {
            transforms,
            idx: 0,
            next_idx: 1,
            forward: true,
            seconds_spent_within_segment: 0.0,
            segment_duration: None,
        })
    }

    fn transform_at(&self, idx: i32) -> Transform {
        self.transforms[idx.clamp(0, self.transforms.len() as i32 - 1) as usize].0
    }

    fn duration_between(&self, start: &Transform, end: &Transform) -> f32 {
        let distance = start.translation.distance(end.translation);
        let start_height = start.translation.length();
        let end_height = end.translation.length();
        let height_difference = end_height - start_height;
        let steepness = height_difference / distance;
        let half_speed_steepness = 0.2; // Steepness at which the speed is halved
        let mut velocity = 0.1; // Units per second
        if height_difference > 0.0 {
            velocity = velocity * half_speed_steepness / (steepness + half_speed_steepness);
        }
        distance / velocity
    }

    fn compute_segment_duration(&mut self) {
        self.segment_duration = Some(self.duration_between(
            &self.transform_at(self.idx as i32),
            &self.transform_at(self.next_idx as i32),
        ));
    }

    fn between_transforms(&self, from: &Transform, to: &Transform, ratio: f32) -> Transform {
        Transform {
            translation: from.translation.lerp(to.translation, ratio),
            rotation: from.rotation.slerp(to.rotation, ratio),
            scale: from.scale.lerp(to.scale, ratio),
        }
    }

    fn bezier_2(&self, a: &Transform, b: &Transform, c: &Transform, ratio: f32) -> Transform {
        let p_ab = self.between_transforms(a, b, ratio);
        let p_bc = self.between_transforms(b, c, ratio);
        self.between_transforms(&p_ab, &p_bc, ratio)
    }

    pub fn current_transform(&mut self) -> Transform {
        if self.segment_duration.is_none() {
            self.compute_segment_duration();
        }

        /*

        a - m_ab - b - m_bc - c - m_cd - d

        a, b, c, d are rail segment endpoints, m_ab, m_bc, m_cd are the midpoints.

        The spline connects the midpoints, with the segment endpoints being the control
        points.

        */

        let idx_diff = self.next_idx as i32 - self.idx as i32;
        let b_idx = self.idx as i32;
        let a_idx = b_idx - idx_diff;
        let c_idx = b_idx + idx_diff;
        let d_idx = c_idx + idx_diff;

        let a = self.transform_at(a_idx);
        let b = self.transform_at(b_idx);
        let c = self.transform_at(c_idx);
        let d = self.transform_at(d_idx);
        let m_ab = self.between_transforms(&a, &b, 0.5);
        let m_bc = self.between_transforms(&b, &c, 0.5);
        let m_cd = self.between_transforms(&c, &d, 0.5);

        let along_segment_ratio =
            self.seconds_spent_within_segment / self.segment_duration.unwrap();

        if along_segment_ratio <= 0.5 {
            self.bezier_2(&m_ab, &b, &m_bc, along_segment_ratio + 0.5)
        } else {
            self.bezier_2(&m_bc, &c, &m_cd, along_segment_ratio - 0.5)
        }
    }

    pub fn update(
        &mut self,
        transform: &mut Transform,
        time_passed_seconds: f32,
        state: &State,
        commands: &mut Commands,
        materials: &mut Assets<StandardMaterial>,
    ) {
        if self.segment_duration.is_none() {
            self.compute_segment_duration();
        }

        self.seconds_spent_within_segment += time_passed_seconds;
        if self.seconds_spent_within_segment >= self.segment_duration.unwrap() {
            let rail_info = state.rails.rails.get(&self.transforms[self.idx].1).unwrap();

            rail_info.counter.fetch_add(1, Ordering::Relaxed);

            let r = 0u8;
            let g = 0u8;
            let b = 0u8;

            let material = materials.add(StandardMaterial {
                base_color: Color::srgb_u8(r, g, b),
                perceptual_roughness: 0.0,
                metallic: 0.0,
                ..default()
            });

            commands
                .entity(rail_info.entity)
                .insert((MeshMaterial3d(material),));

            // Move to the next segment.
            self.idx = self.next_idx;

            // Account for the remaining time.
            self.seconds_spent_within_segment =
                self.segment_duration.unwrap() - self.seconds_spent_within_segment;

            if self.forward {
                if self.idx < self.transforms.len() - 1 {
                    self.next_idx += 1;
                } else {
                    // Reached the end, reverse direction
                    self.forward = false;
                    self.next_idx -= 1; // Set next_idx to the last valid index
                }
            } else {
                // backward
                if self.idx > 0 {
                    self.next_idx -= 1;
                } else {
                    // Reached the start, reverse direction
                    self.forward = true;
                    self.next_idx += 1; // Set next_idx to the first valid index
                }
            }
            self.compute_segment_duration();
        }

        let current_transform = self.current_transform();

        // Update the train's position
        transform.translation = current_transform.translation;
        transform.rotation = current_transform.rotation;
    }
}
