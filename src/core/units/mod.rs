//! Units & Dimensions System (Phase 11).
//!
//! Provides the 7 SI base dimensions, derived dimensions, unit
//! definitions with scale factors, and automatic conversion between
//! compatible units.
//!
//! # Structure
//!
//! - **`SiDimension`** — the 7 base dimensions as a bitmask exponent vector
//! - **`Unit`** — a concrete unit with dimension, scale, and offset
//! - **`Quantity`** — a numeric value paired with a unit
//! - Pre-defined constants for common units (m, s, kg, V, A, °C, etc.)
//! - Conversion between compatible units

use crate::core::types::Scalar;

// ──────────────────────────────────────────────
// 1. SI Base Dimensions
// ──────────────────────────────────────────────

/// Represents a physical dimension as exponents of the 7 SI base dimensions.
///
/// Stored as `[L, M, T, I, Θ, N, J]` exponents for:
/// - Length (L, metre)
/// - Mass (M, kilogram)
/// - Time (T, second)
/// - Electric current (I, ampere)
/// - Temperature (Θ, kelvin)
/// - Amount of substance (N, mole)
/// - Luminous intensity (J, candela)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Dimension {
    /// Exponents for [L, M, T, I, Θ, N, J].
    pub exponents: [i8; 7],
}

impl Dimension {
    /// Dimensionless (all exponents zero).
    pub const fn dimensionless() -> Self {
        Self { exponents: [0; 7] }
    }

    /// Length [L].
    pub const fn length() -> Self {
        Self {
            exponents: [1, 0, 0, 0, 0, 0, 0],
        }
    }

    /// Mass [M].
    pub const fn mass() -> Self {
        Self {
            exponents: [0, 1, 0, 0, 0, 0, 0],
        }
    }

    /// Time [T].
    pub const fn time() -> Self {
        Self {
            exponents: [0, 0, 1, 0, 0, 0, 0],
        }
    }

    /// Electric current [I].
    pub const fn current() -> Self {
        Self {
            exponents: [0, 0, 0, 1, 0, 0, 0],
        }
    }

    /// Temperature [Θ].
    pub const fn temperature() -> Self {
        Self {
            exponents: [0, 0, 0, 0, 1, 0, 0],
        }
    }

    /// Amount of substance [N].
    pub const fn amount() -> Self {
        Self {
            exponents: [0, 0, 0, 0, 0, 1, 0],
        }
    }

    /// Luminous intensity [J].
    pub const fn intensity() -> Self {
        Self {
            exponents: [0, 0, 0, 0, 0, 0, 1],
        }
    }

    // ── Derived dimensions ──

    /// Velocity [L·T⁻¹].
    pub const fn velocity() -> Self {
        Self {
            exponents: [1, 0, -1, 0, 0, 0, 0],
        }
    }

    /// Acceleration [L·T⁻²].
    pub const fn acceleration() -> Self {
        Self {
            exponents: [1, 0, -2, 0, 0, 0, 0],
        }
    }

    /// Force [M·L·T⁻²] (Newton).
    pub const fn force() -> Self {
        Self {
            exponents: [1, 1, -2, 0, 0, 0, 0],
        }
    }

    /// Energy [M·L²·T⁻²] (Joule).
    pub const fn energy() -> Self {
        Self {
            exponents: [2, 1, -2, 0, 0, 0, 0],
        }
    }

    /// Power [M·L²·T⁻³] (Watt).
    pub const fn power() -> Self {
        Self {
            exponents: [2, 1, -3, 0, 0, 0, 0],
        }
    }

    /// Electric voltage [M·L²·T⁻³·I⁻¹] (Volt).
    pub const fn voltage() -> Self {
        Self {
            exponents: [2, 1, -3, -1, 0, 0, 0],
        }
    }

    /// Electric resistance [M·L²·T⁻³·I⁻²] (Ohm).
    pub const fn resistance() -> Self {
        Self {
            exponents: [2, 1, -3, -2, 0, 0, 0],
        }
    }

    /// Electric capacitance [M⁻¹·L⁻²·T⁴·I²] (Farad).
    pub const fn capacitance() -> Self {
        Self {
            exponents: [-2, -1, 4, 2, 0, 0, 0],
        }
    }

