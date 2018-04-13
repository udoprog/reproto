//! Model for enums

use errors::Result;
use serde::Serialize;
use std::fmt;
use std::vec;
use {BigInt, Flavor, Loc, RpCode, RpNumber, RpReg, RpValue, Span, Translate, Translator};

decl_body!(pub struct RpEnumBody<F> {
    /// The type of the variant.
    pub enum_type: F::EnumType,
    /// Variants in the enum.
    pub variants: RpVariants<F>,
    /// Custom code blocks in the enum.
    pub codes: Vec<Loc<RpCode>>,
});

impl<F: 'static, T> Translate<T> for RpEnumBody<F>
where
    F: Flavor,
    T: Translator<Source = F>,
{
    type Source = F;
    type Out = RpEnumBody<T::Target>;

    /// Translate into different flavor.
    fn translate(self, translator: &T) -> Result<RpEnumBody<T::Target>> {
        translator.visit(&self.name)?;

        let name = translator.translate_local_name(RpReg::Enum, self.name)?;
        let enum_type = translator.translate_enum_type(self.enum_type)?;

        Ok(RpEnumBody {
            name: name,
            ident: self.ident,
            comment: self.comment,
            decls: self.decls.translate(translator)?,
            enum_type,
            variants: self.variants.translate(translator)?,
            codes: self.codes,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpVariantValue<'a> {
    String(&'a str),
    Number(&'a RpNumber),
}

impl<'a> From<&'a RpNumber> for RpVariantValue<'a> {
    fn from(value: &'a RpNumber) -> Self {
        RpVariantValue::Number(value)
    }
}

impl<'a> From<&'a String> for RpVariantValue<'a> {
    fn from(value: &'a String) -> Self {
        RpVariantValue::String(value.as_str())
    }
}

impl<'a> fmt::Display for RpVariantValue<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        use self::RpVariantValue::*;

        match *self {
            String(string) => write!(fmt, "{:?}", string),
            Number(value) => value.fmt(fmt),
        }
    }
}

/// A cheap, type-erasured variant that can be used for value comparisons.
///
/// This is typically created using `RpVariants::iter()`.
#[derive(Debug, Clone, Copy)]
pub struct RpVariantRef<'a, F: 'static>
where
    F: Flavor,
{
    pub span: Span,
    pub name: &'a F::Name,
    pub ident: &'a Loc<String>,
    pub comment: &'a Vec<String>,
    pub value: RpVariantValue<'a>,
}

impl<'a, F: 'static> RpVariantRef<'a, F>
where
    F: Flavor,
{
    /// Get the identifier for this variant.
    pub fn ident(&self) -> &'a str {
        self.ident.as_str()
    }
}

impl<'a, F: 'static> fmt::Display for RpVariantRef<'a, F>
where
    F: Flavor,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{} as {}", self.name, self.value)
    }
}

/// Variant in an enum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(bound = "F::Package: Serialize, F::Name: Serialize, V: Serialize")]
pub struct RpVariant<F: 'static, V>
where
    F: Flavor,
{
    pub name: F::Name,
    pub ident: Loc<String>,
    pub comment: Vec<String>,
    pub value: V,
}

impl<'a, F: 'static, V> RpVariant<F, V>
where
    F: Flavor,
{
    /// Get the identifier of the variant.
    pub fn ident(&self) -> &str {
        self.ident.as_str()
    }
}

