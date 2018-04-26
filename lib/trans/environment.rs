use ast;
use core::errors::{Error, Result};
use core::{translator, Context, CoreFlavor, Diagnostics, Flavor, FlavorTranslator, Import, Loc,
           PackageTranslator, Range, Resolved, Resolver, RpFile, RpName, RpPackage, RpReg,
           RpRequiredPackage, RpVersionedPackage, Source, Translate, Translator, Version};
use into_model::IntoModel;
use linked_hash_map::LinkedHashMap;
use naming::Naming;
use parser;
use scope::Scope;
use std::cell::RefCell;
use std::collections::{btree_map, BTreeMap, HashMap};
use std::path::Path;
use std::rc::Rc;
use std::result;
use translated::Translated;

/// Try the given expression, and associated diagnostics with context if an error occurred.
macro_rules! try_with_diag {
    ($ctx:expr, $diag:expr, $block:block) => {
        match $block {
            Err(()) => {
                $ctx.diagnostics($diag)?;
                return Err("error in environment".into());
            }
            Ok(ok) => {
                if $diag.has_errors() {
                    $ctx.diagnostics($diag)?;
                    return Err("error in environment".into());
                }

                ok
            }
        }
    };
}

#[derive(Clone, Debug)]
pub struct File<F: 'static>
where
    F: Flavor,
{
    file: RpFile<F>,
    source: Source,
}

/// Scoped environment for evaluating reproto IDLs.
pub struct Environment<'a, F: 'static>
where
    F: Flavor,
{
    /// Global context for collecting errors.
    pub ctx: Rc<Context>,
    /// Global package prefix.
    package_prefix: Option<RpPackage>,
    /// Index resolver to use.
    resolver: &'a mut Resolver,
    /// Store required packages, to avoid unnecessary lookups.
    visited: HashMap<RpRequiredPackage, Option<RpVersionedPackage>>,
    /// Files and associated declarations.
    files: BTreeMap<RpVersionedPackage, File<F>>,
    /// Registered types.
    types: Rc<LinkedHashMap<RpName<F>, Loc<RpReg>>>,
    /// Keywords that need to be translated.
    keywords: Rc<HashMap<String, String>>,
    /// Whether to use safe packages or not.
    safe_packages: bool,
    /// Package naming to apply.
    package_naming: Option<Rc<Box<Naming>>>,
    /// Field naming to apply.
    field_ident_naming: Option<Box<Naming>>,
    /// Endpoint ident naming to apply.
    endpoint_ident_naming: Option<Box<Naming>>,
    /// Hook to provide to paths that were loaded.
    path_hook: Option<Box<Fn(&Path) -> Result<()>>>,
}

