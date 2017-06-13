use core::*;
use core::into_model::IntoModel;
use linked_hash_map::{self, LinkedHashMap};
use parser;
use parser::ast;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use super::errors::*;

pub type InitFields = HashMap<String, RpLoc<RpFieldInit>>;

const EXT: &str = "reproto";

pub struct Environment {
    paths: Vec<PathBuf>,
    visited: HashSet<RpPackage>,
    pub types: LinkedHashMap<RpTypeId, RpLoc<RpRegistered>>,
    pub decls: LinkedHashMap<RpTypeId, Rc<RpLoc<RpDecl>>>,
    pub used: LinkedHashMap<(RpPackage, String), RpPackage>,
}

impl Environment {
    pub fn new(paths: Vec<PathBuf>) -> Environment {
        Environment {
            paths: paths,
            visited: HashSet::new(),
            types: LinkedHashMap::new(),
            decls: LinkedHashMap::new(),
            used: LinkedHashMap::new(),
        }
    }

    fn into_registered_type(&self,
                            package: &RpPackage,
                            decl: Rc<RpLoc<RpDecl>>)
                            -> Result<Vec<(RpTypeId, RpLoc<RpRegistered>)>> {
        let mut out = Vec::new();

        match decl.inner {
            RpDecl::Type(ref ty) => {
                let type_id = RpTypeId::new(package.clone(),
                                            RpName::with_parts(vec![ty.name.clone()]));
                let token = RpLoc::new(RpRegistered::Type(ty.clone()), decl.pos.clone());
                out.push((type_id, token));
            }
            RpDecl::Interface(ref interface) => {
                let current = vec![interface.name.clone()];
                let type_id = RpTypeId::new(package.clone(), RpName::with_parts(current.clone()));
                let token = RpLoc::new(RpRegistered::Interface(interface.clone()),
                                       decl.pos.clone());

                for (name, sub_type) in &interface.sub_types {
                    let sub_type = RpRegistered::SubType {
                        parent: interface.clone(),
                        sub_type: sub_type.inner.clone(),
                    };
                    let token = RpLoc::new(sub_type, decl.pos.clone());

                    let mut current = current.clone();
                    current.push(name.to_owned());
                    out.push((type_id.with_name(RpName::with_parts(current)), token));
                }

                out.push((type_id, token));
            }
            RpDecl::Enum(ref en) => {
                let current = vec![en.name.clone()];
                let type_id = RpTypeId::new(package.clone(), RpName::with_parts(current.clone()));
                let token = RpLoc::new(RpRegistered::Enum(en.clone()), decl.pos.clone());

                for variant in &en.variants {
                    let enum_constant = RpRegistered::EnumConstant {
                        parent: en.clone(),
                        variant: variant.inner.clone(),
                    };
                    let token = RpLoc::new(enum_constant, decl.pos.clone());

                    let mut current = current.clone();
                    current.push((*variant.name).to_owned());
                    out.push((type_id.with_name(RpName::with_parts(current)), token));
                }

                out.push((type_id, token));
            }
            RpDecl::Tuple(ref tuple) => {
                let type_id = RpTypeId::new(package.clone(),
                                            RpName::with_parts(vec![tuple.name.clone()]));
                let token = RpLoc::new(RpRegistered::Tuple(tuple.clone()), decl.pos.clone());
                out.push((type_id, token));
            }
        }

        Ok(out)
    }

    fn register_alias(&mut self, package: &RpPackage, use_decl: &ast::UseDecl) -> Result<()> {
        if let Some(used) = use_decl.package.parts.iter().last() {
            let alias = if let Some(ref next) = use_decl.alias {
                next
            } else {
                used
            };

            let key = (package.clone(), alias.clone());

            match self.used.entry(key) {
                linked_hash_map::Entry::Vacant(entry) => {
                    entry.insert(use_decl.package.inner.clone())
                }
                linked_hash_map::Entry::Occupied(_) => {
                    return Err(format!("alias {} already in used", alias).into())
                }
            };
        }

        Ok(())
    }

