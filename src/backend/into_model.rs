//! Implementations for converting asts into models.
use parser::ast;
use std::collections::{BTreeMap, HashSet};
use std::collections::btree_map;
use super::errors::*;
use super::merge::Merge;
use super::models::*;
use super::options::Options;

fn code(pos: &Pos, ast_pos: ast::Pos, context: String, lines: Vec<String>) -> Token<Code> {
    let pos = (pos.0.clone(), ast_pos.0, ast_pos.1);

    let code = Code {
        context: context,
        lines: lines,
    };

    Token::new(code, pos)
}

type Fields = Vec<Token<Field>>;
type Codes = Vec<Token<Code>>;
type OptionVec = Vec<Token<OptionDecl>>;

fn members_into_model(pos: &Pos,
                      members: Vec<ast::Token<ast::Member>>)
                      -> Result<(Fields, Codes, OptionVec, MatchDecl)> {
    let mut fields: Vec<Token<Field>> = Vec::new();
    let mut codes = Vec::new();
    let mut options: Vec<Token<OptionDecl>> = Vec::new();
    let mut match_decl = MatchDecl::new();

    for member in members {
        let pos = (pos.0.to_owned(), member.pos.0, member.pos.1);

        match member.inner {
            ast::Member::Field(field) => {
                let field = field.into_model(&pos)?;

                if let Some(other) = fields.iter().find(|f| f.name == field.name) {
                    return Err(Error::field_conflict(field.name.clone(), pos, other.pos.clone()));
                }

                fields.push(Token::new(field, pos));
            }
            ast::Member::Code(context, lines) => {
                codes.push(code(&pos, member.pos, context, lines));
            }
            ast::Member::Option(option) => {
                options.push(option.into_model(&pos)?);
            }
            ast::Member::Match(m) => {
                for member in m.members {
                    match_decl.push(member.into_model(&pos)?)?;
                }
            }
        }
    }

    Ok((fields, codes, options, match_decl))
}

struct OrdinalGenerator {
    next_ordinal: u32,
    ordinals: HashSet<u32>,
}

impl OrdinalGenerator {
    pub fn new() -> OrdinalGenerator {
        OrdinalGenerator {
            next_ordinal: 0,
            ordinals: HashSet::new(),
        }
    }

    pub fn next(&mut self, ordinal: &Option<ast::Token<ast::Value>>, pos: &Pos) -> Result<u32> {
        if let Some(ref ordinal) = *ordinal {
            let pos = (pos.0.to_owned(), ordinal.pos.0, ordinal.pos.1);

            if let ast::Value::Number(ref number) = ordinal.inner {
                let n: u32 = number.floor() as u32;

                if self.ordinals.contains(&n) {
                    return Err(Error::pos("duplicate ordinal".to_owned(), pos));
                }

                self.ordinals.insert(n);

                self.next_ordinal = n + 1;

                return Ok(n);
            }

            return Err(Error::pos("must be a number".to_owned(), pos));
        }

        let o = self.next_ordinal;

        self.next_ordinal += 1;

        if self.ordinals.contains(&o) {
            return Err(Error::pos(format!("generated ordinal {} conflicts with existing", o),
                                  pos.clone()));
        }

        Ok(o)
    }
}

/// Adds the into_model() method for all types that supports conversion into models.
pub trait IntoModel {
    type Output;

    /// Convert the current type to a model.
    fn into_model(self, pos: &Pos) -> Result<Self::Output>;
}

impl<T> IntoModel for ast::Token<T>
    where T: IntoModel
{
    type Output = Token<T::Output>;

    fn into_model(self, pos: &Pos) -> Result<Self::Output> {
        let pos = (pos.0.clone(), self.pos.0, self.pos.1);
        let out = self.inner.into_model(&pos)?;
        Ok(Token::new(out, pos))
    }
}

impl<T> IntoModel for Vec<T>
    where T: IntoModel
{
    type Output = Vec<T::Output>;

    fn into_model(self, pos: &Pos) -> Result<Self::Output> {
        let mut out = Vec::new();

        for v in self {
            out.push(v.into_model(pos)?);
        }

        Ok(out)
    }
}

impl<T> IntoModel for Option<T>
    where T: IntoModel
{
    type Output = Option<T::Output>;

    fn into_model(self, pos: &Pos) -> Result<Self::Output> {
        if let Some(value) = self {
            return Ok(Some(value.into_model(pos)?));
        }

        Ok(None)
    }
}

impl IntoModel for ast::InterfaceBody {
    type Output = InterfaceBody;

