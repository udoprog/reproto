use ast::{self, UseDecl};
use core::errors::{Error, Result};
use core::{translator, Context, CoreFlavor, Flavor, Loc, Object, PathObject, Range, Resolved,
           Resolver, RpFile, RpName, RpPackage, RpReg, RpRequiredPackage, RpVersionedPackage,
           Translate, TypeTranslator, WithPos};
use into_model::IntoModel;
use linked_hash_map::LinkedHashMap;
use naming::{self, Naming};
use parser;
use scope::Scope;
use std::cell::RefCell;
use std::collections::{btree_map, BTreeMap, HashMap};
use std::path::Path;
use std::rc::Rc;
use translated::Translated;

/// Scoped environment for evaluating reproto IDLs.
pub struct Environment<F: 'static>
where
    F: Flavor,
{
    /// Global context for collecting errors.
    ctx: Rc<Context>,
    /// Global package prefix.
    package_prefix: Option<RpPackage>,
    /// Index resolver to use.
    resolver: Box<Resolver>,
    /// Store required packages, to avoid unnecessary lookups.
    visited: HashMap<RpRequiredPackage, Option<RpVersionedPackage>>,
    /// Files and associated declarations.
    files: BTreeMap<RpVersionedPackage, RpFile<F>>,
    /// Registered types.
    types: Rc<LinkedHashMap<RpName, RpReg>>,
    /// Keywords that need to be translated.
    keywords: Rc<HashMap<String, String>>,
    /// Whether to perform package translation or not.
    safe_packages: bool,
    /// Package naming to apply.
    package_naming: Option<Box<Naming>>,
    /// Field naming to apply.
    field_ident_naming: Option<Box<Naming>>,
}

/// Environment containing all loaded declarations.
impl<F: 'static> Environment<F>
where
    F: Flavor,
{
    /// Construct a new, language-neutral environment.
    pub fn new(
        ctx: Rc<Context>,
        package_prefix: Option<RpPackage>,
        resolver: Box<Resolver>,
    ) -> Environment<F> {
        Environment {
            ctx: ctx,
            package_prefix: package_prefix,
            resolver: resolver,
            visited: HashMap::new(),
            files: BTreeMap::new(),
            types: Rc::new(LinkedHashMap::new()),
            keywords: Rc::new(HashMap::new()),
            safe_packages: false,
            package_naming: None,
            field_ident_naming: None,
        }
    }

    /// Configure a new environment on how to use safe packages or not.
    pub fn with_safe_packages(self, safe_packages: bool) -> Self {
        Self {
            safe_packages: safe_packages,
            ..self
        }
    }

    /// Build the environment with the given keywords.
    pub fn with_keywords(self, keywords: HashMap<String, String>) -> Self {
        Self {
            keywords: Rc::new(keywords),
            ..self
        }
    }

    /// Set package naming policy.
    pub fn with_package_naming(self, package_naming: Box<Naming>) -> Self {
        Self {
            package_naming: Some(package_naming),
            ..self
        }
    }

    /// Set field naming policy.
    pub fn with_field_ident_naming(self, field_ident_naming: Box<Naming>) -> Self {
        Self {
            field_ident_naming: Some(field_ident_naming),
            ..self
        }
    }
}

impl Environment<CoreFlavor> {
    /// Build a new translator.
    pub fn translator<T: 'static>(&self, type_translator: T) -> translator::Context<T>
    where
        T: TypeTranslator<Source = CoreFlavor>,
    {
        translator::Context {
            type_translator: type_translator,
            types: Rc::clone(&self.types),
            decls: RefCell::new(LinkedHashMap::new()),
        }
    }

    /// Translate the current environment into another.
    pub fn translate<T: 'static>(self, ctx: translator::Context<T>) -> Result<Translated<T::Target>>
    where
        T: TypeTranslator<Source = CoreFlavor>,
    {
        let mut files = BTreeMap::new();

        for (package, file) in self.files {
            files.insert(package, file.translate(&ctx)?);
        }

        let types = ctx.decls.into_inner();
        Ok(Translated::new(types, files))
    }

    /// Translate without changing the flavor.
    pub fn translate_default(self) -> Result<Translated<CoreFlavor>> {
        let ctx = self.translator(translator::CoreTypeTranslator);
        return self.translate(ctx);
    }

