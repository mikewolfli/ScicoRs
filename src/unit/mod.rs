//! Unified Dimension and Unit System
//!
//! Implements the seven SI base dimensions, automatic derivation of
//! composite dimensions, cross-discipline unit support, and
//! normalization for numerical stability.

use crate::core::types::Scalar;

/// The seven SI base dimensions and an exponent for each.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dimension {
    /// Length exponent (L).
    pub length: i8,
    /// Mass exponent (M).
    pub mass: i8,
    /// Time exponent (T).
    pub time: i8,
    /// Electric current exponent (I).
    pub current: i8,
    /// Thermodynamic temperature exponent (Θ).
    pub temperature: i8,
    /// Amount of substance exponent (N).
    pub amount: i8,
    /// Luminous intensity exponent (J).
    pub intensity: i8,
}

impl Dimension {
    /// Dimensionless quantity.
    pub const fn dimensionless() -> Self {
        Self { length: 0, mass: 0, time: 0, current: 0, temperature: 0, amount: 0, intensity: 0 }
    }

    /// Length [L].
    pub const fn length() -> Self {
        Self { length: 1, ..Self::dimensionless() }
    }

    /// Mass [M].
    pub const fn mass() -> Self {
        Self { mass: 1, ..Self::dimensionless() }
    }

    /// Time [T].
    pub const fn time() -> Self {
        Self { time: 1, ..Self::dimensionless() }
    }

    /// Velocity [L T⁻¹].
    pub const fn velocity() -> Self {
        Self { length: 1, time: -1, ..Self::dimensionless() }
    }

    /// Acceleration [L T⁻²].
    pub const fn acceleration() -> Self {
        Self { length: 1, time: -2, ..Self::dimensionless() }
    }

    /// Force [M L T⁻²].
    pub const fn force() -> Self {
        Self { length: 1, mass: 1, time: -2, ..Self::dimensionless() }
    }

    /// Energy [M L² T⁻²].
    pub const fn energy() -> Self {
        Self { mass: 1, length: 2, time: -2, ..Self::dimensionless() }
    }

    /// Power [M L² T⁻³].
    pub const fn power() -> Self {
        Self { mass: 1, length: 2, time: -3, ..Self::dimensionless() }
    }

    /// Voltage [M L² T⁻³ I⁻¹].
    pub const fn voltage() -> Self {
        Self { mass: 1, length: 2, time: -3, current: -1, ..Self::dimensionless() }
    }

    /// Electric current [I].
    pub const fn electric_current() -> Self {
        Self { current: 1, ..Self::dimensionless() }
    }

    /// Resistance [M L² T⁻³ I⁻²].
    pub const fn resistance() -> Self {
        Self { mass: 1, length: 2, time: -3, current: -2, ..Self::dimensionless() }
    }

    /// Capacitance [M⁻¹ L⁻² T⁴ I²].
    pub const fn capacitance() -> Self {
        Self { mass: -1, length: -2, time: 4, current: 2, ..Self::dimensionless() }
    }

    /// Inductance [M L² T⁻² I⁻²].
    pub const fn inductance() -> Self {
        Self { mass: 1, length: 2, time: -2, current: -2, ..Self::dimensionless() }
    }

    /// Frequency [T⁻¹].
    pub const fn frequency() -> Self {
        Self { time: -1, ..Self::dimensionless() }
    }

    /// Check equality with another dimension.
    pub fn compatible_with(&self, other: &Dimension) -> bool {
        self.length == other.length
            && self.mass == other.mass
            && self.time == other.time
            && self.current == other.current
            && self.temperature == other.temperature
            && self.amount == other.amount
            && self.intensity == other.intensity
    }

    /// Multiply two dimensions (add exponents).
    pub fn multiply(&self, other: &Dimension) -> Self {
        Self {
            length: self.length + other.length,
            mass: self.mass + other.mass,
            time: self.time + other.time,
            current: self.current + other.current,
            temperature: self.temperature + other.temperature,
            amount: self.amount + other.amount,
            intensity: self.intensity + other.intensity,
        }
    }

    /// Divide by another dimension (subtract exponents).
    pub fn divide(&self, other: &Dimension) -> Self {
        Self {
            length: self.length - other.length,
            mass: self.mass - other.mass,
            time: self.time - other.time,
            current: self.current - other.current,
            temperature: self.temperature - other.temperature,
            amount: self.amount - other.amount,
            intensity: self.intensity - other.intensity,
        }
    }
}

/// A measured quantity with a value and dimension.
#[derive(Debug, Clone)]
pub struct Quantity {
    /// The numeric value in SI base units.
    pub value: Scalar,
    /// The physical dimension.
    pub dimension: Dimension,
}

impl Quantity {
    pub fn new(value: Scalar, dimension: Dimension) -> Self {
        Self { value, dimension }
    }

    pub fn dimensionless(value: Scalar) -> Self {
        Self { value, dimension: Dimension::dimensionless() }
    }
}

