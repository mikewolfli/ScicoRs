//! Flight control: 6DOF rigid-body dynamics, quaternion kinematics,
//! autopilot (PID), and trim solution.

use super::aerodynamics::AircraftAerodynamics;
use crate::core::coord::Coord3D;
use crate::core::types::Scalar;

// ──────────────────────────────────────────────
// Quaternion Utilities
// ──────────────────────────────────────────────

/// Convert Euler angles (roll, pitch, yaw) in radians to a quaternion [w, x, y, z].
///
/// Uses the ZYX (yaw-pitch-roll) convention commonly used in aerospace:
/// q = q_yaw * q_pitch * q_roll
pub fn euler_to_quaternion(roll: Scalar, pitch: Scalar, yaw: Scalar) -> [Scalar; 4] {
    let cr = (roll * 0.5).cos();
    let sr = (roll * 0.5).sin();
    let cp = (pitch * 0.5).cos();
    let sp = (pitch * 0.5).sin();
    let cy = (yaw * 0.5).cos();
    let sy = (yaw * 0.5).sin();

    [
        cr * cp * cy + sr * sp * sy,
        sr * cp * cy - cr * sp * sy,
        cr * sp * cy + sr * cp * sy,
        cr * cp * sy - sr * sp * cy,
    ]
}

/// Convert a quaternion [w, x, y, z] to Euler angles (roll, pitch, yaw) in radians.
///
/// Uses the ZYX convention with pitch clamped to (-π/2, π/2) to avoid gimbal lock.
pub fn quaternion_to_euler(q: &[Scalar; 4]) -> (Scalar, Scalar, Scalar) {
    let [w, x, y, z] = *q;

    // Roll (bank angle)
    let sin_roll = 2.0 * (w * x + y * z);
    let cos_roll = 1.0 - 2.0 * (x * x + y * y);
    let roll = sin_roll.atan2(cos_roll);

    // Pitch (elevation angle)
    let sin_pitch = 2.0 * (w * y - z * x);
    let pitch = sin_pitch.clamp(-1.0, 1.0).asin();

    // Yaw (heading angle)
    let sin_yaw = 2.0 * (w * z + x * y);
    let cos_yaw = 1.0 - 2.0 * (y * y + z * z);
    let yaw = sin_yaw.atan2(cos_yaw);

    (roll, pitch, yaw)
}

/// Normalize a quaternion to unit length.
pub fn quaternion_normalize(q: &[Scalar; 4]) -> [Scalar; 4] {
    let norm = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if norm < 1e-30 {
        [1.0, 0.0, 0.0, 0.0]
    } else {
        [q[0] / norm, q[1] / norm, q[2] / norm, q[3] / norm]
    }
}

/// Multiply two quaternions: q_out = q_a * q_b.
pub fn quaternion_multiply(a: &[Scalar; 4], b: &[Scalar; 4]) -> [Scalar; 4] {
    [
        a[0] * b[0] - a[1] * b[1] - a[2] * b[2] - a[3] * b[3],
        a[0] * b[1] + a[1] * b[0] + a[2] * b[3] - a[3] * b[2],
        a[0] * b[2] - a[1] * b[3] + a[2] * b[0] + a[3] * b[1],
        a[0] * b[3] + a[1] * b[2] - a[2] * b[1] + a[3] * b[0],
    ]
}

/// Rotate a vector by a quaternion: v' = q * v * q_conj.
pub fn quaternion_rotate(q: &[Scalar; 4], v: &Coord3D) -> Coord3D {
    let qv = [0.0, v.x, v.y, v.z];
    let q_conj = [q[0], -q[1], -q[2], -q[3]];
    let temp = quaternion_multiply(q, &qv);
    let rotated = quaternion_multiply(&temp, &q_conj);
    Coord3D::new(rotated[1], rotated[2], rotated[3])
}

// ──────────────────────────────────────────────
// 6DOF Aircraft Model
// ──────────────────────────────────────────────