    pub fn is_assignable_from(&self,
                              package: &RpPackage,
                              target: &RpType,
                              source: &RpType)
                              -> Result<bool> {
        match (target, source) {
            (&RpType::Double, &RpType::Double) => Ok(true),
            (&RpType::Float, &RpType::Float) => Ok(true),
            (&RpType::Signed(Some(ref target)), &RpType::Signed(Some(ref source))) => {
                Ok(target <= source)
            }
            // unknown size matches known
            (&RpType::Signed(_), &RpType::Signed(None)) => Ok(true),
            (&RpType::Unsigned(Some(ref target)), &RpType::Unsigned(Some(ref source))) => {
                Ok(target <= source)
            }
            // unknown size matches known
            (&RpType::Unsigned(_), &RpType::Unsigned(None)) => Ok(true),
            (&RpType::Boolean, &RpType::Boolean) => return Ok(true),
            (&RpType::String, &RpType::String) => return Ok(true),
            (&RpType::Bytes, &RpType::Bytes) => return Ok(true),
            // everything assignable to any type
            (&RpType::Any, _) => Ok(true),
            (&RpType::Name(ref target), &RpType::Name(ref source)) => {
                let target = self.lookup(package, target)?;
                let source = self.lookup(package, source)?;
                return Ok(target.is_assignable_from(source));
            }
            // arrays match if inner type matches
            (&RpType::Array(ref target), &RpType::Array(ref source)) => {
                return self.is_assignable_from(package, target, source);
            }
            (&RpType::Map(ref target_key, ref target_value),
             &RpType::Map(ref source_key, ref source_value)) => {
                let key_assignable = self.is_assignable_from(package, target_key, source_key)?;
                let value_assignable =
                    self.is_assignable_from(package, target_value, source_value)?;

                return Ok(key_assignable && value_assignable);
            }
            _ => Ok(false),
        }
    }

