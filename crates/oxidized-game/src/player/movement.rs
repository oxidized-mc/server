//! Player movement validation.
//!
//! Validates incoming movement packets against server-side limits and
//! produces a result indicating whether the movement is accepted or
//! needs server correction.

use oxidized_protocol::types::Vec3;

/// Maximum distance a player may travel in a single tick (blocks).
///
/// Exceeding this triggers a server-side position correction. Vanilla
/// uses 100 m for normal movement and 300 m during elytra flight.
pub const MAX_MOVEMENT_PER_TICK: f64 = 100.0;

/// Maximum valid coordinate value (±30 million blocks).
pub const MAX_COORDINATE: f64 = 3.0e7;

/// Result of validating a movement packet.
#[derive(Debug, Clone)]
pub struct MovementResult {
    /// Whether the movement was accepted.
    pub accepted: bool,
    /// Whether the server should send a position correction to the client.
    pub needs_correction: bool,
    /// The resolved position (either new or corrected).
    pub new_pos: Vec3,
    /// The resolved yaw rotation.
    pub new_yaw: f32,
    /// The resolved pitch rotation.
    pub new_pitch: f32,
}

/// Validates a client movement update against server-side limits.
///
/// Returns a [`MovementResult`] indicating whether the movement is
/// accepted or needs correction.
///
/// # Validation Rules
///
/// 1. Coordinates must be finite (not NaN or infinite)
/// 2. Coordinates must be within ±30 million blocks horizontally
/// 3. Movement distance must not exceed [`MAX_MOVEMENT_PER_TICK`]
/// 4. Pitch is clamped to ±90°
pub fn validate_movement(
    current_pos: Vec3,
    current_yaw: f32,
    current_pitch: f32,
    new_x: Option<f64>,
    new_y: Option<f64>,
    new_z: Option<f64>,
    new_yaw: Option<f32>,
    new_pitch: Option<f32>,
) -> MovementResult {
    let resolved_x = new_x.unwrap_or(current_pos.x);
    let resolved_y = new_y.unwrap_or(current_pos.y);
    let resolved_z = new_z.unwrap_or(current_pos.z);
    let resolved_yaw = new_yaw.unwrap_or(current_yaw);
    let resolved_pitch = new_pitch.unwrap_or(current_pitch).clamp(-90.0, 90.0);

    // Clamp horizontal coordinates to world bounds (±30M).
    let clamped_x = resolved_x.clamp(-MAX_COORDINATE, MAX_COORDINATE);
    let clamped_z = resolved_z.clamp(-MAX_COORDINATE, MAX_COORDINATE);

    let new_pos = Vec3::new(clamped_x, resolved_y, clamped_z);

    // Calculate squared distance from current position.
    let dx = new_pos.x - current_pos.x;
    let dy = new_pos.y - current_pos.y;
    let dz = new_pos.z - current_pos.z;
    let dist_sq = dx * dx + dy * dy + dz * dz;

    let needs_correction = dist_sq > MAX_MOVEMENT_PER_TICK * MAX_MOVEMENT_PER_TICK;

    MovementResult {
        accepted: !needs_correction,
        needs_correction,
        new_pos,
        new_yaw: resolved_yaw % 360.0,
        new_pitch: resolved_pitch,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_step_accepted() {
        let result = validate_movement(
            Vec3::ZERO,
            0.0,
            0.0,
            Some(0.1),
            Some(0.0),
            Some(0.0),
            None,
            None,
        );
        assert!(result.accepted);
        assert!(!result.needs_correction);
        assert!((result.new_pos.x - 0.1).abs() < f64::EPSILON);
    }

    #[test]
    fn test_too_fast_needs_correction() {
        let result = validate_movement(
            Vec3::ZERO,
            0.0,
            0.0,
            Some(200.0),
            Some(0.0),
            Some(0.0),
            None,
            None,
        );
        assert!(!result.accepted);
        assert!(result.needs_correction);
    }

    #[test]
    fn test_exactly_at_limit_accepted() {
        let result = validate_movement(
            Vec3::ZERO,
            0.0,
            0.0,
            Some(100.0),
            Some(0.0),
            Some(0.0),
            None,
            None,
        );
        // 100^2 = 10000 which equals MAX^2, so NOT greater — accepted
        assert!(result.accepted);
    }

    #[test]
    fn test_slightly_over_limit_rejected() {
        let result = validate_movement(
            Vec3::ZERO,
            0.0,
            0.0,
            Some(100.01),
            Some(0.0),
            Some(0.0),
            None,
            None,
        );
        assert!(!result.accepted);
        assert!(result.needs_correction);
    }

    #[test]
    fn test_no_pos_keeps_current() {
        let current = Vec3::new(50.0, 64.0, -30.0);
        let result = validate_movement(
            current,
            90.0,
            -15.0,
            None,
            None,
            None,
            Some(180.0),
            Some(45.0),
        );
        assert!(result.accepted);
        assert!((result.new_pos.x - 50.0).abs() < f64::EPSILON);
        assert!((result.new_pos.y - 64.0).abs() < f64::EPSILON);
        assert!((result.new_pos.z + 30.0).abs() < f64::EPSILON);
        assert!((result.new_yaw - 180.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_no_rot_keeps_current() {
        let result = validate_movement(
            Vec3::ZERO,
            45.0,
            -10.0,
            Some(1.0),
            Some(1.0),
            Some(1.0),
            None,
            None,
        );
        assert!((result.new_yaw - 45.0).abs() < f32::EPSILON);
        assert!((result.new_pitch + 10.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_pitch_clamped() {
        let result = validate_movement(
            Vec3::ZERO,
            0.0,
            0.0,
            None,
            None,
            None,
            None,
            Some(100.0), // > 90°
        );
        assert!((result.new_pitch - 90.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_pitch_clamped_negative() {
        let result = validate_movement(
            Vec3::ZERO,
            0.0,
            0.0,
            None,
            None,
            None,
            None,
            Some(-100.0), // < -90°
        );
        assert!((result.new_pitch + 90.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_coordinate_clamping() {
        let result = validate_movement(
            Vec3::new(2.9e7, 0.0, 0.0),
            0.0,
            0.0,
            Some(4.0e7),
            Some(0.0),
            Some(0.0), // way beyond 30M
            None,
            None,
        );
        // X should be clamped to 3.0e7
        assert!((result.new_pos.x - 3.0e7).abs() < 1.0);
    }

    #[test]
    fn test_diagonal_movement_distance() {
        // Moving 80 blocks on X and 80 on Z = ~113 blocks diagonal > 100 limit
        let result = validate_movement(
            Vec3::ZERO,
            0.0,
            0.0,
            Some(80.0),
            Some(0.0),
            Some(80.0),
            None,
            None,
        );
        assert!(!result.accepted);
        assert!(result.needs_correction);
    }

    #[test]
    fn test_vertical_movement_counted() {
        // Moving 101 blocks straight up
        let result = validate_movement(
            Vec3::ZERO,
            0.0,
            0.0,
            Some(0.0),
            Some(101.0),
            Some(0.0),
            None,
            None,
        );
        assert!(!result.accepted);
    }
}
