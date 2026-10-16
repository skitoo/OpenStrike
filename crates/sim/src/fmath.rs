//! The game's own floating point helpers, reproduced operation by operation so that rounding
//! matches: square root by doubling then 5 Newton steps (`Game_Dll.dll` `0x1005b3a0`), arctangent
//! by 20-term power series around 0 or 1 (`0x1005b2f0`, `0x1005b3f0`). The original runs on the x87
//! with double precision; the results here are computed in `f64`.

/// `0x100abec8`, `0x100abed0`: the game's truncated pi/2 and pi.
#[allow(clippy::approx_constant)] // the game's own, shorter, constants
const HALF_PI: f64 = 1.570_796_326_75;
#[allow(clippy::approx_constant)]
const PI: f64 = 3.141_592_653_5;

/// Series coefficients at `0x100b1de8`: around 0 (arctangent, with the game's sign error on the
/// last term) then around 1.
const SERIES: [[f64; 20]; 2] = [
    [
        0.0,
        1.0,
        0.0,
        -0.333_333_333_333_333_3,
        0.0,
        0.2,
        0.0,
        -0.142_857_142_857_142_85,
        0.0,
        0.111_111_111_111_111_1,
        0.0,
        -0.090_909_090_909_090_91,
        0.0,
        0.076_923_076_923_076_93,
        0.0,
        -0.066_666_666_666_666_67,
        0.0,
        0.058_823_529_411_764_705,
        0.0,
        0.052_631_578_947_368_42,
    ],
    [
        0.785_398_163_375,
        0.5,
        -0.25,
        0.083_333_333_333_333_33,
        0.0,
        -0.025,
        0.020_833_333_333_333_332,
        -0.008_928_571_428_571_428,
        0.0,
        0.003_472_222_222_222_222,
        -0.003_125,
        0.001_420_454_545_454_545_5,
        0.0,
        -0.000_600_961_538_461_538_5,
        0.0,
        -0.000_260_416_666_666_666_66,
        0.0,
        0.000_114_889_705_882_352_94,
        -0.000_108_506_944_444_444_44,
        5.139_802_631_578_947e-5,
    ],
];

/// Square root as the game computes it (note `sqrt(0) == 1/32`).
pub fn sqrt(v: f64) -> f64 {
    let mut x = 1.0;
    if 1.0 < v {
        loop {
            x += x;
            if x * x >= v {
                break;
            }
        }
    }
    for _ in 0..5 {
        if x == 0.0 {
            break;
        }
        x = (x * x + v) / (x + x);
    }
    x
}

fn series(t: f64) -> f64 {
    let k = if t < 0.5 { 0 } else { 1 };
    let u = t - k as f64;
    let (mut sum, mut power) = (0.0, 1.0);
    for c in SERIES[k] {
        sum += power * c;
        power *= u;
    }
    sum
}

/// Arctangent (`0x1005b2f0`).
pub fn atan(x: f64) -> f64 {
    if x < 0.0 {
        -atan(-x)
    } else if x <= 1.0 {
        series(x)
    } else {
        HALF_PI - atan(1.0 / x)
    }
}

/// Angle of `(x, y)` in `[-pi, pi]` (`0x1005b3f0`, called with `y` first).
pub fn atan2(y: f64, x: f64) -> f64 {
    if x.abs() < 1e-8 && y.abs() < 1e-8 {
        return 0.0;
    }
    let a = if x.abs() > y.abs() { atan(y.abs() / x.abs()) } else { HALF_PI - atan(x.abs() / y.abs()) };
    let a = match (x < 0.0, y < 0.0) {
        (false, false) => a,
        (false, true) => -a,
        (true, false) => PI - a,
        (true, true) => a - PI,
    };
    a.clamp(-PI, PI)
}

/// The game's pi as a single precision float (`0x100a8284`, one unit below `f32::consts::PI`).
fn pi_f32() -> f32 {
    f32::from_bits(0x4049_0fda)
}

/// One of the 32 headings (0 east, clockwise, 11.25 degrees each) towards `(dx, dy)`, as the
/// movement code computes it (`0x1002e2c4`): `trunc((atan2 + 2pi) / 2pi * 32 + 0.5) & 31`.
pub fn heading(dx: i32, dy: i32) -> u8 {
    let two_pi = (pi_f32() + pi_f32()) as f64;
    let a = atan2(dy as f64, dx as f64);
    let v = (a + two_pi) / two_pi * 32.0 + 0.5;
    (v as i32 & 31) as u8
}
