//! A dynamically compiled and updated environment.

use ast;
use core::errors::Result;
use core::{self, Context, Diagnostics, Encoding, Import, Loc, Position, Resolved,
           ResolvedByPrefix, Resolver, RpPackage, RpRequiredPackage, RpVersionedPackage, Span};
use env;
use manifest;
use parser;
use std::collections::Bound;
use std::collections::{hash_map, BTreeMap, BTreeSet, HashMap, VecDeque};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use ty;
use url::Url;

/// Specifies a type completion.
#[derive(Debug, Clone)]
pub enum Completion {
    /// Completions for type from a different package.
    Absolute {
        prefix: Option<String>,
        path: Vec<String>,
        suffix: Option<String>,
    },
    /// Completions for a given package.
    Package { results: BTreeSet<String> },
    /// Any type, including primitive types.
    Any,
}

/// Specifies a jump
#[derive(Debug, Clone)]
pub enum Jump {
    /// Perform an absolute jump.
    Absolute {
        prefix: Option<String>,
        path: Vec<String>,
    },
    /// Jump to the specified package prefix.
    Package { prefix: String },
    /// Jump to where the prefix is declared.
    Prefix { prefix: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Range {
    /// Start position.
    pub start: Position,
    /// End position.
    pub end: Position,
}

impl Range {
    pub fn contains(&self, p: &Position) -> bool {
        self.start <= *p && *p <= self.end
    }
}

/// Information about a single prefix.
#[derive(Debug, Clone)]
pub struct Prefix {
    /// The span of the prefix.
    pub span: Span,
    /// The package the prefix referes to.
    pub package: RpVersionedPackage,
}

/// Information about a single symbol.
#[derive(Debug, Clone)]
pub struct Symbol {
    /// The name of the symbol.
    pub name: Loc<String>,
    /// Markdown documentation comment.
    pub comment: Option<String>,
}

impl Symbol {
    /// Convert symbol into documentation.
    pub fn to_documentation(&self) -> Option<ty::Documentation> {
        let comment = match self.comment.as_ref() {
            Some(comment) => comment,
            None => return None,
        };

        let doc = ty::MarkupContent {
            kind: ty::MarkupKind::Markdown,
            value: comment.to_string(),
        };

        Some(ty::Documentation::MarkupContent(doc))
    }
}

#[derive(Debug, Clone)]
pub struct LoadedFile {
    /// Url of the loaded file.
    pub url: Url,
    /// Jumps available in the file.
    pub jumps: BTreeMap<Position, (Range, Jump)>,
    /// Corresponding locations that have available type completions.
    pub completions: BTreeMap<Position, (Range, Completion)>,
    /// package prefixes.
    pub prefixes: HashMap<String, Prefix>,
    /// Symbols present in the file.
    /// The key is the path that the symbol is located in.
    pub symbols: HashMap<Vec<String>, Vec<Symbol>>,
    /// Exact symbol lookup.
    pub symbol: HashMap<Vec<String>, Span>,
    /// Diagnostics for this file.
    pub diag: Diagnostics,
}

impl LoadedFile {
    /// Insert the specified jump.
    pub fn insert_jump(&mut self, span: Span, jump: Jump) -> Result<()> {
        let (start, end) = self.diag.source.span_to_range(span, Encoding::Utf16)?;
        let range = Range { start, end };
        self.jumps.insert(start, (range, jump));
        Ok(())
    }
}

#[derive(Clone)]
pub struct Workspace {
    /// Path of the workspace.
    pub root_path: PathBuf,
    /// Packages which have been loaded through project.
    pub packages: HashMap<RpVersionedPackage, Url>,
    /// Files which have been loaded through project.
    files: HashMap<Url, LoadedFile>,
    /// Versioned packages that have been looked up.
    lookup: HashMap<RpRequiredPackage, RpVersionedPackage>,
    /// Files which are currently being edited.
    pub edited_files: HashMap<Url, LoadedFile>,
    /// Context where to populate compiler errors.
    ctx: Rc<Context>,
}

impl Workspace {
    /// Create a new workspace from the given path.
    pub fn new<P: AsRef<Path>>(root_path: P, ctx: Rc<Context>) -> Self {
        Self {
            root_path: root_path.as_ref().to_owned(),
            packages: HashMap::new(),
            files: HashMap::new(),
            lookup: HashMap::new(),
            edited_files: HashMap::new(),
            ctx,
        }
    }

    /// Access all files in the workspace.
    pub fn files(&self) -> Vec<(&Url, &LoadedFile)> {
        let mut files = Vec::new();
        files.extend(self.files.iter());
        files.extend(self.edited_files.iter());
        files
    }

    /// Access the loaded file with the given Url.
    pub fn file(&self, url: &Url) -> Option<&LoadedFile> {
        if let Some(file) = self.edited_files.get(url) {
            return Some(file);
        }

        if let Some(file) = self.files.get(url) {
            return Some(file);
        }

        None
    }

    /// Reload the workspace.
    pub fn reload(&mut self) -> Result<()> {
        self.packages.clear();
        self.files.clear();
        self.lookup.clear();
        let manifest = self.try_reload()?;
        self.try_compile(manifest)?;
        Ok(())
    }

    /// Reload the workspace.
    pub fn try_reload(&mut self) -> Result<manifest::Manifest> {
        let path = self.root_path.join(env::MANIFEST_NAME);

        let mut manifest = manifest::Manifest::default();

        if path.is_file() {
            manifest.path = Some(path.to_owned());
            manifest.from_yaml(File::open(path)?, env::convert_lang)?;
        }

        let mut resolver = env::resolver(&manifest)?;

        for package in &manifest.packages {
            self.process(resolver.as_mut(), package)?;
        }

        return Ok(manifest);
    }

    /// Try to compile the current environment.
    fn try_compile(&mut self, manifest: manifest::Manifest) -> Result<()> {
        let ctx = self.ctx.clone();
        ctx.clear()?;

        let lang = manifest.lang_or_nolang();
        let package_prefix = manifest.package_prefix.clone();
        let mut env = lang.into_env(ctx.clone(), package_prefix, self);

        for package in &manifest.packages {
            if let Err(e) = env.import(package) {
                debug!("failed to import: {}: {}", package, e.display());
            }
        }

        if let Err(e) = lang.compile(ctx.clone(), env, manifest) {
            // ignore and just go off diagnostics?
            debug!("compile error: {}", e.display());
        }

        return Ok(());
    }

    fn process(
        &mut self,
        resolver: &mut Resolver,
        package: &RpRequiredPackage,
    ) -> Result<Option<RpVersionedPackage>> {
        // need method to report errors in this stage.
        let (url, source, versioned) = {
            let entry = match self.lookup.entry(package.clone()) {
                hash_map::Entry::Occupied(e) => return Ok(Some(e.get().clone())),
                hash_map::Entry::Vacant(e) => e,
            };

            let resolved = match resolver.resolve(package) {
                Ok(resolved) => resolved,
                Err(_) => return Ok(None),
            };

            let Resolved { version, source } = match resolved.into_iter().last() {
                Some(resolved) => resolved,
                None => return Ok(None),
            };

            let path = match source.path().map(|p| p.to_owned()) {
                Some(path) => path,
                None => return Ok(None),
            };

            let versioned = RpVersionedPackage::new(package.package.clone(), version);
            entry.insert(versioned.clone());

            // TODO: report error through diagnostics.
            let path = match path.canonicalize() {
                Ok(path) => path,
                Err(_) => return Ok(None),
            };

            let path = path.canonicalize()
                .map_err(|e| format!("cannot canonicalize path: {}: {}", path.display(), e))?;

            let url = Url::from_file_path(&path)
                .map_err(|_| format!("cannot build url from path: {}", path.display()))?;

            (url, source, versioned)
        };

        if let Some(mut loaded) = self.edited_files.remove(&url) {
            loaded.symbols.clear();
            loaded.symbol.clear();
            loaded.prefixes.clear();
            loaded.jumps.clear();
            loaded.completions.clear();
            loaded.diag.clear();

            self.inner_process(resolver, &mut loaded)?;
            self.edited_files.insert(url.clone(), loaded);
        } else {
            let mut loaded = LoadedFile {
                url: url.clone(),
                jumps: BTreeMap::new(),
                completions: BTreeMap::new(),
                prefixes: HashMap::new(),
                symbols: HashMap::new(),
                symbol: HashMap::new(),
                diag: Diagnostics::new(source),
            };

            self.inner_process(resolver, &mut loaded)?;
            self.files.insert(url.clone(), loaded);
        };

        self.packages.insert(versioned.clone(), url);
        Ok(Some(versioned))
    }

    fn inner_process(&mut self, resolver: &mut Resolver, loaded: &mut LoadedFile) -> Result<()> {
        let content = {
            let mut content = String::new();
            let mut reader = loaded.diag.source.read()?;
            reader.read_to_string(&mut content)?;
            content
        };

        let file = match parser::parse(&mut loaded.diag, content.as_str()) {
            Ok(file) => file,
            Err(()) => {
                return Ok(());
            }
        };

        for u in &file.uses {
            let (u, span) = Loc::borrow_pair(u);

            let range = match u.range {
                Some(ref range) => match core::Range::parse(range.as_str()) {
                    Ok(range) => range,
                    Err(_) => continue,
                },
                None => core::Range::any(),
            };

            let package = {
                let (package, span) = Loc::borrow_pair(&u.package);

                let (start, end) = loaded.diag.source.span_to_range(span, Encoding::Utf16)?;
                let range = Range { start, end };

                let content = &content[span.start..span.end];
                let completion = self.package_completion(content, resolver)?;
                loaded.completions.insert(start, (range, completion));
                package
            };

            let package = match *package {
                ast::Package::Package { ref package } => package,
                ast::Package::Error => {
                    continue;
                }
            };

            let package = RpRequiredPackage::new(package.clone(), range);

            let looked_up = match self.process(resolver, &package)? {
                Some(package) => package,
                None => continue,
            };

            let prefix = u.alias
                .as_ref()
                .map(|a| a.as_ref())
                .or_else(|| package.package.parts().last().map(|p| p.as_str()));

            let prefix = match prefix {
                Some(prefix) => prefix.to_string(),
                None => continue,
            };

            loaded.insert_jump(
                span,
                Jump::Package {
                    prefix: prefix.clone(),
                },
            )?;

            loaded.prefixes.insert(
                prefix,
                Prefix {
                    span: span,
                    package: looked_up,
                },
            );
        }

        let mut queue = VecDeque::new();

        queue.extend(file.decls.iter().map(|d| (vec![], d)));

        while let Some((mut path, decl)) = queue.pop_front() {
            let comment = decl.comment();

            let comment = if !comment.is_empty() {
                Some(
                    comment
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
            } else {
                None
            };

            loaded
                .symbols
                .entry(path.clone())
                .or_insert_with(Vec::default)
                .push(Symbol {
                    name: Loc::map(decl.name(), |n| n.to_string()),
                    comment,
                });

            path.push(decl.name().to_string());

            loaded.symbol.insert(path.clone(), Loc::span(&decl.name()));

            self.process_decl(&path, loaded, content.as_str(), decl)?;

            queue.extend(decl.decls().map(|decl| (path.clone(), decl)));
        }

        Ok(())
    }

    /// Process all locations assocaited with the declarations.
    ///
    /// * `completions`, locations which are applicable for type completions.
    fn process_decl<'input>(
        &mut self,
        current: &Vec<String>,
        loaded: &mut LoadedFile,
        content: &str,
        decl: &ast::Decl<'input>,
    ) -> Result<()> {
        use ast::Decl::*;

        match *decl {
            Type(ref ty) => for f in ty.fields() {
                self.process_ty(current, loaded, content, &f.ty)?;
            },
            Tuple(ref tuple) => for f in tuple.fields() {
                self.process_ty(current, loaded, content, &f.ty)?;
            },
            Interface(ref interface) => for f in interface.fields() {
                self.process_ty(current, loaded, content, &f.ty)?;
            },
            Enum(ref _en) => {}
            Service(ref service) => {
                for e in service.endpoints() {
                    for a in &e.arguments {
                        self.process_ty(current, loaded, content, a.channel.ty())?;
                    }

                    if let Some(response) = e.response.as_ref() {
                        self.process_ty(current, loaded, content, response.ty())?;
                    }
                }
            }
        }

        Ok(())
    }

    fn process_ty<'input>(
        &mut self,
        current: &Vec<String>,
        loaded: &mut LoadedFile,
        content: &str,
        ty: &Loc<ast::Type<'input>>,
    ) -> Result<()> {
        let (ty, span) = Loc::borrow_pair(ty);

        match *ty {
            ast::Type::Array { ref inner } => {
                self.process_ty(current, loaded, content, inner.as_ref())?;
            }
            ast::Type::Map { ref key, ref value } => {
                self.process_ty(current, loaded, content, key.as_ref())?;
                self.process_ty(current, loaded, content, value.as_ref())?;
            }
            ref ty => {
                if let ast::Type::Error = *ty {
                    // NOTE: catch these diagnostics in the compile phase.
                    /*loaded
                        .diag
                        .err(span, "expected type, like: `u32`, `string` or `Foo`");*/
                }

                // load jump-to definitions
                if let ast::Type::Name { ref name } = *ty {
                    self.jumps(name, current, loaded)?;
                }

                let (start, end) = loaded.diag.source.span_to_range(span, Encoding::Utf16)?;
                let range = Range { start, end };

                let content = &content[span.start..span.end];
                let completion = self.type_completion(current, content)?;

                loaded.completions.insert(start, (range, completion));
            }
        }

        Ok(())
    }

    /// Register all available jumps.
    fn jumps<'input>(
        &self,
        name: &Loc<ast::Name<'input>>,
        current: &Vec<String>,
        loaded: &mut LoadedFile,
    ) -> Result<()> {
        let (name, _) = Loc::borrow_pair(name);

        match *name {
            ast::Name::Relative { ref parts } => {
                let mut path = current.clone();

                for p in parts {
                    let (p, span) = Loc::borrow_pair(p);

                    path.push(p.to_string());

                    loaded.insert_jump(
                        span,
                        Jump::Absolute {
                            prefix: None,
                            path: path.clone(),
                        },
                    )?;
                }
            }
            ast::Name::Absolute {
                ref prefix,
                ref parts,
            } => {
                let mut path = Vec::new();

                if let Some(ref prefix) = *prefix {
                    let (prefix, span) = Loc::borrow_pair(prefix);

                    loaded.insert_jump(
                        span,
                        Jump::Prefix {
                            prefix: prefix.to_string(),
                        },
                    )?;
                }

                let prefix = prefix.as_ref().map(|p| p.to_string());

                for p in parts {
                    let (p, span) = Loc::borrow_pair(p);

                    path.push(p.to_string());

                    loaded.insert_jump(
                        span,
                        Jump::Absolute {
                            prefix: prefix.clone(),
                            path: path.clone(),
                        },
                    )?;
                }
            }
        }

        Ok(())
    }