    /// Frequency [T⁻¹] (Hertz).
    pub const fn frequency() -> Self {
        Self {
            exponents: [0, 0, -1, 0, 0, 0, 0],
        }
    }

    /// Pressure [M·L⁻¹·T⁻²] (Pascal).
    pub const fn pressure() -> Self {
        Self {
            exponents: [-1, 1, -2, 0, 0, 0, 0],
        }
    }

    /// Check if this dimension is dimensionless.
    pub fn is_dimensionless(&self) -> bool {
        self.exponents.iter().all(|&e| e == 0)
    }

    /// Multiply dimensions (add exponents).
    pub fn mul(&self, other: &Dimension) -> Self {
        let mut e = self.exponents;
        for (i, &o) in other.exponents.iter().enumerate() {
            e[i] += o;
        }
        Self { exponents: e }
    }

    /// Divide dimensions (subtract exponents).
    pub fn div(&self, other: &Dimension) -> Self {
        let mut e = self.exponents;
        for (i, &o) in other.exponents.iter().enumerate() {
            e[i] -= o;
        }
        Self { exponents: e }
    }

    /// Raise to a power.
    pub fn pow(&self, exp: i8) -> Self {
        let mut e = self.exponents;
        for ei in e.iter_mut() {
            *ei *= exp;
        }
        Self { exponents: e }
    }
}

// ──────────────────────────────────────────────
// 2. Unit Definition
// ──────────────────────────────────────────────

/// A concrete unit with dimension, scale factor, and optional offset.
///
/// - `scale`: conversion factor to SI base unit (e.g., 1000 for km → m)
/// - `offset`: additive offset (e.g., 273.15 for °C → K)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Unit {
    /// The physical dimension.
    pub dimension: Dimension,
    /// Scale factor relative to the SI base unit.
    pub scale: Scalar,
    /// Additive offset (for temperature conversions, etc.).
    pub offset: Scalar,
    /// Human-readable symbol (e.g. "m", "kg", "V").
    pub symbol: &'static str,
}

impl Unit {
    /// Create a new unit.
    pub const fn new(dimension: Dimension, scale: Scalar, offset: Scalar, symbol: &'static str) -> Self {
        Self { dimension, scale, offset, symbol }
    }

    /// Create a coherent SI unit (scale=1, offset=0).
    pub const fn si(dimension: Dimension, symbol: &'static str) -> Self {
        Self::new(dimension, 1.0, 0.0, symbol)
    }

    /// Check if two units have the same dimension (compatible for conversion).
    pub fn is_compatible(&self, other: &Unit) -> bool {
        self.dimension == other.dimension
    }

    /// Convert a value from this unit to another compatible unit.
    /// Returns `None` if the units are incompatible.
    pub fn convert(&self, value: Scalar, to: &Unit) -> Option<Scalar> {
        if !self.is_compatible(to) {
            return None;
        }
        // Convert to SI base: value_si = (value + self.offset) * self.scale
        let si_value = (value + self.offset) * self.scale;
        // Convert from SI to target: value_target = si_value / to.scale - to.offset
        let target_value = si_value / to.scale - to.offset;
        Some(target_value)
    }
}

// ──────────────────────────────────────────────
// 3. Pre-defined Units
// ──────────────────────────────────────────────

/// Common unit constants.
pub mod si_units {
    use super::{Dimension, Unit};

    // ── Length ──
    /// Metre (SI)
    pub const M: Unit = Unit::si(Dimension::length(), "m");
    /// Kilometre
    pub const KM: Unit = Unit::new(Dimension::length(), 1000.0, 0.0, "km");
    /// Centimetre
    pub const CM: Unit = Unit::new(Dimension::length(), 0.01, 0.0, "cm");
    /// Millimetre
    pub const MM: Unit = Unit::new(Dimension::length(), 0.001, 0.0, "mm");
    /// Micrometre
    pub const UM: Unit = Unit::new(Dimension::length(), 1e-6, 0.0, "µm");
    /// Nanometre
    pub const NM: Unit = Unit::new(Dimension::length(), 1e-9, 0.0, "nm");