    /// Import a path into the environment.
    pub fn import_path<P: AsRef<Path>>(
        &mut self,
        path: P,
        package: Option<RpVersionedPackage>,
    ) -> Result<RpVersionedPackage> {
        self.import_object(&PathObject::new(None, path), package)
    }

    /// Import an object into the environment.
    pub fn import_object(
        &mut self,
        object: &Object,
        package: Option<RpVersionedPackage>,
    ) -> Result<RpVersionedPackage> {
        let package = package.unwrap_or_else(|| RpVersionedPackage::new(RpPackage::empty(), None));
        let required = RpRequiredPackage::new(package.package.clone(), Range::any());

        if !self.visited.contains_key(&required) {
            let file = self.load_object(object, &package)?;
            self.process_file(package.clone(), file)?;
            self.visited.insert(required, Some(package.clone()));
        }

        Ok(package)
    }

    /// Import a single, structured file object.
    pub fn import_file(
        &mut self,
        file: ast::File,
        package: Option<RpVersionedPackage>,
    ) -> Result<RpVersionedPackage> {
        let package = package.unwrap_or_else(|| RpVersionedPackage::new(RpPackage::empty(), None));
        let required = RpRequiredPackage::new(package.package.clone(), Range::any());

        if !self.visited.contains_key(&required) {
            let file = self.load_file(file, &package)?;
            self.process_file(package.clone(), file)?;
            self.visited.insert(required, Some(package.clone()));
        }

        Ok(package)
    }

    /// Import a package based on a package and version criteria.
    pub fn import(&mut self, required: &RpRequiredPackage) -> Result<Option<RpVersionedPackage>> {
        debug!("import: {}", required);

        if let Some(existing) = self.visited.get(required) {
            debug!("already loaded: {:?} ({})", existing, required);
            return Ok(existing.as_ref().cloned());
        }

        let mut candidates = BTreeMap::new();

        // find all matching objects from the resolver.
        let files = self.resolver.resolve(required)?;

        if let Some(Resolved { version, object }) = files.into_iter().last() {
            debug!("loading: {}", object);

            let package = RpVersionedPackage::new(required.package.clone(), version);
            let file = self.load_object(object.as_ref(), &package)?;

            candidates
                .entry(package)
                .or_insert_with(Vec::new)
                .push(file);
        }

        let result = if let Some((versioned, files)) = candidates.into_iter().last() {
            debug!("found: {} ({})", versioned, required);

            for file in files.into_iter() {
                self.process_file(versioned.clone(), file)?;
            }

            Some(versioned)
        } else {
            None
        };

        self.visited.insert(required.clone(), result.clone());
        Ok(result)
    }

    /// Verify all declarations.
    pub fn verify(&mut self) -> Result<()> {
        Ok(())
    }

    /// Parse a naming option.
    ///
    /// Since lower_camel is default, do nothing on that case.
    fn parse_naming(&self, naming: &str) -> Result<Option<Box<Naming>>> {
        let result: Option<Box<Naming>> = match naming {
            "upper_camel" => Some(Box::new(naming::to_upper_camel())),
            "lower_camel" => Some(Box::new(naming::to_lower_camel())),
            "upper_snake" => Some(Box::new(naming::to_upper_snake())),
            "lower_snake" => None,
            _ => return Err("illegal value".into()),
        };

        Ok(result)
    }

    /// Load the provided Object into an `RpFile` without registering it to the set of visited
    /// files.
    pub fn load_object(
        &mut self,
        object: &Object,
        package: &RpVersionedPackage,
    ) -> Result<RpFile<CoreFlavor>> {
        let object = Rc::new(object.clone_object());
        let input = parser::read_to_string(object.read()?)?;
        let file = parser::parse(object, input.as_str())?;
        self.load_file(file, package)
    }