/// A 6-degree-of-freedom rigid-body aircraft model.
///
/// State variables:
/// - Position (x, y, z) in Earth frame
/// - Velocity (u, v, w) in body frame
/// - Attitude quaternion (w, x, y, z)
/// - Angular velocity (p, q, r) in body frame
pub struct SixDofAircraft {
    /// Aircraft mass (kg).
    pub mass: Scalar,
    /// Moment of inertia tensor (kg·m²), 3×3 symmetric.
    pub inertia: [[Scalar; 3]; 3],
    /// Position in Earth frame (m).
    pub position: Coord3D,
    /// Velocity in body frame (m/s): [u (forward), v (side), w (down)].
    pub velocity: [Scalar; 3],
    /// Attitude quaternion [w, x, y, z].
    pub attitude: [Scalar; 4],
    /// Angular velocity in body frame (rad/s): [p (roll), q (pitch), r (yaw)].
    pub angular_velocity: [Scalar; 3],
    /// Aerodynamic configuration.
    pub aerodynamics: AircraftAerodynamics,
    /// Wing reference area (m²).
    pub wing_area: Scalar,
    /// Mean aerodynamic chord (m).
    pub chord: Scalar,
    /// Wing span (m).
    pub span: Scalar,
}

impl SixDofAircraft {
    /// Compute aerodynamic forces and moments in body frame.
    ///
    /// Returns `(force_body [Fx, Fy, Fz], moment_body [L, M, N])`.
    pub fn aerodynamic_forces(&self, density: Scalar, speed: Scalar) -> ([Scalar; 3], [Scalar; 3]) {
        let q_inf = 0.5 * density * speed * speed; // dynamic pressure
        let alpha = if speed.abs() < 1e-10 {
            0.0
        } else {
            (self.velocity[2] / speed).asin()
        };
        let beta = if speed.abs() < 1e-10 {
            0.0
        } else {
            (self.velocity[1] / speed).asin()
        };

        let cl = self.aerodynamics.cl(alpha);
        let cd = self.aerodynamics.cd(alpha);

        // Lift force perpendicular to flight path
        let lift = q_inf * self.wing_area * cl;
        let drag = q_inf * self.wing_area * cd;

        // Side force (simplified: C_y = -0.1 * beta)
        let cy = -0.1 * beta;
        let side = q_inf * self.wing_area * cy;

        // Body-frame forces (simplified transformation from wind axes)
        let cos_a = alpha.cos();
        let sin_a = alpha.sin();
        let fx = -drag * cos_a + lift * sin_a;
        let fz = -drag * sin_a - lift * cos_a;
        let fy = side;

        // Moments (simplified stability derivatives)
        let [p, q, r] = self.angular_velocity;
        // Roll damping: C_lp = -0.3, Yaw damping: C_nr = -0.2, Pitch damping: C_mq = -5.0
        let cl_roll = -0.3 * (p * self.span) / (2.0 * speed.max(1.0));
        let cm_pitch = -5.0 * cl * self.chord / self.chord.max(1.0)
            - 5.0 * (q * self.chord) / (2.0 * speed.max(1.0));
        let cn_yaw = -0.2 * (r * self.span) / (2.0 * speed.max(1.0));

        let l_moment = q_inf * self.wing_area * self.span * cl_roll;
        let m_moment = q_inf * self.wing_area * self.chord * cm_pitch;
        let n_moment = q_inf * self.wing_area * self.span * cn_yaw;

        ([fx, fy, fz], [l_moment, m_moment, n_moment])
    }

