use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::Signed;
use num_traits::cast::ToPrimitive;
use serde;
use std::fmt;
use std::result;

macro_rules! convert_method {
    ($ty:ty, $method:ident) => {
        pub fn $method(&self) -> Option<$ty> {
            let m = self.multiple();
            self.digits.checked_div(&m).and_then(|r| r.$method())
        }
    };
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RpNumber {
    // base digits
    pub digits: BigInt,
    // where the decimal point is
    pub decimal: usize,
}

impl RpNumber {
    convert_method!(i32, to_i32);
    convert_method!(i64, to_i64);
    convert_method!(u32, to_u32);
    convert_method!(u64, to_u64);
    convert_method!(usize, to_usize);

    /// Get the decimal multiple.
    fn multiple(&self) -> BigInt {
        let mut multiple: BigInt = 1.into();

        for _ in 0..self.decimal {
            let ten: BigInt = 10.into();
            multiple = multiple * ten;
        }

        multiple
    }

    /// Try to convert to bigint.
    pub fn to_bigint(&self) -> Option<&BigInt> {
        if self.decimal != 0 {
            return None;
        }

        Some(&self.digits)
    }

    pub fn to_f64(&self) -> Option<f64> {
        let multiple = self.multiple();
        let (base, decimal) = self.digits.div_mod_floor(&multiple);

        base.to_f64().and_then(|base| {
            decimal.to_f64().and_then(|decimal| {
                multiple
                    .to_f64()
                    .map(|multiple| base + (decimal / multiple))
            })
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

impl From<u64> for RpNumber {
    fn from(value: u64) -> RpNumber {
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

impl From<i64> for RpNumber {
    fn from(value: i64) -> RpNumber {
        RpNumber {
            digits: value.into(),
            decimal: 0usize,
        }
    }
}

impl From<BigInt> for RpNumber {
    fn from(value: BigInt) -> RpNumber {
        RpNumber {
            digits: value,
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
    fn serialize<S>(&self, serializer: S) -> result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let n = self.to_f64().unwrap();
        serializer.serialize_f64(n)
    }
}

#[cfg(test)]
mod test_numbers {
    use super::*;

    #[test]
    fn test_number() {
        let n = RpNumber {
            digits: 104321.into(),
            decimal: 2,
        };

        assert_eq!(Some(1043.21), n.to_f64());
        assert_eq!(Some(1043), n.to_u32());
        assert_eq!(Some(1043), n.to_u64());
        assert_eq!(Some(1043), n.to_i32());
        assert_eq!(Some(1043), n.to_i64());
    }

    #[test]
    fn test_negative() {
        let n = RpNumber {
            digits: (-104321).into(),
            decimal: 2,
        };

        assert_eq!(None, n.to_u64());
        assert_eq!(Some(-1043.21), n.to_f64());
        assert_eq!(Some(-1043), n.to_i32());
        assert_eq!(Some(-1043), n.to_i64());
        assert_eq!(None, n.to_u32());
        assert_eq!(None, n.to_u64());
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