/// An enumeration of commonly used units for quick reference.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Unit {
    // Length
    Nanometer,
    Micrometer,
    Millimeter,
    Meter,
    Kilometer,
    AstronomicalUnit,
    LightYear,
    // Time
    Nanosecond,
    Microsecond,
    Millisecond,
    Second,
    Minute,
    Hour,
    // Mass
    Gram,
    Kilogram,
    // Current
    Ampere,
    // Temperature
    Kelvin,
    Celsius,
    // Amount
    Mole,
    // Luminous
    Candela,
    // Derived
    Newton,
    Pascal,
    Joule,
    Watt,
    Volt,
    Ohm,
    Farad,
    Henry,
    Hertz,
}

impl Unit {
    /// Convert a value from this unit to SI base units.
    pub fn to_si(&self, value: Scalar) -> Scalar {
        match self {
            Self::Nanometer => value * 1e-9,
            Self::Micrometer => value * 1e-6,
            Self::Millimeter => value * 1e-3,
            Self::Meter => value,
            Self::Kilometer => value * 1e3,
            Self::AstronomicalUnit => value * 1.495978707e11,
            Self::LightYear => value * 9.4607304725808e15,
            Self::Nanosecond => value * 1e-9,
            Self::Microsecond => value * 1e-6,
            Self::Millisecond => value * 1e-3,
            Self::Second => value,
            Self::Minute => value * 60.0,
            Self::Hour => value * 3600.0,
            Self::Gram => value * 1e-3,
            Self::Kilogram => value,
            Self::Ampere => value,
            Self::Kelvin => value,
            Self::Celsius => value + 273.15,
            Self::Mole => value,
            Self::Candela => value,
            Self::Newton => value,
            Self::Pascal => value,
            Self::Joule => value,
            Self::Watt => value,
            Self::Volt => value,
            Self::Ohm => value,
            Self::Farad => value,
            Self::Henry => value,
            Self::Hertz => value,
        }
    }

    /// Convert a value from SI base units to this unit.
    pub fn from_si(&self, value: Scalar) -> Scalar {
        match self {
            Self::Nanometer => value * 1e9,
            Self::Micrometer => value * 1e6,
            Self::Millimeter => value * 1e3,
            Self::Meter => value,
            Self::Kilometer => value * 1e-3,
            Self::AstronomicalUnit => value / 1.495978707e11,
            Self::LightYear => value / 9.4607304725808e15,
            Self::Nanosecond => value * 1e9,
            Self::Microsecond => value * 1e6,
            Self::Millisecond => value * 1e3,
            Self::Second => value,
            Self::Minute => value / 60.0,
            Self::Hour => value / 3600.0,
            Self::Gram => value * 1e3,
            Self::Kilogram => value,
            Self::Ampere => value,
            Self::Kelvin => value,
            Self::Celsius => value - 273.15,
            Self::Mole => value,
            Self::Candela => value,
            Self::Newton => value,
            Self::Pascal => value,
            Self::Joule => value,
            Self::Watt => value,
            Self::Volt => value,
            Self::Ohm => value,
            Self::Farad => value,
            Self::Henry => value,
            Self::Hertz => value,
        }
    }

    pub fn dimension(&self) -> Dimension {
        match self {
            Self::Nanometer | Self::Micrometer | Self::Millimeter | Self::Meter | Self::Kilometer
            | Self::AstronomicalUnit | Self::LightYear => Dimension::length(),
            Self::Nanosecond | Self::Microsecond | Self::Millisecond | Self::Second | Self::Minute | Self::Hour => {
                Dimension::time()
            }
            Self::Gram | Self::Kilogram => Dimension::mass(),
            Self::Ampere => Dimension::electric_current(),
            Self::Kelvin | Self::Celsius => Dimension { temperature: 1, ..Dimension::dimensionless() },
            Self::Mole => Dimension { amount: 1, ..Dimension::dimensionless() },
            Self::Candela => Dimension { intensity: 1, ..Dimension::dimensionless() },
            Self::Newton => Dimension::force(),
            Self::Pascal => Dimension::force().divide(&Dimension::length().multiply(&Dimension::length())),
            Self::Joule | Self::Watt => Dimension::energy(),
            Self::Volt => Dimension::voltage(),
            Self::Ohm => Dimension::resistance(),
            Self::Farad => Dimension::capacitance(),
            Self::Henry => Dimension::inductance(),
            Self::Hertz => Dimension::frequency(),
        }
    }
}

/// Normalize a value to the core [-1, 1] range for numerical stability.
pub fn normalize_to_core(value: Scalar, scale: Scalar) -> Scalar {
    if scale == 0.0 {
        return 0.0;
    }
    (value / scale).clamp(-1.0, 1.0)
}

/// Denormalize from core [-1, 1] back to physical value.
pub fn denormalize_from_core(normalized: Scalar, scale: Scalar) -> Scalar {
    normalized * scale
}
