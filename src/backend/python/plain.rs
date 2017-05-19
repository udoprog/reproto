use backend::Backend;
use codegen::python::*;
use environment::Environment;
use options::Options;
use parser::ast;
use super::processor;

use errors::*;

pub struct PlainPythonBackend {
    staticmethod: BuiltInName,
    dict: BuiltInName,
}

impl PlainPythonBackend {
    pub fn new() -> PlainPythonBackend {
        PlainPythonBackend {
            staticmethod: Name::built_in("staticmethod"),
            dict: Name::built_in("dict"),
        }
    }

    fn encode_method<F>(&self,
                        processor: &processor::Processor,
                        package: &ast::Package,
                        fields: &Vec<processor::Field>,
                        builder: &BuiltInName,
                        field_set: F)
                        -> Result<MethodSpec>
        where F: Fn(&str, Statement) -> Statement
    {
        let mut encode = MethodSpec::new("encode");
        encode.push_argument(python_stmt!["self"]);
        encode.push(python_stmt!["data = ", builder, "()"]);

        for field in fields {
            let stmt = python_stmt!["self.", &field.name];
            let stmt = processor.encode(package, &field.ty, stmt)?;
            encode.push(field_set(&field.name, stmt));
        }

        encode.push(python_stmt!["return data"]);
        Ok(encode)
    }

    fn encode_tuple_method(&self,
                           processor: &processor::Processor,
                           package: &ast::Package,
                           fields: &Vec<processor::Field>)
                           -> Result<MethodSpec> {
        let mut values = Statement::new();

        let mut encode = MethodSpec::new("encode");
        encode.push_argument(python_stmt!["self"]);

        for field in fields {
            let stmt = python_stmt!["self.", &field.name];
            values.push(processor.encode(package, &field.ty, stmt)?);
        }

        encode.push(python_stmt!["return (", values.join(", "), ")"]);
        Ok(encode)
    }

    fn decode_method<F>(&self,
                        processor: &processor::Processor,
                        package: &ast::Package,
                        fields: &Vec<processor::Field>,
                        class: &ClassSpec,
                        variable_fn: F)
                        -> Result<MethodSpec>
        where F: Fn(usize, &str) -> Variable
    {
        let mut decode = MethodSpec::new("decode");
        decode.push_decorator(&self.staticmethod);
        decode.push_argument(python_stmt!["data"]);

        let mut arguments = Statement::new();

        for (i, field) in fields.iter().enumerate() {
            let var_name = format!("f_{}", field.name);

            let var_stmt = python_stmt!["data[", variable_fn(i, &field.name), "]"];

            let mut stmt = Statement::new();
            stmt.push(&var_name);
            stmt.push(" = ");
            stmt.push(processor.decode(package, &field.ty, var_stmt)?);
            decode.push(stmt);
            arguments.push(var_name);
        }

        let arguments = arguments.join(", ");
        decode.push(python_stmt!["return ", &class.name, "(", arguments, ")"]);

        Ok(decode)
    }
}

impl processor::Listeners for PlainPythonBackend {
    fn class_added(&self,
                   processor: &processor::Processor,
                   package: &ast::Package,
                   fields: &Vec<processor::Field>,
                   class: &mut ClassSpec)
                   -> Result<()> {

        let decode = self.decode_method(processor,
                           package,
                           fields,
                           class,
                           |_, name| Variable::String(name.to_owned()))?;

        let encode = self.encode_method(processor, package, fields, &self.dict, |name, stmt| {
                python_stmt!["data[", Variable::String(name.to_owned()), "] = ", stmt]
            })?;

        class.push(decode);
        class.push(encode);
        Ok(())
    }

    fn tuple_added(&self,
                   processor: &processor::Processor,
                   package: &ast::Package,
                   fields: &Vec<processor::Field>,
                   class: &mut ClassSpec)
                   -> Result<()> {

        let decode = self.decode_method(processor,
                           package,
                           fields,
                           class,
                           |i, _| Variable::Literal(i.to_string()))?;

        let encode = self.encode_tuple_method(processor, package, fields)?;

        class.push(decode);
        class.push(encode);
        Ok(())
    }

    fn interface_added(&self,
                       _processor: &processor::Processor,
                       _package: &ast::Package,
                       interface: &ast::InterfaceDecl,
                       interface_spec: &mut ClassSpec)
                       -> Result<()> {
        let mut decode = MethodSpec::new("decode");
        decode.push_decorator(&self.staticmethod);
        decode.push_argument(python_stmt!["data"]);

        let mut decode_body = Vec::new();

        decode_body.push(python_stmt!["type = data[", Variable::String("type".to_owned()), "]"]
            .as_element_spec());

        for (_, ref sub_type) in &interface.sub_types {
            for name in sub_type.options.lookup_string("name") {
                let if_stmt = python_stmt!["if type == ", Variable::String(name.to_owned()), ":"];

                let name = Name::local(&sub_type.name).as_name();

                let if_body = python_stmt!["return ", name, ".decode(data)"];

                let mut check: Vec<ElementSpec> = Vec::new();

                check.push(if_stmt.as_element_spec());
                check.push(ElementSpec::Nested(Box::new(if_body.as_element_spec())));

                decode_body.push(check.as_element_spec());
            }
        }

        let raise =
            python_stmt!["raise Exception(", Variable::String("bad type".to_owned()), " + type)"]
                .as_element_spec();
        decode_body.push(raise);

        decode.push(decode_body.as_element_spec().join(ElementSpec::Spacing));
        interface_spec.push(decode);

        Ok(())
    }
}

impl Backend for PlainPythonBackend {
    fn process(&self, options: &Options, env: &Environment) -> Result<()> {
        let package_prefix = options.package_prefix
            .clone()
            .map(|prefix| ast::Package::new(prefix.split(".").map(ToOwned::to_owned).collect()));

        let processor = processor::Processor::new(options, env, package_prefix);
        processor.process(self)
    }
}