    pub fn constant<'a>(&'a self,
                        pos: &RpPos,
                        package: &'a RpPackage,
                        constant: &RpName,
                        target: &RpName)
                        -> Result<&'a RpRegistered> {
        let reg_constant = self.lookup(package, constant)
            .map_err(|e| Error::pos(e.description().to_owned(), pos.clone()))?;

        let reg_target = self.lookup(package, target)
            .map_err(|e| Error::pos(e.description().to_owned(), pos.clone()))?;

        if !reg_target.is_assignable_from(reg_constant) {
            return Err(Error::pos(format!("expected instance of `{}` but found `{}`",
                                          reg_target.display(),
                                          reg_constant.display()),
                                  pos.clone()));
        }

        Ok(reg_constant)
    }

    /// Convert instance arguments to the known registered type of the instance, and a map
    /// containing the arguments being instantiated.
    pub fn instance<'a>(&'a self,
                        pos: &RpPos,
                        package: &'a RpPackage,
                        instance: &RpInstance,
                        target: &RpName)
                        -> Result<(&'a RpRegistered, InitFields)> {
        let reg_instance = self.lookup(package, &instance.ty)
            .map_err(|e| Error::pos(e.description().to_owned(), pos.clone()))?;

        let reg_target = self.lookup(package, target)
            .map_err(|e| Error::pos(e.description().to_owned(), pos.clone()))?;

        if !reg_target.is_assignable_from(reg_instance) {
            return Err(Error::pos(format!("expected instance of `{}` but found `{}`",
                                          reg_target.display(),
                                          reg_instance.display()),
                                  pos.clone()));
        }

        let required_fields = match *reg_instance {
            RpRegistered::Type(ref ty) => ty.fields(),
            RpRegistered::SubType { ref parent, ref sub_type } => {
                Box::new(parent.fields().chain(sub_type.fields()))
            }
            RpRegistered::Tuple(ref tuple) => tuple.fields(),
            _ => return Err(Error::pos("expected instantiable type".into(), pos.clone())),
        };

        // pick required fields.
        let required_fields = required_fields.filter(|f| f.modifier == RpModifier::Required);

        let mut known: HashMap<String, RpLoc<RpFieldInit>> = HashMap::new();

        // check that all required fields are set.
        let mut required: BTreeMap<String, RpLoc<RpField>> = required_fields.map(Clone::clone)
            .map(|f| (f.name.clone(), f))
            .collect();

        for init in &*instance.arguments {
            if let Some(ref field) = reg_instance.find_field(&init.name)? {
                // TODO: map out init position, and check that required variables are set.
                known.insert(field.name.clone(), init.clone());
                required.remove(&field.name);
            } else {
                return Err(Error::pos("no such field".to_owned(), init.pos.clone()));
            }
        }

        if !required.is_empty() {
            let required: Vec<(String, RpLoc<RpField>)> = required.into_iter()
                .collect();

            let names: Vec<String> =
                required.iter().map(|&(ref name, _)| name.to_owned()).collect();

            let positions: Vec<RpPos> = required.iter().map(|&(_, ref t)| t.pos.clone()).collect();

            return Err(ErrorKind::MissingRequired(names,
                                                  instance.arguments.pos.clone(),
                                                  positions)
                .into());
        }

        Ok((reg_instance, known))
    }

    /// Lookup the package declaration a used alias refers to.
    pub fn lookup_used(&self, package: &RpPackage, used: &str) -> Result<&RpPackage> {
        // resolve alias
        self.used
            .get(&(package.clone(), used.to_owned()))
            .ok_or_else(|| format!("Missing import alias for ({})", used).into())
    }

    /// Lookup the declaration matching the custom type.
    pub fn lookup<'a>(&'a self,
                      package: &'a RpPackage,
                      custom: &RpName)
                      -> Result<&'a RpRegistered> {
        let package = if let Some(ref prefix) = custom.prefix {
            self.lookup_used(package, prefix)?
        } else {
            package
        };

        let key = RpTypeId::new(package.clone(), custom.clone());

        if let Some(ty) = self.types.get(&key) {
            return Ok(ty);
        }

        return Err("no such type".into());
    }

    pub fn import_file(&mut self, path: &Path, package: Option<&RpPackage>) -> Result<()> {
        debug!("in: {}", path.display());

        let file = parser::parse_file(&path)?;

        if let Some(package) = package {
            if *file.package != *package {
                return Err(format!("Expected package ({}) in file {}, but was ({})",
                                   package,
                                   path.display(),
                                   *file.package)
                    .into());
            }
        }

        for use_decl in &file.uses {
            self.register_alias(&file.package, use_decl)?;
            self.import(&use_decl.package)?;
        }

        let mut decls = LinkedHashMap::new();

        for decl in file.decls {
            let pos = (path.to_owned(), decl.pos.0, decl.pos.1);
            let decl = decl.into_model(&pos)?;

            let custom = RpName::with_parts(vec![decl.name().to_owned()]);
            let key = RpTypeId::new(file.package.inner.clone(), custom);

            match decls.entry(key) {
                linked_hash_map::Entry::Vacant(entry) => {
                    entry.insert(Rc::new(decl));
                }
                linked_hash_map::Entry::Occupied(entry) => {
                    entry.into_mut().merge(Rc::new(decl))?;
                }
            }
        }

        let mut types = LinkedHashMap::new();

        // again, post-merge
        for (_, decl) in &decls {
            let registered_types = self.into_registered_type(&file.package, decl.clone())?;

            for (key, t) in registered_types.into_iter() {
                if let Some(_) = types.insert(key.clone(), t) {
                    return Err(ErrorKind::RegisteredTypeConflict(key.clone()).into());
                }
            }
        }

        self.decls.extend(decls);
        self.types.extend(types);
        Ok(())
    }

    pub fn import(&mut self, package: &RpPackage) -> Result<()> {
        if self.visited.contains(package) {
            return Ok(());
        }

        self.visited.insert(package.clone());

        let mut files: Vec<PathBuf> = Vec::new();

        let candidates: Vec<PathBuf> = self.paths
            .iter()
            .map(|p| {
                let mut path = p.clone();

                for part in &package.parts {
                    path.push(part);
                }

                path.set_extension(EXT);
                path
            })
            .collect();

        for path in &candidates {
            if !path.is_file() {
                continue;
            }

            files.push(path.clone());
        }

        if files.len() == 0 {
            let candidates_format: Vec<String> = candidates.iter()
                .map(|c| format!("{}", c.display()))
                .collect();

            let candidates_format = candidates_format.join(", ");

            return Err(format!("No files matching package ({}), expected one of: {}",
                               *package,
                               candidates_format)
                .into());
        }

        for path in files {
            self.import_file(&path, Some(package))?;
        }

        Ok(())
    }

    pub fn verify(&mut self) -> Result<()> {
        for (_, ref ty) in &self.decls {
            match ty.inner {
                RpDecl::Type(ref ty) => {
                    ty.verify()?;
                }
                _ => {}
            }
        }

        Ok(())
    }
}