    /// Loads the given file, without registering it to the set of visited packages.
    fn load_file(
        &mut self,
        mut file: ast::File,
        package: &RpVersionedPackage,
    ) -> Result<RpFile<CoreFlavor>> {
        let prefixes = self.process_uses(&file.uses)?;

        // TODO: support through file attributes.

        let package_prefix = self.package_prefix.clone();
        let package = package.clone();

        let mut scope = Scope::new(
            self.ctx.clone(),
            package_prefix,
            package,
            prefixes,
            self.keywords.clone(),
            self.safe_packages,
            self.package_naming.as_ref().map(|n| n.copy()),
            self.field_ident_naming.as_ref().map(|n| n.copy()),
        );

        let attributes = file.attributes.drain(..).collect::<Vec<_>>();
        let mut attributes = attributes.into_model(&scope)?;

        {
            let root = scope.mut_root()?;

            if let Some(endpoint_naming) = attributes.take_selection("endpoint_naming") {
                let (mut endpoint_naming, pos) = Loc::take_pair(endpoint_naming);

                root.endpoint_naming = endpoint_naming
                    .take_word()
                    .ok_or_else(|| Error::from("expected argument"))
                    .and_then(|n| n.as_identifier().and_then(|n| self.parse_naming(n)))
                    .with_pos(&pos)?;

                check_selection!(&self.ctx, endpoint_naming);
            }

            if let Some(field_naming) = attributes.take_selection("field_naming") {
                let (mut field_naming, pos) = Loc::take_pair(field_naming);

                root.field_naming = field_naming
                    .take_word()
                    .ok_or_else(|| Error::from("expected argument"))
                    .and_then(|n| n.as_identifier().and_then(|n| self.parse_naming(n)))
                    .with_pos(&pos)?;

                check_selection!(&self.ctx, field_naming);
            }

            check_attributes!(&self.ctx, attributes);
        }

        Ok(file.into_model(&scope)?)
    }

    /// Apply global package prefix.
    fn package_prefix(&self, package: &RpVersionedPackage) -> RpVersionedPackage {
        self.package_prefix
            .as_ref()
            .map(|prefix| prefix.join_versioned(package))
            .unwrap_or_else(|| package.clone())
    }

    /// Parse the given version requirement.
    fn parse_range(v: &Loc<String>) -> Result<Range> {
        let (value, pos) = Loc::borrow_pair(v);

        Range::parse(value)
            .map_err(|e| format!("bad version requirement: {}", e).into())
            .with_pos(pos)
    }

    /// Process use declarations found at the top of each object.
    fn process_uses(
        &mut self,
        uses: &[Loc<UseDecl>],
    ) -> Result<HashMap<String, RpVersionedPackage>> {
        use std::collections::hash_map::Entry;

        let mut prefixes = HashMap::new();

        for use_decl in uses {
            let package = Loc::value(&use_decl.package).clone();

            let range = use_decl
                .range
                .as_ref()
                .map(Self::parse_range)
                .unwrap_or_else(|| Ok(Range::any()))?;

            let required = RpRequiredPackage::new(package, range);

            let use_package = self.import(&required)?;

            if let Some(use_package) = use_package {
                let use_package = self.package_prefix(&use_package);

                if let Some(used) = use_decl.package.parts.iter().last() {
                    let alias = use_decl.alias.as_ref().map(|v| v.as_ref()).unwrap_or(used);

                    match prefixes.entry(alias.to_owned()) {
                        Entry::Vacant(entry) => entry.insert(use_package.clone()),
                        Entry::Occupied(_) => {
                            return Err(format!("alias {} already in use", alias).into())
                        }
                    };
                }

                continue;
            }

            return Err(Error::new(format!("no package found: {}", required)).with_pos(Loc::pos(use_decl)));
        }

        Ok(prefixes)
    }

    /// Process a single file, populating the environment.
    fn process_file(
        &mut self,
        package: RpVersionedPackage,
        file: RpFile<CoreFlavor>,
    ) -> Result<()> {
        use linked_hash_map::Entry::*;

        let new_package = package.clone().with_replacements(&self.keywords);

        let file = match self.files.entry(new_package) {
            btree_map::Entry::Vacant(entry) => entry.insert(file),
            btree_map::Entry::Occupied(_) => {
                return Err(format!("package already registered: {}", package).into());
            }
        };

        for (key, pos, t) in file.decls.iter().flat_map(|d| d.to_reg()) {
            let key = key.clone().without_prefix();

            debug!("new reg ty: {}", key);

            let types = Rc::get_mut(&mut self.types).ok_or_else(|| "types registry in use")?;

            match types.entry(key) {
                Vacant(entry) => entry.insert(t),
                Occupied(_) => {
                    return Err(self.ctx.report().err(pos, "conflicting declaration").into());
                }
            };
        }

        Ok(())
    }
}
