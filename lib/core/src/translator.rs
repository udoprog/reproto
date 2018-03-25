//! Translates one IR in-place into another.

use Flavor;
use errors::Result;
use linked_hash_map::LinkedHashMap;
use std::cell::RefCell;
use std::rc::Rc;
use {CoreFlavor, Loc, RpEndpoint, RpField, RpName, RpReg, RpType};

pub trait TypeTranslator {
    type Source: 'static + Clone + Flavor;
    type Target: 'static + Clone + Flavor;

    fn translate_i32(&self) -> Result<<Self::Target as Flavor>::Type>;

    fn translate_i64(&self) -> Result<<Self::Target as Flavor>::Type>;

    fn translate_u32(&self) -> Result<<Self::Target as Flavor>::Type>;

    fn translate_u64(&self) -> Result<<Self::Target as Flavor>::Type>;

    fn translate_float(&self) -> Result<<Self::Target as Flavor>::Type>;

    fn translate_double(&self) -> Result<<Self::Target as Flavor>::Type>;

    fn translate_boolean(&self) -> Result<<Self::Target as Flavor>::Type>;

    fn translate_string(&self) -> Result<<Self::Target as Flavor>::Type>;

    fn translate_datetime(&self) -> Result<<Self::Target as Flavor>::Type>;

    fn translate_array(
        &self,
        _: <Self::Target as Flavor>::Type,
    ) -> Result<<Self::Target as Flavor>::Type>;

    fn translate_map(
        &self,
        _: <Self::Target as Flavor>::Type,
        _: <Self::Target as Flavor>::Type,
    ) -> Result<<Self::Target as Flavor>::Type>;

    fn translate_any(&self) -> Result<<Self::Target as Flavor>::Type>;

    fn translate_bytes(&self) -> Result<<Self::Target as Flavor>::Type>;

    /// Translate the given name.
    fn translate_name(&self, name: RpName, reg: RpReg) -> Result<<Self::Target as Flavor>::Type>;

    /// Translate the given field.
    fn translate_field<T>(
        &self,
        translator: &T,
        field: <Self::Source as Flavor>::Field,
    ) -> Result<<Self::Target as Flavor>::Field>
    where
        T: Translator<Source = Self::Source, Target = Self::Target>;

    /// Translate the given endpoint.
    fn translate_endpoint<T>(
        &self,
        translator: &T,
        endpoint: <Self::Source as Flavor>::Endpoint,
    ) -> Result<<Self::Target as Flavor>::Endpoint>
    where
        T: Translator<Source = Self::Source, Target = Self::Target>;
}

pub struct CoreTypeTranslator;

impl TypeTranslator for CoreTypeTranslator {
    type Source = CoreFlavor;
    type Target = CoreFlavor;

    fn translate_i32(&self) -> Result<RpType> {
        Ok(RpType::Signed { size: 32 })
    }

    fn translate_i64(&self) -> Result<RpType> {
        Ok(RpType::Signed { size: 64 })
    }

    fn translate_u32(&self) -> Result<RpType> {
        Ok(RpType::Unsigned { size: 32 })
    }

    fn translate_u64(&self) -> Result<RpType> {
        Ok(RpType::Unsigned { size: 64 })
    }

    fn translate_float(&self) -> Result<RpType> {
        Ok(RpType::Float)
    }

    fn translate_double(&self) -> Result<RpType> {
        Ok(RpType::Double)
    }

    fn translate_boolean(&self) -> Result<RpType> {
        Ok(RpType::Boolean)
    }

    fn translate_string(&self) -> Result<RpType> {
        Ok(RpType::String)
    }

    fn translate_datetime(&self) -> Result<RpType> {
        Ok(RpType::DateTime)
    }

    fn translate_array(&self, inner: RpType) -> Result<RpType> {
        Ok(RpType::Array {
            inner: Box::new(inner),
        })
    }

    fn translate_map(&self, key: RpType, value: RpType) -> Result<RpType> {
        Ok(RpType::Map {
            key: Box::new(key),
            value: Box::new(value),
        })
    }

    fn translate_any(&self) -> Result<RpType> {
        Ok(RpType::Any)
    }

    fn translate_bytes(&self) -> Result<RpType> {
        Ok(RpType::Bytes)
    }

    fn translate_name(&self, name: RpName, _reg: RpReg) -> Result<<Self::Target as Flavor>::Type> {
        Ok(RpType::Name { name })
    }

    fn translate_field<T>(
        &self,
        _translator: &T,
        field: RpField<CoreFlavor>,
    ) -> Result<RpField<CoreFlavor>>
    where
        T: Translator<Source = CoreFlavor, Target = CoreFlavor>,
    {
        Ok(field)
    }

    fn translate_endpoint<T>(
        &self,
        translator: &T,
        endpoint: RpEndpoint<CoreFlavor>,
    ) -> Result<RpEndpoint<CoreFlavor>>
    where
        T: Translator<Source = CoreFlavor, Target = CoreFlavor>,
    {
        Ok(RpEndpoint {
            ident: endpoint.ident,
            safe_ident: endpoint.safe_ident,
            name: endpoint.name,
            comment: endpoint.comment,
            attributes: endpoint.attributes,
            arguments: endpoint.arguments.translate(translator)?,
            request: endpoint.request.translate(translator)?,
            response: endpoint.response.translate(translator)?,
            http: endpoint.http.translate(translator)?,
        })
    }
}

/// Translator trait from one flavor to another.
pub trait Translator {
    type Source: 'static + Flavor;
    type Target: 'static + Clone + Flavor;

    /// Indicate that the given name has been visited.
    fn visit(&self, _: &RpName) -> Result<()> {
        Ok(())
    }