/// Environment containing all loaded declarations.
impl<'a, F: 'static> Environment<'a, F>
where
    F: Flavor,
{
    /// Construct a new, language-neutral environment.
    pub fn new(
        ctx: Rc<Context>,
        package_prefix: Option<RpPackage>,
        resolver: &'a mut Resolver,
    ) -> Environment<'a, F> {
        Environment {
            ctx: ctx,
            package_prefix,
            resolver,
            visited: HashMap::new(),
            files: BTreeMap::new(),
            types: Rc::new(LinkedHashMap::new()),
            keywords: Rc::new(HashMap::new()),
            safe_packages: false,
            package_naming: None,
            field_ident_naming: None,
            endpoint_ident_naming: None,
            path_hook: None,
        }
    }

    /// Setup a new path hook for this environment.
    pub fn with_path_hook<H: 'static>(self, path_hook: H) -> Self
    where
        H: Fn(&Path) -> Result<()>,
    {
        Self {
            path_hook: Some(Box::new(path_hook)),
            ..self
        }
    }

    /// Configure a new environment on how to use safe packages or not.
    pub fn with_safe_packages(self, safe_packages: bool) -> Self {
        Self {
            safe_packages,
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
            package_naming: Some(Rc::new(package_naming)),
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

    /// Set endpoint ident naming.
    pub fn with_endpoint_ident_naming(self, endpoint_ident_naming: Box<Naming>) -> Self {
        Self {
            endpoint_ident_naming: Some(endpoint_ident_naming),
            ..self
        }
    }

    /// Identify if a character is unsafe for use in a package name.
    fn package_version_unsafe(c: char) -> bool {
        match c {
            '.' | '-' | '~' => true,
            _ => false,
        }
    }

    /// Default strategy for building the version package.
    fn version_package(version: &Version, level: usize, random: &str) -> String {
        let mut parts = String::new();

        parts.push_str("v");
        parts.push_str(&version.major.to_string());

        if level > 0 {
            parts.push_str("_");
            parts.push_str(&version.minor.to_string());
        }

        if level > 1 {
            parts.push_str("_");
            parts.push_str(&version.patch.to_string());
        }

        if level > 2 {
            for p in &version.pre {
                parts.push_str("_");
                parts.push_str(&p.to_string().replace(Self::package_version_unsafe, "_"));
            }
        }

        if level > 3 {
            for b in &version.build {
                parts.push_str("_");
                parts.push_str(&b.to_string().replace(Self::package_version_unsafe, "_"));
            }
        }

        if level > 4 {
            parts.push_str("_");
            parts.push_str(random);
        }

        parts
    }

    /// Build the full package of a versioned package.
    ///
    /// This uses a relatively safe strategy for encoding the version number. This can be adjusted
    /// by overriding `version_package`.
    fn package_with_level(
        &self,
        package: &RpVersionedPackage,
        level: usize,
        random: &str,
    ) -> RpPackage {
        package.to_package(|v| Self::version_package(v, level, random))
    }
}

impl<'a> Environment<'a, CoreFlavor> {
    /// Build a new translator.
    pub fn translator<T: 'static>(&self, flavor: T) -> Result<translator::Context<T>>
    where
        T: FlavorTranslator<Source = CoreFlavor>,
    {
        Ok(translator::Context {
            flavor: flavor,
            types: Rc::clone(&self.types),
            decls: Some(RefCell::new(LinkedHashMap::new())),
        })
    }

    /// Translate the current environment into another.
    ///
    /// This is the final step of the compilation, the environment is consumed by this.
    pub fn translate<T>(self, mut ctx: translator::Context<T>) -> Result<Translated<T::Target>>
    where
        T: FlavorTranslator<Source = CoreFlavor>,
    {
        // Report all collected errors.
        if self.ctx.has_diagnostics()? {
            return Err(Error::new("error in context"));
        }

        let mut files = BTreeMap::new();

        for (package, file) in self.files {
            let package = ctx.translate_package(package)?;
            let mut diag = Diagnostics::new(file.source.clone());

            let file = match file.file.translate(&mut diag, &ctx) {
                Ok(file) => file,
                Err(e) => {
                    self.ctx.diagnostics(diag)?;
                    return Err(e);
                }
            };

            files.insert(package, file);
        }

        let mut decls = LinkedHashMap::new();

        if let Some(d) = ctx.decls.take() {
            // NOTE: we do not know which source to associate this diagnostics with.
            let mut diag = Diagnostics::new(Source::empty("no diagnostics"));

            for (name, reg) in d.into_inner() {
                // NB: it must always be possible to translate name without declarations until all
                // backends to translation.
                let name = name.translate(&mut diag, &ctx)?;
                decls.insert(name, reg);
            }
        }

        Ok(Translated::new(decls, files))
    }

    /// Translation to simplified packages.
    pub fn packages(&self) -> Result<Rc<Packages>> {
        let mut queue = self.files
            .keys()
            .cloned()
            .map(|p| (p, 0))
            .collect::<Vec<_>>();

        let mut files = HashMap::new();

        while !queue.is_empty() {
            let mut candidates = HashMap::new();

            for (count, (package, level)) in queue.drain(..).enumerate() {
                let random = count.to_string();
                let converted = self.package_with_level(&package, level, &random);

                candidates
                    .entry(converted)
                    .or_insert_with(Vec::new)
                    .push((package, level + 1));
            }

            for (converted, partial) in candidates {
                if partial.len() > 1 {
                    // push back into the queue for another round.
                    for p in partial {
                        queue.push(p);
                    }

                    continue;
                }

                if let Some((original, _)) = partial.into_iter().next() {
                    files.insert(original, converted);
                }
            }
        }

        let package_prefix = self.package_prefix.clone();
        let keywords = self.keywords.clone();
        let package_naming = self.package_naming.clone();

        Ok(Rc::new(Packages {
            files,
            package_prefix,
            keywords,
            safe_packages: self.safe_packages,
            package_naming,
        }))
    }

    /// Translate without changing the flavor.
    pub fn translate_default(self) -> Result<Translated<CoreFlavor>> {
        let ctx = self.translator(translator::CoreFlavorTranslator::<_, CoreFlavor>::new(()))?;
        self.translate(ctx)
    }

    /// Import a path into the environment.
    pub fn import_path<P: AsRef<Path>>(
        &mut self,
        path: P,
        package: Option<RpVersionedPackage>,
    ) -> Result<RpVersionedPackage> {
        self.import_source(Source::from_path(path), package)
    }

    /// Import a source into the environment.
    pub fn import_source(
        &mut self,
        source: Source,
        package: Option<RpVersionedPackage>,
    ) -> Result<RpVersionedPackage> {
        let package = package.unwrap_or_else(|| RpVersionedPackage::new(RpPackage::empty(), None));
        let required = RpRequiredPackage::new(package.package.clone(), Range::any());

        if !self.visited.contains_key(&required) {
            let mut diag = Diagnostics::new(source.clone());

            try_with_diag!(self.ctx, diag, {
                self.load_source_diag(&mut diag, &package)
                    .and_then(|file| self.process_file(&mut diag, package.clone(), file))
            });

            self.visited.insert(required, Some(package.clone()));
        }

        Ok(package)
    }

    /// Import a single, structured file.
    pub fn import_file(
        &mut self,
        file: ast::File,
        package: Option<RpVersionedPackage>,
    ) -> Result<RpVersionedPackage> {
        let package = package.unwrap_or_else(|| RpVersionedPackage::new(RpPackage::empty(), None));
        let required = RpRequiredPackage::new(package.package.clone(), Range::any());

        if !self.visited.contains_key(&required) {
            let mut diag = Diagnostics::new(Source::empty("generated"));

            try_with_diag!(self.ctx, diag, {
                self.load_file(&mut diag, file, &package)
                    .and_then(|file| self.process_file(&mut diag, package.clone(), file))
            });

            self.visited.insert(required, Some(package.clone()));
        }

        Ok(package)
    }

    /// Verify all declarations.
    pub fn verify(&mut self) -> Result<()> {
        Ok(())
    }

    /// Load the provided Source into an `RpFile` without registering it to the set of visited
    /// files.
    pub fn load_source(
        &mut self,
        source: Source,
        package: &RpVersionedPackage,
    ) -> Result<RpFile<CoreFlavor>> {
        let mut diag = Diagnostics::new(source.clone());

        Ok(try_with_diag!(self.ctx, diag, {
            self.load_source_diag(&mut diag, &package)
        }))
    }

    /// Load the provided Source into an `RpFile` without registering it to the set of visited
    /// files.
    /// Diagnostics is provided as an argument.
    fn load_source_diag(
        &mut self,
        diag: &mut Diagnostics,
        package: &RpVersionedPackage,
    ) -> result::Result<RpFile<CoreFlavor>, ()> {
        // Notify hook that we loaded a path.
        if let Some(hook) = self.path_hook.as_ref() {
            let r = if let Some(path) = diag.source.path() {
                Some(hook(path))
            } else {
                None
            };

            if let Some(e) = r {
                match e {
                    Ok(()) => {}
                    Err(e) => {
                        diag.err((0, 0), format!("failed to call path hook: {}", e.display()));
                    }
                }
            }
        }

        let reader = match diag.source.read() {
            Ok(reader) => reader,
            Err(e) => {
                diag.err(
                    (0, 0),
                    format!("failed to open file for reading: {}", e.display()),
                );
                return Err(());
            }
        };

        let input = match parser::read_to_string(reader) {
            Ok(input) => input,
            Err(e) => {
                diag.err((0, 0), format!("failed to read file: {}", e.display()));
                return Err(());
            }
        };

        let file = match parser::parse(diag, input.as_str()) {
            Ok(file) => file,
            Err(()) => return Err(()),
        };

        self.load_file(diag, file, package)
    }

    /// try to load the file with the given scope.
    fn load_file<'input>(
        &mut self,
        diag: &mut Diagnostics,
        file: ast::File,
        package: &RpVersionedPackage,
    ) -> result::Result<RpFile<CoreFlavor>, ()> {
        let mut scope = Scope::new(
            package.clone(),
            self.keywords.clone(),
            self.field_ident_naming.as_ref().map(|n| n.copy()),
            self.endpoint_ident_naming.as_ref().map(|n| n.copy()),
            self,
        );

        file.into_model(diag, &mut scope)
    }

    /// Process a single file, populating the environment.
    fn process_file(
        &mut self,
        diag: &mut Diagnostics,
        package: RpVersionedPackage,
        file: RpFile<CoreFlavor>,
    ) -> result::Result<(), ()> {
        use linked_hash_map::Entry::*;

        let file = match self.files.entry(package) {
            btree_map::Entry::Vacant(entry) => entry.insert(File {
                file,
                source: diag.source.clone(),
            }),
            btree_map::Entry::Occupied(_) => {
                return Ok(());
            }
        };

        for (key, _, t) in file.file.decls.iter().flat_map(|d| d.to_reg()) {
            let (key, span) = Loc::borrow_pair(key);
            let key = key.clone().without_prefix();

            debug!("new reg ty: {}", key);

            let types = match Rc::get_mut(&mut self.types) {
                None => {
                    diag.err(span, "non-unique access to environment");
                    continue;
                }
                Some(types) => types,
            };

            match types.entry(key.clone()) {
                Vacant(entry) => entry.insert(Loc::new(t, span)),
                Occupied(entry) => {
                    diag.err(
                        span,
                        format!("`{}` conflicts with existing declaration", key),
                    );
                    diag.info(Loc::span(entry.get()), "existing declaration here");
                    continue;
                }
            };
        }

        if diag.has_errors() {
            return Err(());
        }

        Ok(())
    }
}