    /// Build a package completion.
    fn package_completion(&self, content: &str, resolver: &mut Resolver) -> Result<Completion> {
        debug!("package completion from {:?}", content);

        let mut parts = content.split(|c: char| c.is_whitespace());

        let content = match parts.next() {
            Some(content) => content,
            None => content,
        };

        let mut parts = content
            .split(".")
            .map(|s| s.to_string())
            .collect::<Vec<_>>();

        let suffix = parts.pop();
        let package = RpPackage::new(parts);

        let resolved = resolver.resolve_by_prefix(&package)?;

        let mut results = BTreeSet::new();

        for r in resolved {
            if let Some(value) = r.package.parts().skip(package.len()).next() {
                if let Some(suffix) = suffix.as_ref() {
                    let suffix = suffix.to_lowercase();

                    if !value.to_lowercase().starts_with(&suffix) {
                        continue;
                    }
                }

                results.insert(value.to_string());
            }
        }

        Ok(Completion::Package { results })
    }

    /// Figure out the kind of completion to support.
    fn type_completion(&self, current: &Vec<String>, content: &str) -> Result<Completion> {
        if content.chars().all(|c| c.is_whitespace()) {
            return Ok(Completion::Any);
        }

        if content.starts_with("::") {
            let content = &content[2..];

            let mut path = current.clone();
            path.extend(content.split("::").map(|p| p.to_string()));

            let suffix = path.pop().and_then(|s| {
                if !s.chars().all(|c| c.is_whitespace()) {
                    Some(s.to_string())
                } else {
                    None
                }
            });

            return Ok(Completion::Absolute {
                prefix: None,
                path,
                suffix,
            });
        }

        let mut path = content
            .split("::")
            .map(|p| p.to_string())
            .collect::<Vec<_>>();

        if !path.is_empty() {
            let prefix = if let Some(first) = path.first() {
                if first.chars().all(|c| c.is_lowercase()) {
                    Some(first.to_string())
                } else {
                    None
                }
            } else {
                None
            };

            if prefix.is_some() {
                path.remove(0);
            }

            let suffix = path.pop().and_then(|s| {
                if !s.chars().all(|c| c.is_whitespace()) {
                    Some(s.to_string())
                } else {
                    None
                }
            });

            return Ok(Completion::Absolute {
                prefix,
                path,
                suffix,
            });
        }

        Ok(Completion::Any)
    }

