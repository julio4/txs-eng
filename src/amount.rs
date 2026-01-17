use std::fmt;

/// Fixed-point decimal with 4 decimal places, stored as a scaled integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Amount(i64);

impl Amount {
    const SCALE: i64 = 10_000;

    pub fn from_float(value: f64) -> Self {
        Amount((value * Self::SCALE as f64).round() as i64)
    }

    pub fn from_scaled(value: i64) -> Self {
        Amount(value)
    }
}

impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.0 < 0 { "-" } else { "" };
        let abs = self.0.abs();
        let whole = abs / Self::SCALE;
        let frac = abs % Self::SCALE;
        write!(f, "{sign}{whole}.{frac:04}")
    }
}

impl std::ops::Add for Amount {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Amount(self.0 + rhs.0)
    }
}

impl std::ops::AddAssign for Amount {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl std::ops::SubAssign for Amount {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_scaled_preserves_value() {
        let amount = Amount::from_scaled(123456);
        assert_eq!(amount, Amount(123456));
    }

    #[test]
    fn from_float_converts_correctly() {
        assert_eq!(Amount::from_float(100.0), Amount::from_scaled(1_000_000));
        assert_eq!(Amount::from_float(1.5), Amount::from_scaled(15_000));
        assert_eq!(Amount::from_float(0.0001), Amount::from_scaled(1));
    }

    #[test]
    fn from_float_rounds_correctly() {
        assert_eq!(Amount::from_float(1.23456), Amount::from_scaled(12346));
        assert_eq!(Amount::from_float(1.23454), Amount::from_scaled(12345));
    }

    #[test]
    fn from_float_handles_negative() {
        assert_eq!(Amount::from_float(-50.25), Amount::from_scaled(-502_500));
    }

    #[test]
    fn display_formats_positive() {
        assert_eq!(Amount::from_scaled(1_000_000).to_string(), "100.0000");
        assert_eq!(Amount::from_scaled(15_000).to_string(), "1.5000");
        assert_eq!(Amount::from_scaled(1).to_string(), "0.0001");
        assert_eq!(Amount::from_scaled(0).to_string(), "0.0000");
    }

    #[test]
    fn display_formats_negative() {
        assert_eq!(Amount::from_scaled(-502_500).to_string(), "-50.2500");
        assert_eq!(Amount::from_scaled(-1).to_string(), "-0.0001");
    }

    #[test]
    fn default_is_zero() {
        assert_eq!(Amount::default(), Amount::from_scaled(0));
    }

    #[test]
    fn add() {
        let a = Amount::from_scaled(100);
        let b = Amount::from_scaled(50);
        assert_eq!(a + b, Amount::from_scaled(150));
    }

    #[test]
    fn add_assign() {
        let mut a = Amount::from_scaled(100);
        a += Amount::from_scaled(50);
        assert_eq!(a, Amount::from_scaled(150));
    }

    #[test]
    fn sub_assign() {
        let mut a = Amount::from_scaled(100);
        a -= Amount::from_scaled(30);
        assert_eq!(a, Amount::from_scaled(70));
    }

    #[test]
    fn ordering() {
        let small = Amount::from_scaled(100);
        let large = Amount::from_scaled(200);
        assert!(small < large);
        assert!(large > small);
    }

    #[test]
    fn negative_ordering() {
        let negative = Amount::from_scaled(-100);
        let zero = Amount::from_scaled(0);
        let positive = Amount::from_scaled(100);
        assert!(negative < zero);
        assert!(zero < positive);
        assert!(negative < positive);
    }
}