    /// Compute state derivatives.
    ///
    /// Returns `(position_dot, velocity_dot, attitude_dot, angular_velocity_dot)`.
    /// Attitude derivative is a quaternion rate `[Scalar; 4]`.
    pub fn derivatives(
        &self,
        controls: &[Scalar; 4],
    ) -> ([Scalar; 3], [Scalar; 3], [Scalar; 4], [Scalar; 3]) {
        let [thrust, elevator, aileron, rudder] = *controls;

        // Speed magnitude
        let [u, v, w] = self.velocity;
        let speed = (u * u + v * v + w * w).sqrt();

        // Get atmospheric density at current altitude
        let density = super::physics::IsaAtmosphere::density(-self.position.z.max(0.0));
        let (forces, moments) = self.aerodynamic_forces(density, speed);

        // Add thrust (assumed along body x-axis)
        let fx_total = forces[0] + thrust;

        // Gravity in body frame
        let q = &self.attitude;
        let g_body = quaternion_rotate(q, &Coord3D::new(0.0, 0.0, 9.80665));
        let fz_total = forces[2] + self.mass * g_body.z;
        let fy_total = forces[1] + self.mass * g_body.y;

        // Position derivative (velocity in Earth frame)
        let earth_vel = quaternion_rotate(q, &Coord3D::new(u, v, w));
        let pos_dot = [earth_vel.x, earth_vel.y, earth_vel.z];

        // Velocity derivative (body frame)
        let p = self.angular_velocity[0];
        let q_av = self.angular_velocity[1];
        let r = self.angular_velocity[2];

        let u_dot = fx_total / self.mass - q_av * w + r * v;
        let v_dot = fy_total / self.mass - r * u + p * w;
        let w_dot = fz_total / self.mass - p * v + q_av * u;
        let vel_dot = [u_dot, v_dot, w_dot];

        // Attitude derivative (quaternion kinematics)
        let w_att = q[0];
        let x_att = q[1];
        let y_att = q[2];
        let z_att = q[3];
        let att_dot = [
            -0.5 * (x_att * p + y_att * q_av + z_att * r),
            0.5 * (w_att * p - z_att * q_av + y_att * r),
            0.5 * (z_att * p + w_att * q_av - x_att * r),
            -0.5 * (y_att * p - x_att * q_av - w_att * r),
        ];

        // Angular acceleration (Euler's equations)
        let ixx = self.inertia[0][0];
        let iyy = self.inertia[1][1];
        let izz = self.inertia[2][2];
        let ixz = self.inertia[0][2];

        // Control surface contributions (simplified)
        let d_l = aileron * 0.1 * density * speed * speed * self.wing_area * self.span;
        let d_m = elevator * 0.05 * density * speed * speed * self.wing_area * self.chord;
        let d_n = rudder * 0.08 * density * speed * speed * self.wing_area * self.span;

        let l_total = moments[0] + d_l;
        let m_total = moments[1] + d_m;
        let n_total = moments[2] + d_n;

        let den = ixx * izz - ixz * ixz;
        let p_dot = if den.abs() < 1e-30 {
            0.0
        } else {
            (l_total * izz
                + ixz
                    * (n_total - (iyy - izz - ixx) * p * q_av + ixz * p * q_av - izz * q_av * r
                        + ixz * q_av * r))
                / den
        };
        // Simplified: use denom check
        let _p_dot_safe = if den.abs() < 1e-30 {
            0.0
        } else {
            ((iyy - izz) * q_av * r + ixz * (p * r - p_dot)) / ixx + l_total / ixx
        };
        // Actually compute properly
        let p_dot_correct =
            (l_total - (izz - iyy) * q_av * r - ixz * (p * q_av + r_dot_approx(0.0))) / ixx;
        let q_dot = (m_total - (ixx - izz) * p * r - ixz * (p * p - r * r)) / iyy;
        let r_dot = (n_total - (iyy - ixx) * p * q_av - ixz * (q_av * r - p_dot_correct)) / izz;

        let av_dot = [p_dot_correct, q_dot, r_dot];

        (pos_dot, vel_dot, att_dot, av_dot)
    }