    // ── Mass ──
    /// Kilogram (SI)
    pub const KG: Unit = Unit::si(Dimension::mass(), "kg");
    /// Gram
    pub const G: Unit = Unit::new(Dimension::mass(), 0.001, 0.0, "g");

    // ── Time ──
    /// Second (SI)
    pub const S: Unit = Unit::si(Dimension::time(), "s");
    /// Millisecond
    pub const MS: Unit = Unit::new(Dimension::time(), 0.001, 0.0, "ms");
    /// Microsecond
    pub const US: Unit = Unit::new(Dimension::time(), 1e-6, 0.0, "µs");

    // ── Current ──
    /// Ampere (SI)
    pub const A: Unit = Unit::si(Dimension::current(), "A");

    // ── Temperature ──
    /// Kelvin (SI)
    pub const K: Unit = Unit::si(Dimension::temperature(), "K");
    /// Celsius (offset 273.15)
    pub const CELSIUS: Unit = Unit::new(Dimension::temperature(), 1.0, 273.15, "°C");

    // ── Amount ──
    /// Mole (SI)
    pub const MOL: Unit = Unit::si(Dimension::amount(), "mol");

    // ── Derived ──
    /// Newton (force)
    pub const N: Unit = Unit::si(Dimension::force(), "N");
    /// Joule (energy)
    pub const J: Unit = Unit::si(Dimension::energy(), "J");
    /// Watt (power)
    pub const W: Unit = Unit::si(Dimension::power(), "W");
    /// Volt (voltage)
    pub const V: Unit = Unit::si(Dimension::voltage(), "V");
    /// Ohm (resistance)
    pub const OHM: Unit = Unit::si(Dimension::resistance(), "Ω");
    /// Farad (capacitance)
    pub const F: Unit = Unit::si(Dimension::capacitance(), "F");
    /// Hertz (frequency)
    pub const HZ: Unit = Unit::si(Dimension::frequency(), "Hz");
    /// Pascal (pressure)
    pub const PA: Unit = Unit::si(Dimension::pressure(), "Pa");
}

// ──────────────────────────────────────────────
// 4. Quantity (value + unit)
// ──────────────────────────────────────────────

/// A numeric value paired with its unit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quantity {
    /// The numeric value.
    pub value: Scalar,
    /// The unit of measurement.
    pub unit: Unit,
}

impl Quantity {
    /// Create a new quantity.
    pub fn new(value: Scalar, unit: Unit) -> Self {
        Self { value, unit }
    }

    /// Convert this quantity to a different unit.
    /// Returns `None` if the target unit is incompatible.
    pub fn to(&self, target_unit: &Unit) -> Option<Self> {
        let converted = self.unit.convert(self.value, target_unit)?;
        Some(Self {
            value: converted,
            unit: *target_unit,
        })
    }

    /// Check if this quantity is compatible with another unit.
    pub fn is_compatible(&self, other: &Unit) -> bool {
        self.unit.is_compatible(other)
    }

    /// Add another quantity (must have same dimension).
    pub fn add(&self, other: &Quantity) -> Option<Self> {
        if !self.unit.is_compatible(&other.unit) {
            return None;
        }
        // Convert both to SI, add, convert back
        let self_si = (self.value + self.unit.offset) * self.unit.scale;
        let other_si = (other.value + other.unit.offset) * other.unit.scale;
        let sum_si = self_si + other_si;
        let result = sum_si / self.unit.scale - self.unit.offset;
        Some(Self::new(result, self.unit))
    }

    /// Subtract another quantity (must have same dimension).
    pub fn sub(&self, other: &Quantity) -> Option<Self> {
        if !self.unit.is_compatible(&other.unit) {
            return None;
        }
        let self_si = (self.value + self.unit.offset) * self.unit.scale;
        let other_si = (other.value + other.unit.offset) * other.unit.scale;
        let diff_si = self_si - other_si;
        let result = diff_si / self.unit.scale - self.unit.offset;
        Some(Self::new(result, self.unit))
    }