    /// Find the type completion associated with the given position.
    pub fn find_completion(
        &self,
        url: &Url,
        position: ty::Position,
    ) -> Result<Option<(&LoadedFile, &Completion)>> {
        let file = match self.file(url) {
            Some(file) => file,
            None => return Ok(None),
        };

        let end = Position {
            line: position.line as usize,
            col: position.character as usize,
        };

        let mut range = file.completions
            .range((Bound::Unbounded, Bound::Included(&end)));

        let (range, value) = match range.next_back() {
            Some((_, &(ref range, ref value))) => (range, value),
            None => return Ok(None),
        };

        if !range.contains(&end) {
            return Ok(None);
        }

        Ok(Some((file, value)))
    }

    /// Find the associated jump.
    pub fn find_jump(
        &self,
        url: &Url,
        position: ty::Position,
    ) -> Result<Option<(&LoadedFile, &Jump)>> {
        let file = match self.file(url) {
            Some(file) => file,
            None => return Ok(None),
        };

        let end = Position {
            line: position.line as usize,
            col: position.character as usize,
        };

        let mut range = file.jumps.range((Bound::Unbounded, Bound::Included(&end)));

        let (range, value) = match range.next_back() {
            Some((_, &(ref range, ref value))) => (range, value),
            None => return Ok(None),
        };

        if !range.contains(&end) {
            return Ok(None);
        }

        Ok(Some((file, value)))
    }
}

impl Resolver for Workspace {
    /// Resolve a single package.
    fn resolve(&mut self, package: &RpRequiredPackage) -> Result<Vec<Resolved>> {
        let mut result = Vec::new();

        if let Some(looked_up) = self.lookup.get(package) {
            if let Some(url) = self.packages.get(looked_up) {
                if let Some(loaded) = self.file(url) {
                    result.push(Resolved {
                        version: looked_up.version.clone(),
                        source: loaded.diag.source.clone(),
                    });
                }
            }
        }

        Ok(result)
    }

    /// Not supported for workspace.
    fn resolve_by_prefix(&mut self, _: &RpPackage) -> Result<Vec<ResolvedByPrefix>> {
        Ok(vec![])
    }
}
