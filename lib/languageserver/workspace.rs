//! A dynamically compiled and updated environment.

use ast;
use core::errors::Result;
use core::{self, Diagnostics, Loc, Resolved, Resolver, RpRequiredPackage, RpVersionedPackage};
use env;
use internal_log::InternalLog;
use lexer::Token;
use manifest;
use parser;
use ropey::Rope;
use std::cell::RefCell;
use std::collections::{hash_map, BTreeMap, HashMap, VecDeque};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use url::Url;

/// Specifies a type completion.
#[derive(Debug, Clone)]
pub enum TypeCompletion {
    /// Completions for type from a different package.
    Imported { prefix: String, parts: Vec<String> },
    /// A local type.
    Local { parts: Vec<String> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position {
    /// Line that the position relates to.
    pub line: u64,
    /// Column that the position relates to.
    pub col: u64,
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

#[derive(Debug, Clone)]
pub struct LoadedFile {
    /// Corresponding locations that have available type completions.
    pub types: BTreeMap<Position, (Range, TypeCompletion)>,
    /// package prefixes.
    pub prefixes: HashMap<String, RpVersionedPackage>,
    /// Symbols present in the file.
    /// The key is the path that the symbol is located in.
    pub symbols: HashMap<Vec<String>, Vec<String>>,
    /// Diagnostics for this file.
    pub diag: Diagnostics,
    /// File buffer.
    pub rope: Rope,
}

#[derive(Debug, Clone)]
pub struct Workspace {
    pub root_path: PathBuf,
    pub packages: HashMap<RpVersionedPackage, Rc<RefCell<LoadedFile>>>,
    pub files: HashMap<Url, Rc<RefCell<LoadedFile>>>,
}

impl Workspace {
    /// Create a new workspace from the given path.
    pub fn new<P: AsRef<Path>>(root_path: P) -> Self {
        Self {
            root_path: root_path.as_ref().to_owned(),
            packages: HashMap::new(),
            files: HashMap::new(),
        }
    }

    /// Reload the workspace.
    pub fn reload(&mut self, log: &mut InternalLog) -> Result<()> {
        self.packages.clear();
        self.files.clear();
        self.try_reload(log)
    }

    /// Reload the workspace.
    pub fn try_reload(&mut self, log: &mut InternalLog) -> Result<()> {
        let path = self.root_path.join(env::MANIFEST_NAME);

        let mut manifest = manifest::Manifest::default();

        if path.is_file() {
            manifest.path = Some(path.to_owned());
            manifest.from_yaml(File::open(path)?, env::convert_lang)?;
        }

        let mut resolver = env::resolver(&manifest)?;

        let mut lookup = HashMap::new();

        for package in &manifest.packages {
            writeln!(log, "package: {}", package).expect("bye");
            self.process(log, &mut lookup, resolver.as_mut(), package);
        }

        return Ok(());
    }

    fn process(
        &mut self,
        log: &mut InternalLog,
        lookup: &mut HashMap<RpRequiredPackage, RpVersionedPackage>,
        resolver: &mut Resolver,
        package: &RpRequiredPackage,
    ) -> Option<RpVersionedPackage> {
        // need method to report errors in this stage.
        let (rope, path, source, versioned) = {
            let entry = match lookup.entry(package.clone()) {
                hash_map::Entry::Occupied(e) => return Some(e.get().clone()),
                hash_map::Entry::Vacant(e) => e,
            };

            let resolved = match resolver.resolve(package) {
                Ok(resolved) => resolved,
                Err(_) => return None,
            };

            let Resolved { version, source } = match resolved.into_iter().last() {
                Some(resolved) => resolved,
                None => return None,
            };

            let path = match source.path().map(|p| p.to_owned()) {
                Some(path) => path,
                None => return None,
            };

            let versioned = RpVersionedPackage::new(package.package.clone(), version);
            entry.insert(versioned.clone());

            let reader = match source.read() {
                Ok(reader) => reader,
                Err(_) => return None,
            };

            let rope = match Rope::from_reader(reader) {
                Ok(rope) => rope,
                Err(_) => return None,
            };

            // TODO: report error through diagnostics.
            let path = match path.canonicalize() {
                Ok(path) => path,
                Err(_) => return None,
            };

            let path = match path.canonicalize() {
                Ok(path) => path,
                Err(_) => return None,
            };

            let path = match Url::from_file_path(path) {
                Ok(path) => path,
                Err(_) => return None,
            };

            (rope, path, source, versioned)
        };

        let mut loaded = LoadedFile {
            types: BTreeMap::new(),
            prefixes: HashMap::new(),
            symbols: HashMap::new(),
            diag: Diagnostics::new(source.clone()),
            rope,
        };

        self.inner_process(log, lookup, resolver, &mut loaded);
        let loaded = Rc::new(RefCell::new(loaded));

        self.packages.insert(versioned.clone(), Rc::clone(&loaded));
        self.files.insert(path, Rc::clone(&loaded));
        Some(versioned)
    }

    fn inner_process(
        &mut self,
        log: &mut InternalLog,
        lookup: &mut HashMap<RpRequiredPackage, RpVersionedPackage>,
        resolver: &mut Resolver,
        loaded: &mut LoadedFile,
    ) {
        let string = loaded.rope.to_string();

        let file = match parser::parse(&mut loaded.diag, string.as_str()) {
            Ok(file) => file,
            Err(()) => {
                return;
            }
        };

        for u in file.uses {
            let package = Loc::borrow(&u.package).clone();

            let range = match u.range {
                Some(ref range) => match core::Range::parse(range.as_str()) {
                    Ok(range) => range,
                    Err(_) => {
                        loaded.diag.err(Loc::span(range), "illegal range");
                        continue;
                    }
                },
                None => core::Range::any(),
            };

            let package = RpRequiredPackage::new(package, range);

            let looked_up = match self.process(log, lookup, resolver, &package) {
                Some(package) => package,
                None => {
                    loaded
                        .diag
                        .err(Loc::span(&u), format!("no such package: {}", package));
                    continue;
                }
            };

            let prefix = u.alias
                .as_ref()
                .map(|a| a.as_ref())
                .or_else(|| package.package.parts().last().map(|p| p.as_str()));

            let prefix = match prefix {
                Some(prefix) => prefix.to_string(),
                None => {
                    loaded.diag.err(Loc::span(&u), "no prefix for use");
                    continue;
                }
            };

            loaded.prefixes.insert(prefix, looked_up);
        }

        let mut queue = VecDeque::new();

        queue.extend(file.decls.iter().map(|d| (vec![], d)));

        while let Some((mut path, decl)) = queue.pop_front() {
            self.process_locations(log, loaded, decl);

            loaded
                .symbols
                .entry(path.clone())
                .or_insert_with(Vec::default)
                .push(decl.name().to_string());

            path.push(decl.name().to_string());
            queue.extend(decl.decls().map(|decl| (path.clone(), decl)));
        }
    }

    /// Process all locations assocaited with the declarations.
    ///
    /// * `types`, locations which are applicable for type completions.
    fn process_locations<'input>(
        &mut self,
        log: &mut InternalLog,
        loaded: &mut LoadedFile,
        decl: &ast::Decl<'input>,
    ) {
        use ast::Decl::*;

        match *decl {
            Type(ref ty) => for f in ty.fields() {
                self.process_ty(log, loaded, &f.ty);
            },
            Tuple(ref tuple) => for f in tuple.fields() {
                self.process_ty(log, loaded, &f.ty);
            },
            Interface(ref interface) => for f in interface.fields() {
                self.process_ty(log, loaded, &f.ty);
            },
            Enum(ref _en) => {}
            Service(ref _service) => {}
        }
    }

    fn process_ty<'input>(
        &mut self,
        log: &mut InternalLog,
        loaded: &mut LoadedFile,
        ty: &Loc<ast::ErrorRecovery<ast::Type<'input>>>,
    ) {
        use ast::ErrorRecovery::*;

        let (ty, span) = Loc::borrow_pair(ty);

        let reader = match loaded.diag.source.read() {
            Ok(reader) => reader,
            Err(_) => return,
        };

        let (line_start, line_end, col_start, col_end) =
            match core::utils::find_range(reader, (span.start, span.end)) {
                Ok(range) => range,
                Err(_) => return,
            };

        let start = Position {
            line: line_start as u64,
            col: col_start as u64,
        };

        let end = Position {
            line: line_end as u64,
            col: col_end as u64,
        };

        let range = Range { start, end };

        match *ty {
            Error(ref tokens) => {
                if let Some(c) = self.type_completion(log, tokens) {
                    loaded.types.insert(start, (range, c));
                }

                loaded
                    .diag
                    .err(span, "expected type, like: `u32`, `string` or `Foo`");
            }
            Value(ref ty) => {}
        }
    }

    /// Build a type completion.
    fn type_completion<'input>(
        &mut self,
        log: &mut InternalLog,
        errors: &Vec<(usize, Token<'input>, usize)>,
    ) -> Option<TypeCompletion> {
        writeln!(log, "recovery: {:?}", errors).expect("bad log");

        let mut it = errors.into_iter().map(|t| &t.1);

        let first = it.next();

        if let Some(prefix) = first.and_then(|f| f.as_ident()) {
            let mut parts = Vec::new();

            load_scope(&mut parts, &mut it);

            if it.next().is_some() {
                return None;
            }

            return Some(TypeCompletion::Imported {
                prefix: prefix.to_string(),
                parts: parts,
            });
        }

        let mut parts = Vec::new();

        if let Some(&Token::TypeIdentifier(ref ident)) = first {
            parts.push(ident.to_string());
        }

        load_scope(&mut parts, &mut it);

        if it.next().is_some() {
            return None;
        }

        return Some(TypeCompletion::Local { parts: parts });

        /// Read parts of a name including all the scope parts.
        fn load_scope<'a, 'input: 'a, I>(parts: &mut Vec<String>, mut it: I)
        where
            I: Iterator<Item = &'a Token<'input>>,
        {
            loop {
                if let Some(&Token::TypeIdentifier(ref ident)) = it.next() {
                    parts.push(ident.to_string());
                }

                if let Some(&Token::Scope) = it.next() {
                    continue;
                }

                break;
            }
        }
    }
}