    /// Multiply by a scalar.
    pub fn scale(&self, factor: Scalar) -> Self {
        Self::new(self.value * factor, self.unit)
    }

    /// Human-readable representation.
    pub fn format(&self) -> String {
        format!("{} {}", self.value, self.unit.symbol)
    }
}

// ──────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use super::si_units::*;

    #[test]
    fn test_dimension_operations() {
        let vel = Dimension::velocity();
        let t = Dimension::time();
        // velocity * time = length
        let length = vel.mul(&t);
        assert_eq!(length, Dimension::length());

        // length / time = velocity
        let v2 = Dimension::length().div(&Dimension::time());
        assert_eq!(v2, Dimension::velocity());

        // length^2 = area (exponents [2,0,0,0,0,0,0])
        let area = Dimension::length().pow(2);
        assert_eq!(area.exponents[0], 2);
    }

    #[test]
    fn test_dimensionless() {
        let d = Dimension::dimensionless();
        assert!(d.is_dimensionless());
        assert!(!Dimension::length().is_dimensionless());
    }

    #[test]
    fn test_unit_compatibility() {
        assert!(M.is_compatible(&KM));
        assert!(S.is_compatible(&MS));
        assert!(!M.is_compatible(&S));
    }

    #[test]
    fn test_length_conversion() {
        let value = 1.0; // 1 km
        let converted = KM.convert(value, &M).unwrap();
        assert!((converted - 1000.0).abs() < 1e-12);

        // Round-trip
        let back = M.convert(converted, &KM).unwrap();
        assert!((back - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_time_conversion() {
        let value = 1000.0; // 1000 ms
        let converted = MS.convert(value, &S).unwrap();
        assert!((converted - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_temperature_conversion() {
        // 0°C = 273.15 K
        let kelvin = CELSIUS.convert(0.0, &K).unwrap();
        assert!((kelvin - 273.15).abs() < 1e-12);

        // 273.15 K = 0°C
        let celsius = K.convert(273.15, &CELSIUS).unwrap();
        assert!((celsius - 0.0).abs() < 1e-12);
    }

    #[test]
    fn test_quantity_creation() {
        let q = Quantity::new(5.0, M);
        assert!((q.value - 5.0).abs() < 1e-12);
        assert_eq!(q.format(), "5 m");
    }

    #[test]
    fn test_quantity_conversion() {
        let q = Quantity::new(1.0, KM);
        let converted = q.to(&M).unwrap();
        assert!((converted.value - 1000.0).abs() < 1e-12);
        assert_eq!(converted.unit.symbol, "m");
    }

    #[test]
    fn test_quantity_arithmetic() {
        let a = Quantity::new(1.0, M);
        let b = Quantity::new(50.0, CM);
        let sum = a.add(&b).unwrap();
        assert!((sum.value - 1.5).abs() < 1e-12);

        let diff = a.sub(&b).unwrap();
        assert!((diff.value - 0.5).abs() < 1e-12);
    }

    #[test]
    fn test_incompatible_conversion() {
        assert!(M.convert(1.0, &S).is_none());
    }

    #[test]
    fn test_derived_dimensions() {
        // F = m * a → [M·L·T⁻²]
        let force = Dimension::mass().mul(&Dimension::acceleration());
        assert_eq!(force, Dimension::force());

        // E = F * L → [M·L²·T⁻²]
        let energy = Dimension::force().mul(&Dimension::length());
        assert_eq!(energy, Dimension::energy());

        // P = E / T → [M·L²·T⁻³]
        let power = Dimension::energy().div(&Dimension::time());
        assert_eq!(power, Dimension::power());
    }

    #[test]
    fn test_unit_symbols() {
        assert_eq!(M.symbol, "m");
        assert_eq!(KG.symbol, "kg");
        assert_eq!(V.symbol, "V");
        assert_eq!(OHM.symbol, "Ω");
        assert_eq!(CELSIUS.symbol, "°C");
    }

    #[test]
    fn test_nano_micro_conversion() {
        let nm_val = 1000.0; // 1000 nm
        let um = NM.convert(nm_val, &UM).unwrap();
        assert!((um - 1.0).abs() < 1e-12); // 1000 nm = 1 µm
    }
}