impl<'e> Import for Environment<'e, CoreFlavor> {
    /// Import a package based on a package and version criteria.
    fn import(&mut self, required: &RpRequiredPackage) -> Result<Option<RpVersionedPackage>> {
        debug!("import: {}", required);

        if let Some(existing) = self.visited.get(required) {
            debug!("already loaded: {:?} ({})", existing, required);
            return Ok(existing.clone());
        }

        // find all matching objects from the resolver.
        let files = self.resolver.resolve(required)?;

        let result = if let Some(Resolved { version, source }) = files.into_iter().last() {
            debug!("loading: {}", source);

            let package = RpVersionedPackage::new(required.package.clone(), version);

            debug!("found: {} ({})", package, required);

            let mut diag = Diagnostics::new(source.clone());

            try_with_diag!(self.ctx, diag, {
                self.load_source_diag(&mut diag, &package)
                    .and_then(|file| self.process_file(&mut diag, package.clone(), file))
            });

            Some(package)
        } else {
            None
        };

        self.visited.insert(required.clone(), result.clone());
        Ok(result)
    }
}

/// Forward implementation for a mutable reference to the environment.
impl<'a, 'e: 'a> Import for &'a mut Environment<'e, CoreFlavor> {
    fn import(&mut self, required: &RpRequiredPackage) -> Result<Option<RpVersionedPackage>> {
        (*self).import(required)
    }
}