    /// Advance the state by one RK4 integration step.
    ///
    /// Uses the shared Butcher tableau coefficients from `runtime::solver::fixed_step`
    /// so that the RK4 weights are defined in a single location.
    pub fn rk4_step(&mut self, controls: &[Scalar; 4], dt: Scalar) {
        use crate::runtime::solver::fixed_step::{RK4_A, RK4_B};

        let dt_full = dt; // c3 = 1.0, full step for k4

        // Store initial state
        let p0 = self.position;
        let v0 = self.velocity;
        let a0 = self.attitude;
        let av0 = self.angular_velocity;

        // k1 (stage 0)
        let (k1_pos, k1_vel, k1_att, k1_av) = self.derivatives(controls);

        // k2 (stage 1): evaluate at t + c1*dt using k1
        let dt_k = dt * RK4_A[1][0];
        self.position = Coord3D::new(
            p0.x + k1_pos[0] * dt_k,
            p0.y + k1_pos[1] * dt_k,
            p0.z + k1_pos[2] * dt_k,
        );
        self.velocity = [
            v0[0] + k1_vel[0] * dt_k,
            v0[1] + k1_vel[1] * dt_k,
            v0[2] + k1_vel[2] * dt_k,
        ];
        self.attitude = quaternion_normalize(&[
            a0[0] + k1_att[0] * dt_k,
            a0[1] + k1_att[1] * dt_k,
            a0[2] + k1_att[2] * dt_k,
            a0[3] + k1_att[3] * dt_k,
        ]);
        self.angular_velocity = [
            av0[0] + k1_av[0] * dt_k,
            av0[1] + k1_av[1] * dt_k,
            av0[2] + k1_av[2] * dt_k,
        ];
        let (k2_pos, k2_vel, k2_att, k2_av) = self.derivatives(controls);

        // k3 (stage 2): evaluate at t + c2*dt using k2
        let dt_k = dt * RK4_A[2][1];
        self.position = Coord3D::new(
            p0.x + k2_pos[0] * dt_k,
            p0.y + k2_pos[1] * dt_k,
            p0.z + k2_pos[2] * dt_k,
        );
        self.velocity = [
            v0[0] + k2_vel[0] * dt_k,
            v0[1] + k2_vel[1] * dt_k,
            v0[2] + k2_vel[2] * dt_k,
        ];
        self.attitude = quaternion_normalize(&[
            a0[0] + k2_att[0] * dt_k,
            a0[1] + k2_att[1] * dt_k,
            a0[2] + k2_att[2] * dt_k,
            a0[3] + k2_att[3] * dt_k,
        ]);
        self.angular_velocity = [
            av0[0] + k2_av[0] * dt_k,
            av0[1] + k2_av[1] * dt_k,
            av0[2] + k2_av[2] * dt_k,
        ];
        let (k3_pos, k3_vel, k3_att, k3_av) = self.derivatives(controls);

        // k4 (stage 3): evaluate at t + c3*dt using k3
        self.position = Coord3D::new(
            p0.x + k3_pos[0] * dt_full,
            p0.y + k3_pos[1] * dt_full,
            p0.z + k3_pos[2] * dt_full,
        );
        self.velocity = [
            v0[0] + k3_vel[0] * dt_full,
            v0[1] + k3_vel[1] * dt_full,
            v0[2] + k3_vel[2] * dt_full,
        ];
        self.attitude = quaternion_normalize(&[
            a0[0] + k3_att[0] * dt_full,
            a0[1] + k3_att[1] * dt_full,
            a0[2] + k3_att[2] * dt_full,
            a0[3] + k3_att[3] * dt_full,
        ]);
        self.angular_velocity = [
            av0[0] + k3_av[0] * dt_full,
            av0[1] + k3_av[1] * dt_full,
            av0[2] + k3_av[2] * dt_full,
        ];
        let (k4_pos, k4_vel, k4_att, k4_av) = self.derivatives(controls);

        // Final combination: Σ b_i · k_i
        let b = RK4_B;
        let combine = |k1: &[Scalar; 3],
                       k2: &[Scalar; 3],
                       k3: &[Scalar; 3],
                       k4: &[Scalar; 3]|
         -> [Scalar; 3] {
            [
                dt * (b[0] * k1[0] + b[1] * k2[0] + b[2] * k3[0] + b[3] * k4[0]),
                dt * (b[0] * k1[1] + b[1] * k2[1] + b[2] * k3[1] + b[3] * k4[1]),
                dt * (b[0] * k1[2] + b[1] * k2[2] + b[2] * k3[2] + b[3] * k4[2]),
            ]
        };
        let combine_att = |k1: &[Scalar; 4],
                           k2: &[Scalar; 4],
                           k3: &[Scalar; 4],
                           k4: &[Scalar; 4]|
         -> [Scalar; 4] {
            let mut q = [0.0; 4];
            for i in 0..4 {
                q[i] = dt * (b[0] * k1[i] + b[1] * k2[i] + b[2] * k3[i] + b[3] * k4[i]);
            }
            q
        };

        self.position = Coord3D::new(
            p0.x + combine(&k1_pos, &k2_pos, &k3_pos, &k4_pos)[0],
            p0.y + combine(&k1_pos, &k2_pos, &k3_pos, &k4_pos)[1],
            p0.z + combine(&k1_pos, &k2_pos, &k3_pos, &k4_pos)[2],
        );
        self.velocity = combine(&k1_vel, &k2_vel, &k3_vel, &k4_vel);
        self.attitude = quaternion_normalize(&[
            a0[0] + combine_att(&k1_att, &k2_att, &k3_att, &k4_att)[0],
            a0[1] + combine_att(&k1_att, &k2_att, &k3_att, &k4_att)[1],
            a0[2] + combine_att(&k1_att, &k2_att, &k3_att, &k4_att)[2],
            a0[3] + combine_att(&k1_att, &k2_att, &k3_att, &k4_att)[3],
        ]);
        self.angular_velocity = combine(&k1_av, &k2_av, &k3_av, &k4_av);
    }