    fn into_model(self, pos: &Pos) -> Result<InterfaceBody> {
        let (fields, codes, options, match_decl) = members_into_model(&pos, self.members)?;

        let mut sub_types: BTreeMap<String, Token<SubType>> = BTreeMap::new();

        for sub_type in self.sub_types.into_model(pos)? {
            // key has to be owned by entry
            let key = sub_type.name.clone();

            match sub_types.entry(key) {
                btree_map::Entry::Occupied(entry) => {
                    entry.into_mut().merge(sub_type)?;
                }
                btree_map::Entry::Vacant(entry) => {
                    entry.insert(sub_type);
                }
            }
        }

        let _options = Options::new(&pos, options);

        let interface_body = InterfaceBody {
            name: self.name,
            fields: fields,
            codes: codes,
            match_decl: match_decl,
            sub_types: sub_types,
        };

        Ok(interface_body)
    }
}

impl IntoModel for ast::EnumBody {
    type Output = EnumBody;

    fn into_model(self, pos: &Pos) -> Result<EnumBody> {
        let mut values = Vec::new();

        let mut ordinals = OrdinalGenerator::new();

        let (fields, codes, options, match_decl) = members_into_model(pos, self.members)?;

        for value in self.values {
            let ordinal = ordinals.next(&value.ordinal, pos)?;
            /// need to tack on an ordinal value.
            values.push(value.map_inner(|v| (v, ordinal)).into_model(pos)?);
        }

        let options = Options::new(pos, options);

        let serialized_as: Option<Token<String>> = options.find_one_identifier("serialized_as")?
            .to_owned();

        let serialized_as_name = options.find_one_boolean("serialized_as_name")?
            .to_owned()
            .map(|t| t.inner)
            .unwrap_or(false);

        let en = EnumBody {
            name: self.name,
            values: values,
            fields: fields,
            codes: codes,
            match_decl: match_decl,
            serialized_as: serialized_as,
            serialized_as_name: serialized_as_name,
        };

        Ok(en)
    }
}

/// enum value with assigned ordinal
impl IntoModel for (ast::EnumValue, u32) {
    type Output = EnumValue;

    fn into_model(self, pos: &Pos) -> Result<Self::Output> {
        let value = self.0;
        let ordinal = self.1;

        let value = EnumValue {
            name: value.name,
            arguments: value.arguments.into_model(pos)?,
            ordinal: ordinal,
        };

        Ok(value)
    }
}

impl IntoModel for ast::TypeBody {
    type Output = TypeBody;

    fn into_model(self, pos: &Pos) -> Result<TypeBody> {
        let (fields, codes, options, match_decl) = members_into_model(&pos, self.members)?;

        let options = Options::new(&pos, options);

        let reserved: HashSet<Token<String>> =
            options.find_all_identifiers("reserved")?.into_iter().collect();

        let type_body = TypeBody {
            name: self.name,
            fields: fields,
            codes: codes,
            match_decl: match_decl,
            reserved: reserved,
        };

        Ok(type_body)
    }
}

impl IntoModel for ast::SubType {
    type Output = SubType;

    fn into_model(self, pos: &Pos) -> Result<SubType> {
        let mut fields: Vec<Token<Field>> = Vec::new();
        let mut codes = Vec::new();
        let mut options = Vec::new();

        for member in self.members {
            let pos = (pos.0.to_owned(), member.pos.0, member.pos.1);

            match member.inner {
                ast::Member::Field(field) => {
                    let field = field.into_model(&pos)?;

                    if let Some(other) = fields.iter().find(|f| f.name == field.name) {
                        return Err(Error::field_conflict(field.name.clone(),
                                                         pos,
                                                         other.pos.clone()));
                    }

                    fields.push(Token::new(field, pos));
                }
                ast::Member::Code(context, lines) => {
                    codes.push(code(&pos, member.pos, context, lines));
                }
                ast::Member::Option(option) => {
                    options.push(option.into_model(&pos)?);
                }
                _ => {
                    return Err(Error::pos("not supported".to_owned(), pos));
                }
            }
        }

        let options = Options::new(&pos, options);

        let names = options.find_all_strings("name")?;

        let sub_type = SubType {
            name: self.name,
            fields: fields,
            codes: codes,
            names: names,
        };

        Ok(sub_type)
    }
}

impl IntoModel for ast::TupleBody {
    type Output = TupleBody;

