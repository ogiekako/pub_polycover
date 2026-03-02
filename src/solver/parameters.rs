use rand::{rngs::SmallRng, Rng};

#[derive(Clone, Debug)]
pub struct Parameters {
    pub initial_cov2_check_depth: usize,
    // 0-1
    pub use_restriction_prob: f64,
    pub use_frontness_threshold_prob: f64,
    pub use_tie_prob: f64,
    pub max_temp: f64,
    pub min_temp: f64,

    // 0-100
    pub act_flip: usize,
    pub act_clear: usize,
    pub act_set: usize,
    pub act_swap: usize,
    pub act_clear_to_break: usize,
    pub act_set_to_break: usize,
    pub act_zip: usize,
    pub act_2x2: usize,
    pub act_3x3: usize,
    // 0-50
    pub act_2x2_to_break: usize,
    pub act_3x3_to_break: usize,

    // 1: linear, 2: quadratic, 3: cubic, ...
    pub temp_power: f64,
    pub temp_exp: bool,

    pub run_full_check: bool,
    pub frontness_threshold: usize,

    pub pen_depenalize_same_position: bool,
    pub pen_unplug_coeff: f64,
    pub pen_unplug_power: f64,
    pub pen_unplug_diag_mult: f64,
    pub pen_base: f64,
    pub pen_w0_common_rect: f64,
    pub pen_w1_common_rect: f64,
    pub pen_w2_common_rect: f64,
    pub pen_friction_log_base: f64,

    // 0-1
    pub pen_friction_base: f64,
    pub pen_friction_weight: f64,
    pub pen_depth_base: f64,
    pub pen_depth_weight: f64,
    pub pen_shallowness_diag_weight: f64,
    pub pen_shallowness_depth_power: f64,
    pub pen_shallowness_base: f64,
    pub pen_nth_weight: f64,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            initial_cov2_check_depth: 5,
            use_restriction_prob: 0.99,
            use_frontness_threshold_prob: 0.7,
            use_tie_prob: 0.005,
            max_temp: 2.5,
            min_temp: 0.0,
            act_flip: 60,
            act_clear: 10,
            act_set: 0,
            act_swap: 1,
            act_clear_to_break: 3,
            act_set_to_break: 0,
            act_zip: 1,
            act_2x2: 4,
            act_3x3: 2,
            act_2x2_to_break: 1,
            act_3x3_to_break: 1,
            temp_exp: true,
            temp_power: 2.5,
            run_full_check: true,
            frontness_threshold: 6,

            pen_depenalize_same_position: false,
            pen_unplug_coeff: 10.0,
            pen_unplug_power: 1.86,
            pen_unplug_diag_mult: 1.15,
            pen_base: 2.25,
            pen_w0_common_rect: 2.0,
            pen_w1_common_rect: 2.0,
            pen_w2_common_rect: 2.0,
            pen_friction_log_base: 2.0,

            pen_friction_base: 0.05,
            pen_friction_weight: 0.1,
            pen_depth_base: 0.51,
            pen_depth_weight: 0.9,
            pen_shallowness_diag_weight: 1.0,
            pen_shallowness_depth_power: 1.5,
            pen_shallowness_base: 0.9,
            pen_nth_weight: 0.05,
        }
    }
}

impl Parameters {
    pub fn random(rng: &mut SmallRng) -> Self {
        let mut p = Self::default();

        // p.initial_cov2_check_depth = rng.gen_range(3..6);
        // p.use_restriction_prob = rng.gen_range(0.8..1.0);
        p.use_frontness_threshold_prob = rng.gen_range(0.2..0.5);
        // p.use_tie_prob = rng.gen_range(0.0..0.1);
        // p.max_temp = rng.gen_range(1000.0..2500.0);
        // p.min_temp = rng.gen_range(0.0..25.0);
        p.act_flip = rng.gen_range(50..60);
        p.act_clear = rng.gen_range(0..20);
        // p.act_set = rng.gen_range(0..5);
        // p.act_swap = rng.gen_range(0..5);
        p.act_clear_to_break = rng.gen_range(0..5);
        // p.act_set_to_break = rng.gen_range(0..5);
        // p.act_zip = rng.gen_range(0..5);
        // p.act_2x2 = rng.gen_range(0..5);
        // p.act_3x3 = rng.gen_range(0..5);
        p.act_2x2_to_break = rng.gen_range(0..5);
        p.act_3x3_to_break = rng.gen_range(0..3);

        p.temp_exp = rng.gen_bool(0.5);
        p.temp_power = rng.gen_range(1.5..4.5);

        p.frontness_threshold = rng.gen_range(6..8);

        // p.pen_depenalize_same_position = rng.gen_bool(0.5);
        // p.pen_unplug_power = rng.gen_range(0.5..2.0);
        // p.pen_unplug_diag_mult = rng.gen_range(0.8..1.2);
        // p.pen_base = rng.gen_range(1.1..3.0);
        // p.pen_w0_common_rect = rng.gen_range(0.0..2.0);
        // p.pen_w1_common_rect = rng.gen_range(0.0..2.0);
        // p.pen_w2_common_rect = rng.gen_range(0.0..2.0);
        // p.pen_friction_log_base = rng.gen_range(1.1..3.1);

        // p.pen_friction_base = rng.gen_range(0.05..0.1);
        // p.pen_friction_weight = rng.gen_range(0.05..0.15);
        // p.pen_depth_base = rng.gen_range(0.2..0.6);
        // p.pen_depth_weight = rng.gen_range(0.0..0.4);
        p.pen_shallowness_diag_weight = rng.gen_range(0.6..0.8);
        // p.pen_shallowness_depth_power = rng.gen_range(1.0..3.0);
        p.pen_shallowness_base = rng.gen_range(0.9..0.95);
        // p.pen_nth_weight = rng.gen_range(0.05..0.1);

        p
    }
}