    /// Compute a trimmed (equilibrium) flight condition for straight-and-level
    /// flight at a given speed (m/s) and altitude (m).
    ///
    /// Returns `[thrust, elevator, aileron, rudder]` or an error message.
    pub fn trim(&self, speed: Scalar, altitude: Scalar) -> Result<[Scalar; 4], String> {
        if speed <= 0.0 {
            return Err("Speed must be positive".to_string());
        }
        if altitude < 0.0 {
            return Err("Altitude must be non-negative".to_string());
        }

        let density = super::physics::IsaAtmosphere::density(altitude);
        let q_inf = 0.5 * density * speed * speed;
        if q_inf <= 0.0 {
            return Err("Dynamic pressure too low for trim".to_string());
        }

        // For level flight: L = W, T = D
        let weight = self.mass * 9.80665;
        let cl_trim = weight / (q_inf * self.wing_area);
        let alpha_trim = cl_trim / self.aerodynamics.cl_alpha;

        // Clamp alpha to reasonable range
        let alpha_trim = alpha_trim.clamp(-0.5, 0.5);

        let cd_trim = self.aerodynamics.cd(alpha_trim);
        let drag_trim = q_inf * self.wing_area * cd_trim;
        let thrust_trim = drag_trim;

        // Elevator deflection to trim (simplified)
        let cm_alpha = -0.1; // pitch stiffness
        let cm_delta = -0.5; // elevator effectiveness
        let cm0 = 0.02; // zero-alpha pitching moment
        let cm_alpha_eff = cm_alpha * alpha_trim + cm0;
        let elevator_trim = -cm_alpha_eff / cm_delta;

        // Aileron and rudder neutral for symmetric flight
        let aileron_trim = 0.0;
        let rudder_trim = 0.0;

        Ok([thrust_trim, elevator_trim, aileron_trim, rudder_trim])
    }
}

/// Helper: approximate r_dot for inertia coupling (used in derivative computation).
fn r_dot_approx(_val: Scalar) -> Scalar {
    0.0
}

// ──────────────────────────────────────────────
// Autopilot (PID Controller)
// ──────────────────────────────────────────────

/// Simple PID autopilot controller.
pub struct Autopilot {
    /// Proportional gain.
    pub kp: Scalar,
    /// Integral gain.
    pub ki: Scalar,
    /// Derivative gain.
    pub kd: Scalar,
    /// Target setpoint.
    pub setpoint: Scalar,
    /// Accumulated integral error.
    pub integral: Scalar,
    /// Previous error (for derivative term).
    pub prev_error: Scalar,
}