    fn into_model(self, pos: &Pos) -> Result<TupleBody> {
        let (fields, codes, options, match_decl) = members_into_model(&pos, self.members)?;

        let _options = Options::new(&pos, options);

        let tuple_body = TupleBody {
            name: self.name,
            fields: fields,
            codes: codes,
            match_decl: match_decl,
        };

        Ok(tuple_body)
    }
}

impl IntoModel for ast::Decl {
    type Output = Decl;

    fn into_model(self, pos: &Pos) -> Result<Decl> {
        let decl = match self {
            ast::Decl::Type(body) => Decl::Type(body.into_model(pos)?),
            ast::Decl::Interface(body) => Decl::Interface(body.into_model(pos)?),
            ast::Decl::Enum(body) => Decl::Enum(body.into_model(pos)?),
            ast::Decl::Tuple(body) => Decl::Tuple(body.into_model(pos)?),
        };

        Ok(decl)
    }
}

impl IntoModel for ast::OptionDecl {
    type Output = OptionDecl;

    fn into_model(self, pos: &Pos) -> Result<OptionDecl> {
        let decl = OptionDecl {
            name: self.name,
            values: self.values.into_model(pos)?,
        };

        Ok(decl)
    }
}

impl IntoModel for ast::Field {
    type Output = Field;

    fn into_model(self, pos: &Pos) -> Result<Field> {
        let field_as = self.field_as.into_model(pos)?;

        let field_as = if let Some(field_as) = field_as {
            if let Value::String(name) = field_as.inner {
                Some(Token::new(name, field_as.pos.clone()))
            } else {
                return Err(Error::pos("must be a string".to_owned(), field_as.pos));
            }
        } else {
            None
        };

        let field = Field {
            modifier: self.modifier,
            name: self.name,
            ty: self.ty,
            field_as: field_as,
        };

        Ok(field)
    }
}

impl IntoModel for ast::Value {
    type Output = Value;

    fn into_model(self, pos: &Pos) -> Result<Value> {
        let value = match self {
            ast::Value::String(string) => Value::String(string),
            ast::Value::Number(number) => Value::Number(number),
            ast::Value::Boolean(boolean) => Value::Boolean(boolean),
            ast::Value::Identifier(identifier) => Value::Identifier(identifier),
            ast::Value::Type(ty) => Value::Type(ty),
            ast::Value::Instance(instance) => Value::Instance(instance.into_model(pos)?),
            ast::Value::Constant(constant) => Value::Constant(constant.into_model(pos)?),
        };

        Ok(value)
    }
}

impl IntoModel for ast::FieldInit {
    type Output = FieldInit;

    fn into_model(self, pos: &Pos) -> Result<FieldInit> {
        let field_init = FieldInit {
            name: self.name.into_model(pos)?,
            value: self.value.into_model(pos)?,
        };

        Ok(field_init)
    }
}

impl IntoModel for ast::Instance {
    type Output = Instance;

    fn into_model(self, pos: &Pos) -> Result<Instance> {
        let instance = Instance {
            ty: self.ty,
            arguments: self.arguments.into_model(pos)?,
        };

        Ok(instance)
    }
}

impl IntoModel for ast::Constant {
    type Output = Constant;

    fn into_model(self, _pos: &Pos) -> Result<Constant> {
        let constant = Constant {
            prefix: self.prefix,
            parts: self.parts,
        };

        Ok(constant)
    }
}

impl IntoModel for String {
    type Output = String;

    fn into_model(self, _pos: &Pos) -> Result<String> {
        Ok(self)
    }
}

impl IntoModel for ast::MatchVariable {
    type Output = MatchVariable;

    fn into_model(self, pos: &Pos) -> Result<MatchVariable> {
        let match_variable = MatchVariable {
            name: self.name.into_model(pos)?,
            ty: self.ty,
        };

        Ok(match_variable)
    }
}

impl IntoModel for ast::MatchCondition {
    type Output = MatchCondition;

    fn into_model(self, pos: &Pos) -> Result<MatchCondition> {
        let match_condition = match self {
            ast::MatchCondition::Value(value) => MatchCondition::Value(value.into_model(pos)?),
            ast::MatchCondition::Type(ty) => MatchCondition::Type(ty.into_model(pos)?),
        };

        Ok(match_condition)
    }
}

impl IntoModel for ast::MatchMember {
    type Output = MatchMember;

    fn into_model(self, pos: &Pos) -> Result<MatchMember> {
        let member = MatchMember {
            condition: self.condition.into_model(pos)?,
            value: self.value.into_model(pos)?,
        };

        Ok(member)
    }
}