impl<'a, F: 'static, V: 'a> RpVariant<F, V>
where
    F: Flavor,
    RpVariantValue<'a>: From<&'a V>,
{
    /// Convert into a variant value.
    pub fn value(&'a self) -> RpVariantValue<'a> {
        RpVariantValue::from(&self.value)
    }
}

impl<F: 'static, T, V> Translate<T> for RpVariant<F, V>
where
    F: Flavor,
    T: Translator<Source = F>,
{
    type Source = F;
    type Out = RpVariant<T::Target, V>;

    /// Translate into different flavor.
    fn translate(self, translator: &T) -> Result<RpVariant<T::Target, V>> {
        translator.visit(&self.name)?;

        let name = translator.translate_local_name(RpReg::EnumVariant, self.name)?;

        Ok(RpVariant {
            name: name,
            ident: self.ident,
            comment: self.comment,
            value: self.value,
        })
    }
}

/// Model for enum types
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum RpEnumType {
    String,
    U32,
    U64,
    I32,
    I64,
}

impl RpEnumType {
    pub fn is_assignable_from(&self, value: &RpValue) -> bool {
        use self::RpEnumType::*;

        match (self, value) {
            (&String, &RpValue::String(_)) => true,
            (&U32, &RpValue::Number(_)) => true,
            (&U64, &RpValue::Number(_)) => true,
            (&I32, &RpValue::Number(_)) => true,
            (&I64, &RpValue::Number(_)) => true,
            _ => false,
        }
    }

    /// Validate that the given number doesn't violate expected numeric bounds.
    pub fn validate_number(&self, number: &RpNumber) -> Result<()> {
        // max contiguous whole number that can be represented with a double: 2^53 - 1
        const MAX_SAFE_INTEGER: i64 = 9007199254740991i64;
        const MIN_SAFE_INTEGER: i64 = -9007199254740991i64;

        use self::RpEnumType::*;

        let (mn, mx): (BigInt, BigInt) = match *self {
            String => return Err("expected number, got `string`".into()),
            U32 => (0u32.into(), i32::max_value().into()),
            U64 => (0u64.into(), MAX_SAFE_INTEGER.into()),
            I32 => (i32::min_value().into(), i32::max_value().into()),
            I64 => (MIN_SAFE_INTEGER.into(), MAX_SAFE_INTEGER.into()),
        };

        let n = number.to_bigint().ok_or_else(|| "not a whole number")?;

        // withing bounds
        if &mn <= n && n <= &mx {
            return Ok(());
        }

        return Err(format!("number is not within {} to {} (inclusive)", mn, mx).into());
    }
}

impl fmt::Display for RpEnumType {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        use self::RpEnumType::*;

        match *self {
            String => "string".fmt(fmt),
            U32 => "u32".fmt(fmt),
            U64 => "u64".fmt(fmt),
            I32 => "i32".fmt(fmt),
            I64 => "i64".fmt(fmt),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(bound = "F: Serialize, F::Package: Serialize, F::Name: Serialize")]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RpVariants<F: 'static>
where
    F: Flavor,
{
    String {
        variants: Vec<Loc<RpVariant<F, String>>>,
    },
    Number {
        variants: Vec<Loc<RpVariant<F, RpNumber>>>,
    },
}

pub struct RpVariantsIter<'a, F: 'static>
where
    F: Flavor,
{
    iter: vec::IntoIter<RpVariantRef<'a, F>>,
}

impl<'a, F: 'static> Iterator for RpVariantsIter<'a, F>
where
    F: Flavor,
{
    type Item = RpVariantRef<'a, F>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl<F: 'static> RpVariants<F>
where
    F: Flavor,
{
    /// Iterate over all variants in a type-erasured manner.
    pub fn iter(&self) -> RpVariantsIter<F> {
        use self::RpVariants::*;

        macro_rules! variants {
            ($slf:ident, $($ty:ident),*) => {
                match *$slf {
                $(
                $ty { ref variants } => {
                    let mut __o = Vec::new();

                    for v in variants {
                        let (value, span) = Loc::borrow_pair(v);

                        __o.push(RpVariantRef {
                            span: span,
                            name: &value.name,
                            ident: &value.ident,
                            comment: &value.comment,
                            value: RpVariantValue::from(&value.value),
                        })
                    }

                    __o
                },
                )*
                }
            };
        }

        let variants: Vec<_> = variants!(self, String, Number);

        RpVariantsIter {
            iter: variants.into_iter(),
        }
    }
}

impl<F: 'static, T> Translate<T> for RpVariants<F>
where
    F: Flavor,
    T: Translator<Source = F>,
{
    type Source = F;
    type Out = RpVariants<T::Target>;

    /// Translate into different flavor.
    fn translate(self, translator: &T) -> Result<RpVariants<T::Target>> {
        use self::RpVariants::*;

        let out = match self {
            String { variants } => String {
                variants: variants.translate(translator)?,
            },
            Number { variants } => Number {
                variants: variants.translate(translator)?,
            },
        };

        Ok(out)
    }
}

impl<'a, F: 'static> IntoIterator for &'a RpVariants<F>
where
    F: Flavor,
{
    type Item = RpVariantRef<'a, F>;
    type IntoIter = RpVariantsIter<'a, F>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