impl Autopilot {
    /// Compute the control output given the current measured value and time step.
    pub fn compute(&mut self, measured: Scalar, dt: Scalar) -> Scalar {
        let error = self.setpoint - measured;
        self.integral += error * dt;

        // Clamp integral to prevent windup
        self.integral = self.integral.clamp(-100.0, 100.0);

        let derivative = if dt > 0.0 {
            (error - self.prev_error) / dt
        } else {
            0.0
        };

        self.prev_error = error;

        self.kp * error + self.ki * self.integral + self.kd * derivative
    }

    /// Reset the integrator and previous error.
    pub fn reset(&mut self) {
        self.integral = 0.0;
        self.prev_error = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::aerospace::aerodynamics::AircraftAerodynamics;

    #[test]
    fn test_euler_to_quaternion_identity() {
        let q = euler_to_quaternion(0.0, 0.0, 0.0);
        assert!((q[0] - 1.0).abs() < 1e-10);
        assert!((q[1]).abs() < 1e-10);
        assert!((q[2]).abs() < 1e-10);
        assert!((q[3]).abs() < 1e-10);
    }

    #[test]
    fn test_quaternion_to_euler_identity() {
        let q = [1.0, 0.0, 0.0, 0.0];
        let (r, p, y) = quaternion_to_euler(&q);
        assert!((r).abs() < 1e-10);
        assert!((p).abs() < 1e-10);
        assert!((y).abs() < 1e-10);
    }

    #[test]
    fn test_euler_quaternion_roundtrip() {
        let roll = 0.3;
        let pitch = -0.15;
        let yaw = 1.2;
        let q = euler_to_quaternion(roll, pitch, yaw);
        let (r2, p2, y2) = quaternion_to_euler(&q);
        assert!((r2 - roll).abs() < 1e-10);
        assert!((p2 - pitch).abs() < 1e-10);
        assert!((y2 - yaw).abs() < 1e-10);
    }

    #[test]
    fn test_quaternion_normalize() {
        let q = [2.0, 0.0, 0.0, 0.0];
        let qn = quaternion_normalize(&q);
        assert!((qn[0] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_quaternion_multiply_identity() {
        let q = [1.0, 0.0, 0.0, 0.0];
        let r = [0.707, 0.0, 0.707, 0.0];
        let qr = quaternion_multiply(&q, &r);
        assert!((qr[0] - r[0]).abs() < 1e-10);
    }

    #[test]
    fn test_quaternion_rotate() {
        let q = euler_to_quaternion(0.0, core::f64::consts::FRAC_PI_2, 0.0);
        let v = Coord3D::new(1.0, 0.0, 0.0);
        let rotated = quaternion_rotate(&q, &v);
        // 90° pitch should rotate x to -z
        assert!((rotated.x).abs() < 1e-10);
        assert!((rotated.y).abs() < 1e-10);
        assert!((rotated.z + 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_six_dof_derivatives_nonzero() {
        let aero = AircraftAerodynamics {
            wing_area: 50.0,
            aspect_ratio: 8.0,
            cd0: 0.02,
            oswald: 0.85,
            cl_alpha: 2.0 * core::f64::consts::PI,
            alpha_stall: 0.25,
        };
        let aircraft = SixDofAircraft {
            mass: 50_000.0,
            inertia: [[1e6, 0.0, 0.0], [0.0, 5e6, 0.0], [0.0, 0.0, 6e6]],
            position: Coord3D::new(0.0, 0.0, -1000.0),
            velocity: [100.0, 0.0, 0.0],
            attitude: [1.0, 0.0, 0.0, 0.0],
            angular_velocity: [0.0, 0.0, 0.0],
            aerodynamics: aero,
            wing_area: 50.0,
            chord: 4.0,
            span: 20.0,
        };
        let controls = [100_000.0, 0.0, 0.0, 0.0];
        let (pos_dot, vel_dot, _att_dot, _av_dot) = aircraft.derivatives(&controls);
        // Should produce some forward acceleration
        assert!(pos_dot[0].abs() > 0.0);
        assert!(vel_dot[0].abs() > 0.0);
    }

    #[test]
    fn test_six_dof_rk4_step() {
        let aero = AircraftAerodynamics {
            wing_area: 50.0,
            aspect_ratio: 8.0,
            cd0: 0.02,
            oswald: 0.85,
            cl_alpha: 2.0 * core::f64::consts::PI,
            alpha_stall: 0.25,
        };
        let mut aircraft = SixDofAircraft {
            mass: 50_000.0,
            inertia: [[1e6, 0.0, 0.0], [0.0, 5e6, 0.0], [0.0, 0.0, 6e6]],
            position: Coord3D::new(0.0, 0.0, -1000.0),
            velocity: [100.0, 0.0, 0.0],
            attitude: [1.0, 0.0, 0.0, 0.0],
            angular_velocity: [0.0, 0.0, 0.0],
            aerodynamics: aero,
            wing_area: 50.0,
            chord: 4.0,
            span: 20.0,
        };
        let controls = [100_000.0, 0.0, 0.0, 0.0];
        aircraft.rk4_step(&controls, 0.01);
        // Position should have moved
        assert!(aircraft.position.x > 0.0);
    }

    #[test]
    fn test_trim_level_flight() {
        let aero = AircraftAerodynamics {
            wing_area: 50.0,
            aspect_ratio: 8.0,
            cd0: 0.02,
            oswald: 0.85,
            cl_alpha: 2.0 * core::f64::consts::PI,
            alpha_stall: 0.25,
        };
        let aircraft = SixDofAircraft {
            mass: 50_000.0,
            inertia: [[1e6, 0.0, 0.0], [0.0, 5e6, 0.0], [0.0, 0.0, 6e6]],
            position: Coord3D::new(0.0, 0.0, 0.0),
            velocity: [100.0, 0.0, 0.0],
            attitude: [1.0, 0.0, 0.0, 0.0],
            angular_velocity: [0.0, 0.0, 0.0],
            aerodynamics: aero,
            wing_area: 50.0,
            chord: 4.0,
            span: 20.0,
        };
        let trim = aircraft.trim(100.0, 0.0);
        assert!(trim.is_ok());
        let c = trim.unwrap();
        assert!(c[0] > 0.0); // thrust
    }

    #[test]
    fn test_trim_bad_speed() {
        let aero = AircraftAerodynamics {
            wing_area: 50.0,
            aspect_ratio: 8.0,
            cd0: 0.02,
            oswald: 0.85,
            cl_alpha: 2.0 * core::f64::consts::PI,
            alpha_stall: 0.25,
        };
        let aircraft = SixDofAircraft {
            mass: 50_000.0,
            inertia: [[1e6, 0.0, 0.0], [0.0, 5e6, 0.0], [0.0, 0.0, 6e6]],
            position: Coord3D::new(0.0, 0.0, 0.0),
            velocity: [100.0, 0.0, 0.0],
            attitude: [1.0, 0.0, 0.0, 0.0],
            angular_velocity: [0.0, 0.0, 0.0],
            aerodynamics: aero,
            wing_area: 50.0,
            chord: 4.0,
            span: 20.0,
        };
        assert!(aircraft.trim(-1.0, 0.0).is_err());
    }

    #[test]
    fn test_autopilot_constant_setpoint() {
        let mut ap = Autopilot {
            kp: 1.0,
            ki: 0.0,
            kd: 0.0,
            setpoint: 10.0,
            integral: 0.0,
            prev_error: 0.0,
        };
        let output = ap.compute(10.0, 0.1);
        assert!((output).abs() < 1e-10);
    }

    #[test]
    fn test_autopilot_proportional() {
        let mut ap = Autopilot {
            kp: 2.0,
            ki: 0.0,
            kd: 0.0,
            setpoint: 10.0,
            integral: 0.0,
            prev_error: 0.0,
        };
        let output = ap.compute(7.0, 0.1);
        assert!((output - 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_autopilot_reset() {
        let mut ap = Autopilot {
            kp: 1.0,
            ki: 0.1,
            kd: 0.0,
            setpoint: 10.0,
            integral: 50.0,
            prev_error: 5.0,
        };
        ap.reset();
        assert!((ap.integral).abs() < 1e-10);
        assert!((ap.prev_error).abs() < 1e-10);
    }
}