    /// Translate the given type from one flavor to another.
    fn translate_type(
        &self,
        <Self::Source as Flavor>::Type,
    ) -> Result<<Self::Target as Flavor>::Type>;

    /// Translate the given field from one flavor to another.
    fn translate_field(
        &self,
        <Self::Source as Flavor>::Field,
    ) -> Result<<Self::Target as Flavor>::Field>;

    /// Translate the given endpoint from one flavor to another.
    fn translate_endpoint(
        &self,
        <Self::Source as Flavor>::Endpoint,
    ) -> Result<<Self::Target as Flavor>::Endpoint>;
}

/// A translated type.
pub trait Translate<T>
where
    T: Translator<Source = Self::Source>,
{
    type Source: 'static + Flavor;
    type Out;

    /// Translate into different flavor.
    fn translate(self, translator: &T) -> Result<Self::Out>;
}

impl<T, I> Translate<T> for Loc<I>
where
    I: Translate<T>,
    T: Translator<Source = I::Source>,
{
    type Source = I::Source;
    type Out = Loc<I::Out>;

    /// Translate into different flavor.
    fn translate(self, translator: &T) -> Result<Loc<I::Out>> {
        Loc::and_then(self, |s| s.translate(translator))
    }
}

impl<T, I> Translate<T> for Vec<I>
where
    I: Translate<T>,
    T: Translator<Source = I::Source>,
{
    type Source = I::Source;
    type Out = Vec<I::Out>;

    /// Translate into different flavor.
    fn translate(self, translator: &T) -> Result<Vec<I::Out>> {
        self.into_iter()
            .map(|v| v.translate(translator))
            .collect::<Result<Vec<_>>>()
    }
}

impl<T, I> Translate<T> for Option<I>
where
    I: Translate<T>,
    T: Translator<Source = I::Source>,
{
    type Source = I::Source;
    type Out = Option<I::Out>;

    /// Translate into different flavor.
    fn translate(self, translator: &T) -> Result<Option<I::Out>> {
        let out = match self {
            Some(inner) => Some(inner.translate(translator)?),
            None => None,
        };

        Ok(out)
    }
}

pub struct Fields<F>(pub Vec<Loc<F>>);

impl<T, F: 'static> Translate<T> for Fields<F::Field>
where
    F: Flavor,
    T: Translator<Source = F>,
{
    type Source = F;
    type Out = Vec<Loc<<T::Target as Flavor>::Field>>;

    /// Translate into different flavor.
    fn translate(self, translator: &T) -> Result<Self::Out> {
        let out = self.0
            .into_iter()
            .map(|f| Loc::and_then(f, |f| translator.translate_field(f)))
            .collect::<Result<Vec<_>>>()?;

        Ok(out)
    }
}

/// Context used when translating.
pub struct Context<T> {
    /// Type used to translate types.
    pub type_translator: T,
    /// Registered declarations of the source type.
    pub types: Rc<LinkedHashMap<RpName, RpReg>>,
    /// Cached and translated registered declarations.
    pub decls: RefCell<LinkedHashMap<RpName, RpReg>>,
}

impl<T> Context<T> {
    /// Lookup and cause the given name to be registered.
    fn lookup(&self, key: &RpName) -> Result<RpReg> {
        let key = key.clone().without_prefix();

        let mut decls = self.decls.try_borrow_mut()?;

        match decls.get(&key) {
            Some(reg) => return Ok(reg.clone()),
            None => {}
        }

        let reg = match self.types.get(&key) {
            Some(reg) => reg.clone(),
            None => {
                return Err(format!("no such type: {}", key).into());
            }
        };

        let reg = decls.entry(key).or_insert(reg);
        return Ok(reg.clone());
    }
}

impl<T> Translator for Context<T>
where
    T: TypeTranslator<Source = CoreFlavor>,
{
    type Source = CoreFlavor;
    type Target = T::Target;

    /// Indicate that the given name has been visited.
    fn visit(&self, name: &RpName) -> Result<()> {
        self.lookup(name)?;
        Ok(())
    }

    fn translate_type(
        &self,
        source: <Self::Source as Flavor>::Type,
    ) -> Result<<Self::Target as Flavor>::Type> {
        use self::RpType::*;

        let out = match source {
            String => self.type_translator.translate_string()?,
            DateTime => self.type_translator.translate_datetime()?,
            Bytes => self.type_translator.translate_bytes()?,
            Signed { size: 32 } => self.type_translator.translate_i32()?,
            Signed { size: 64 } => self.type_translator.translate_i64()?,
            Unsigned { size: 32 } => self.type_translator.translate_u32()?,
            Unsigned { size: 64 } => self.type_translator.translate_u64()?,
            Float => self.type_translator.translate_float()?,
            Double => self.type_translator.translate_double()?,
            Boolean => self.type_translator.translate_boolean()?,
            Array { inner } => {
                let inner = self.translate_type(*inner)?;
                self.type_translator.translate_array(inner)?
            }
            Name { name } => {
                let reg = self.lookup(&name)?;
                self.type_translator.translate_name(name, reg)?
            }
            Map { key, value } => {
                let key = self.translate_type(*key)?;
                let value = self.translate_type(*value)?;
                self.type_translator.translate_map(key, value)?
            }
            Any => self.type_translator.translate_any()?,
            ty => return Err(format!("unsupported type: {}", ty).into()),
        };

        Ok(out)
    }

    fn translate_field(
        &self,
        source: <Self::Source as Flavor>::Field,
    ) -> Result<<Self::Target as Flavor>::Field> {
        self.type_translator.translate_field(self, source)
    }

    fn translate_endpoint(
        &self,
        source: <Self::Source as Flavor>::Endpoint,
    ) -> Result<<Self::Target as Flavor>::Endpoint> {
        self.type_translator.translate_endpoint(self, source)
    }
}
