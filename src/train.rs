use bevy::prelude::*;

#[derive(Component)]
pub struct Train {
    pub transforms: Vec<Transform>,
    pub idx: usize,
    pub next_idx: usize,
    pub forward: bool,
    pub seconds_spent_within_segment: f32,
    pub segment_duration: Option<f32>,
}

#[derive(Component)]
pub struct SelectedTrain;

impl Train {
    pub fn new(transforms: Vec<Transform>) -> Option<Self> {
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

    fn compute_segment_duration(&mut self) {
        let start = self.transforms[self.idx].translation;
        let end = self.transforms[self.next_idx].translation;
        let distance = start.distance(end);
        let start_height = start.length();
        let end_height = end.length();
        let height_difference = end_height - start_height;
        let steepness = height_difference / distance;
        let half_speed_steepness = 0.2; // Steepness at which the speed is halved
        let mut velocity = 0.1; // Units per second
        if height_difference > 0.0 {
            velocity = velocity * half_speed_steepness / (steepness + half_speed_steepness);
        }
        self.segment_duration = Some(distance / velocity);
    }

    pub fn current_transform(&mut self) -> Transform {
        if self.segment_duration.is_none() {
            self.compute_segment_duration();
        }
        let along_segment_ratio =
            self.seconds_spent_within_segment / self.segment_duration.unwrap();
        Transform {
            translation: self.transforms[self.idx].translation.lerp(
                self.transforms[self.next_idx].translation,
                along_segment_ratio,
            ),
            rotation: self.transforms[self.idx]
                .rotation
                .slerp(self.transforms[self.next_idx].rotation, along_segment_ratio),
            scale: self.transforms[self.idx]
                .scale
                .lerp(self.transforms[self.next_idx].scale, along_segment_ratio),
        }
    }

    pub fn update(&mut self, transform: &mut Transform, time_passed_seconds: f32) {
        if self.segment_duration.is_none() {
            self.compute_segment_duration();
        }

        self.seconds_spent_within_segment += time_passed_seconds;
        if self.seconds_spent_within_segment >= self.segment_duration.unwrap() {
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
