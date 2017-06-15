use num::bigint::BigInt;
use num::integer::Integer;
use num::traits::Signed;
use num::traits::cast::ToPrimitive;
use serde;
use std::fmt;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RpNumber {
    // base digits
    pub digits: BigInt,
    // where the decimal point is
    pub decimal: usize,
}

impl RpNumber {
    fn multiple(&self) -> BigInt {
        let mut multiple: BigInt = 1.into();

        for _ in 0..self.decimal {
            let ten: BigInt = 10.into();
            multiple = multiple * ten;
        }

        multiple
    }

    pub fn to_u64(&self) -> Option<u64> {
        let m = self.multiple();

        self.digits
            .checked_div(&m)
            .and_then(|r| r.to_u64())
    }

    pub fn to_u32(&self) -> Option<u32> {
        self.to_u64().map(|v| v as u32)
    }

    pub fn to_usize(&self) -> Option<usize> {
        self.to_u64().map(|v| v as usize)
    }

    pub fn to_f64(&self) -> Option<f64> {
        let multiple = self.multiple();
        let (base, decimal) = self.digits.div_mod_floor(&multiple);

        base.to_f64().and_then(|base| {
            decimal.to_f64()
                .and_then(|decimal| multiple.to_f64().map(|multiple| base + (decimal / multiple)))
        })
    }
}

impl From<u32> for RpNumber {
    fn from(value: u32) -> RpNumber {
        RpNumber {
            digits: value.into(),
            decimal: 0usize,
        }
    }
}

impl From<i32> for RpNumber {
    fn from(value: i32) -> RpNumber {
        RpNumber {
            digits: value.into(),
            decimal: 0usize,
        }
    }
}

impl fmt::Debug for RpNumber {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "RpNumber({})", self)
    }
}

impl fmt::Display for RpNumber {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.decimal == 0 {
            return write!(f, "{}", self.digits);
        }

        let multiple = self.multiple();
        let (base, decimal) = self.digits.abs().div_mod_floor(&multiple);

        let decimal = format!("{}", decimal);

        // pad leading zeros if needed
        let decimal = if decimal.len() < self.decimal {
            let mut s = String::new();

            for _ in decimal.len()..self.decimal {
                s.push('0');
            }

            s.push_str(&decimal);
            s
        } else {
            decimal
        };

        if self.digits.is_negative() {
            write!(f, "-{}.{}", base, decimal)
        } else {
            write!(f, "{}.{}", base, decimal)
        }
    }
}

impl serde::Serialize for RpNumber {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: serde::Serializer
    {
        let n = self.to_f64().unwrap();
        serializer.serialize_f64(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_number() {
        let n = RpNumber {
            digits: 104321.into(),
            decimal: 2,
        };

        assert_eq!(Some(1043), n.to_u64());
        assert_eq!(Some(1043.21), n.to_f64());
    }

    #[test]
    fn test_negative() {
        let n = RpNumber {
            digits: (-104321).into(),
            decimal: 2,
        };

        assert_eq!(None, n.to_u64());
        assert_eq!(Some(-1043.21), n.to_f64());
    }

    #[test]
    fn test_display() {
        let n = RpNumber {
            digits: (104321).into(),
            decimal: 2,
        };

        assert_eq!("1043.21", format!("{}", n));

        let n2 = RpNumber {
            digits: (104321).into(),
            decimal: 0,
        };

        assert_eq!("104321", format!("{}", n2));

        let n3 = RpNumber {
            digits: (104321).into(),
            decimal: 10,
        };

        assert_eq!("0.0000104321", format!("{}", n3));
    }
}