/// Package translation to use.
pub struct Packages {
    files: HashMap<RpVersionedPackage, RpPackage>,
    package_prefix: Option<RpPackage>,
    keywords: Rc<HashMap<String, String>>,
    safe_packages: bool,
    package_naming: Option<Rc<Box<Naming>>>,
}

impl Packages {
    pub fn new(&self, package: &str) -> Result<RpPackage> {
        self.package(RpPackage::parse(package))
    }

    /// Translate the given package.
    pub fn package(&self, package: RpPackage) -> Result<RpPackage> {
        let package = if let Some(package_prefix) = self.package_prefix.as_ref() {
            package_prefix.clone().join_package(package)
        } else {
            package
        };

        let package = if let Some(naming) = self.package_naming.as_ref() {
            package.with_naming(|part| naming.convert(part))
        } else {
            package
        };

        let package = if !self.safe_packages {
            package.with_replacements(&self.keywords)
        } else {
            package
        };

        Ok(package)
    }
}

impl PackageTranslator<RpVersionedPackage, RpPackage> for Packages {
    fn translate_package(&self, package: RpVersionedPackage) -> Result<RpPackage> {
        let package = self.files
            .get(&package)
            .ok_or_else(|| format!("no such package: {}", package))?;

        self.package(package.clone())
    }
}
