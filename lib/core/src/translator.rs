//! Translates one IR in-place into another.

use Flavor;
use errors::Result;
use linked_hash_map::LinkedHashMap;
use std::cell::RefCell;
use std::rc::Rc;
use {CoreFlavor, Loc, RpEndpoint, RpEnumType, RpField, RpName, RpReg, RpType, RpVersionedPackage};

/// Method for translating package.
pub trait PackageTranslator<K, V> {
    /// Translate the given package.
    fn translate_package(&self, source: K) -> Result<V>;
}

pub trait FlavorTranslator {
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

    /// Translate the given package.
    fn translate_package(
        &self,
        package: <Self::Source as Flavor>::Package,
    ) -> Result<<Self::Target as Flavor>::Package>;

    /// Translate the given name.
    fn translate_name(
        &self,
        reg: RpReg,
        name: RpName<Self::Target>,
    ) -> Result<<Self::Target as Flavor>::Type>;

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

    /// Translate a local declaration name.
    fn translate_local_name<T>(
        &self,
        translator: &T,
        reg: RpReg,
        name: <Self::Source as Flavor>::Name,
    ) -> Result<<Self::Target as Flavor>::Name>
    where
        T: Translator<Source = Self::Source, Target = Self::Target>;

    /// Enum type to translate.
    fn translate_enum_type<T>(
        &self,
        translator: &T,
        enum_type: <Self::Source as Flavor>::EnumType,
    ) -> Result<<Self::Target as Flavor>::EnumType>
    where
        T: Translator<Source = Self::Source, Target = Self::Target>;
}

impl PackageTranslator<RpVersionedPackage, RpVersionedPackage> for () {
    fn translate_package(&self, package: RpVersionedPackage) -> Result<RpVersionedPackage> {
        Ok(package)
    }
}

pub struct CoreFlavorTranslator<P, F> {
    package_translator: P,
    flavor: ::std::marker::PhantomData<F>,
}

impl<P, F> CoreFlavorTranslator<P, F> {
    pub fn new(package_translator: P) -> Self {
        Self {
            package_translator,
            flavor: ::std::marker::PhantomData,
        }
    }
}

impl<P: 'static, F: 'static> FlavorTranslator for CoreFlavorTranslator<P, F>
where
    P: PackageTranslator<RpVersionedPackage, F::Package>,
    F: Flavor<
        Type = RpType<F>,
        Field = RpField<F>,
        Endpoint = RpEndpoint<F>,
        Name = RpName<F>,
        EnumType = RpEnumType,
    >,
{
    type Source = CoreFlavor;
    type Target = F;

    translator_defaults!(Self, rp_type, local_name, field, endpoint, enum_type);

    fn translate_package(
        &self,
        package: <Self::Source as Flavor>::Package,
    ) -> Result<<F as Flavor>::Package> {
        self.package_translator.translate_package(package)
    }
}

/// Translator trait from one flavor to another.
pub trait Translator {
    type Source: 'static + Flavor;
    type Target: 'static + Flavor;

    /// Indicate that the given name has been visited.
    fn visit(&self, _: &<Self::Source as Flavor>::Name) -> Result<()> {
        Ok(())
    }

    /// Translate the given package from one flavor to another.
    fn translate_package(
        &self,
        <Self::Source as Flavor>::Package,
    ) -> Result<<Self::Target as Flavor>::Package>;

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

    /// Translate a local declaration name.
    fn translate_local_name(
        &self,
        reg: RpReg,
        name: <Self::Source as Flavor>::Name,
    ) -> Result<<Self::Target as Flavor>::Name>;

    /// Enum type to translate.
    fn translate_enum_type(
        &self,
        enum_type: <Self::Source as Flavor>::EnumType,
    ) -> Result<<Self::Target as Flavor>::EnumType>;
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
pub struct Context<T>
where
    T: FlavorTranslator<Source = CoreFlavor>,
{
    /// Type used to translate types.
    pub flavor: T,
    /// Registered declarations of the source type.
    pub types: Rc<LinkedHashMap<RpName<T::Source>, RpReg>>,
    /// Cached and translated registered declarations.
    pub decls: Option<RefCell<LinkedHashMap<RpName<T::Source>, RpReg>>>,
}

impl<T> Context<T>
where
    T: FlavorTranslator<Source = CoreFlavor>,
{
    /// Lookup and cause the given name to be registered.
    fn lookup(&self, key: &RpName<T::Source>) -> Result<RpReg> {
        let key = key.clone().without_prefix();

        let decls = self.decls.as_ref().ok_or_else(|| "no declarations")?;
        let mut decls = decls.try_borrow_mut()?;

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
    T: FlavorTranslator<Source = CoreFlavor>,
{
    type Source = T::Source;
    type Target = T::Target;

    /// Indicate that the given name has been visited.
    fn visit(&self, name: &RpName<Self::Source>) -> Result<()> {
        self.lookup(name)?;
        Ok(())
    }

    fn translate_package(
        &self,
        source: <Self::Source as Flavor>::Package,
    ) -> Result<<Self::Target as Flavor>::Package> {
        self.flavor.translate_package(source)
    }

    fn translate_type(
        &self,
        source: <Self::Source as Flavor>::Type,
    ) -> Result<<Self::Target as Flavor>::Type> {
        use self::RpType::*;

        let out = match source {
            String => self.flavor.translate_string()?,
            DateTime => self.flavor.translate_datetime()?,
            Bytes => self.flavor.translate_bytes()?,
            Signed { size: 32 } => self.flavor.translate_i32()?,
            Signed { size: 64 } => self.flavor.translate_i64()?,
            Unsigned { size: 32 } => self.flavor.translate_u32()?,
            Unsigned { size: 64 } => self.flavor.translate_u64()?,
            Float => self.flavor.translate_float()?,
            Double => self.flavor.translate_double()?,
            Boolean => self.flavor.translate_boolean()?,
            Array { inner } => {
                let inner = self.translate_type(*inner)?;
                self.flavor.translate_array(inner)?
            }
            Name { name } => {
                let reg = self.lookup(&name)?;
                let name = name.translate(self)?;
                self.flavor.translate_name(reg, name)?
            }
            Map { key, value } => {
                let key = self.translate_type(*key)?;
                let value = self.translate_type(*value)?;
                self.flavor.translate_map(key, value)?
            }
            Any => self.flavor.translate_any()?,
            ty => return Err(format!("unsupported type: {}", ty).into()),
        };

        Ok(out)
    }

    fn translate_field(
        &self,
        source: <Self::Source as Flavor>::Field,
    ) -> Result<<Self::Target as Flavor>::Field> {
        self.flavor.translate_field(self, source)
    }

    fn translate_endpoint(
        &self,
        source: <Self::Source as Flavor>::Endpoint,
    ) -> Result<<Self::Target as Flavor>::Endpoint> {
        self.flavor.translate_endpoint(self, source)
    }

    /// Translate a local declaration name.
    fn translate_local_name(
        &self,
        reg: RpReg,
        name: <Self::Source as Flavor>::Name,
    ) -> Result<<Self::Target as Flavor>::Name> {
        self.flavor.translate_local_name(self, reg, name)
    }

    /// Translate enum type.
    fn translate_enum_type(
        &self,
        enum_type: <Self::Source as Flavor>::EnumType,
    ) -> Result<<Self::Target as Flavor>::EnumType> {
        self.flavor.translate_enum_type(self, enum_type)
    }
}
