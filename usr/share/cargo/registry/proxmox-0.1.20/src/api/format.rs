//! Module to generate and format API Documenation

use failure::Error;

use std::io::Write;

use crate::api::schema::*;
use crate::api::{ApiHandler, ApiMethod};

/// Enumerate different styles to display parameters/properties.
#[derive(Copy, Clone)]
pub enum ParameterDisplayStyle {
    /// Used for properties in configuration files: ``key:``
    Config,
    //SonfigSub,
    /// Used for command line options: ``--key``
    Arg,
    /// Used for command line options passed as arguments: ``<key>``
    Fixed,
}

/// CLI usage information format.
#[derive(Copy, Clone, PartialEq)]
pub enum DocumentationFormat {
    /// Text, command line only (one line).
    Short,
    /// Text, list all options.
    Long,
    /// Text, include description.
    Full,
    /// Like full, but in reStructuredText format.
    ReST,
}

/// Line wrapping to form simple list of paragraphs.
pub fn wrap_text(
    initial_indent: &str,
    subsequent_indent: &str,
    text: &str,
    columns: usize,
) -> String {
    let wrapper1 = textwrap::Wrapper::new(columns)
        .initial_indent(initial_indent)
        .subsequent_indent(subsequent_indent);

    let wrapper2 = textwrap::Wrapper::new(columns)
        .initial_indent(subsequent_indent)
        .subsequent_indent(subsequent_indent);

    text.split("\n\n")
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .fold(String::new(), |mut acc, p| {
            if acc.is_empty() {
                acc.push_str(&wrapper1.wrap(p).join("\n"));
            } else {
                acc.push_str(&wrapper2.wrap(p).join("\n"));
            }
            acc.push_str("\n\n");
            acc
        })
}

#[test]
fn test_wrap_text() {
    let text = "Command. This may be a list in order to spefify nested sub-commands.";
    let expect = "             Command. This may be a list in order to spefify nested sub-\n             commands.\n\n";

    let indent = "             ";
    let wrapped = wrap_text(indent, indent, text, 80);

    assert_eq!(wrapped, expect);
}

/// Helper to format the type text
///
/// The result is a short string including important constraints, for
/// example ``<integer> (0 - N)``.
pub fn get_schema_type_text(schema: &Schema, _style: ParameterDisplayStyle) -> String {
    match schema {
        Schema::Null => String::from("<null>"), // should not happen
        Schema::String(_) => String::from("<string>"),
        Schema::Boolean(_) => String::from("<boolean>"),
        Schema::Integer(integer_schema) => match (integer_schema.minimum, integer_schema.maximum) {
            (Some(min), Some(max)) => format!("<integer> ({} - {})", min, max),
            (Some(min), None) => format!("<integer> ({} - N)", min),
            (None, Some(max)) => format!("<integer> (-N - {})", max),
            _ => String::from("<integer>"),
        },
        Schema::Number(number_schema) => match (number_schema.minimum, number_schema.maximum) {
            (Some(min), Some(max)) => format!("<number> ({} - {})", min, max),
            (Some(min), None) => format!("<number> ({} - N)", min),
            (None, Some(max)) => format!("<number> (-N - {})", max),
            _ => String::from("<integer>"),
        },
        Schema::Object(_) => String::from("<object>"),
        Schema::Array(_) => String::from("<array>"),
    }
}

/// Helper to format an object property, including name, type and description.
pub fn get_property_description(
    name: &str,
    schema: &Schema,
    style: ParameterDisplayStyle,
    format: DocumentationFormat,
) -> String {
    let type_text = get_schema_type_text(schema, style);

    let (descr, default) = match schema {
        Schema::Null => ("null", None),
        Schema::String(ref schema) => (schema.description, schema.default.map(|v| v.to_owned())),
        Schema::Boolean(ref schema) => (schema.description, schema.default.map(|v| v.to_string())),
        Schema::Integer(ref schema) => (schema.description, schema.default.map(|v| v.to_string())),
        Schema::Number(ref schema) => (schema.description, schema.default.map(|v| v.to_string())),
        Schema::Object(ref schema) => (schema.description, None),
        Schema::Array(ref schema) => (schema.description, None),
    };

    let default_text = match default {
        Some(text) => format!("   (default={})", text),
        None => String::new(),
    };

    if format == DocumentationFormat::ReST {
        let mut text = match style {
            ParameterDisplayStyle::Config => {
                format!(":``{} {}{}``:  ", name, type_text, default_text)
            }
            ParameterDisplayStyle::Arg => {
                format!(":``--{} {}{}``:  ", name, type_text, default_text)
            }
            ParameterDisplayStyle::Fixed => {
                format!(":``<{}> {}{}``:  ", name, type_text, default_text)
            }
        };

        text.push_str(&wrap_text("", "  ", descr, 80));
        text.push('\n');

        text
    } else {
        let display_name = match style {
            ParameterDisplayStyle::Config => format!("{}:", name),
            ParameterDisplayStyle::Arg => format!("--{}", name),
            ParameterDisplayStyle::Fixed => format!("<{}>", name),
        };

        let mut text = format!(" {:-10} {}{}", display_name, type_text, default_text);
        let indent = "             ";
        text.push('\n');
        text.push_str(&wrap_text(indent, indent, descr, 80));

        text
    }
}

fn dump_api_parameters(param: &ObjectSchema) -> String {
    let mut res = wrap_text("", "", param.description, 80);

    let mut required_list: Vec<String> = Vec::new();
    let mut optional_list: Vec<String> = Vec::new();

    for (prop, optional, schema) in param.properties {
        let param_descr = get_property_description(
            prop,
            &schema,
            ParameterDisplayStyle::Config,
            DocumentationFormat::ReST,
        );

        if *optional {
            optional_list.push(param_descr);
        } else {
            required_list.push(param_descr);
        }
    }

    if !required_list.is_empty() {
        res.push_str("\n*Required properties:*\n\n");

        for text in required_list {
            res.push_str(&text);
            res.push('\n');
        }
    }

    if !optional_list.is_empty() {
        res.push_str("\n*Optional properties:*\n\n");

        for text in optional_list {
            res.push_str(&text);
            res.push('\n');
        }
    }

    res
}

fn dump_api_return_schema(schema: &Schema) -> String {
    let mut res = String::from("*Returns*: ");

    let type_text = get_schema_type_text(schema, ParameterDisplayStyle::Config);
    res.push_str(&format!("**{}**\n\n", type_text));

    match schema {
        Schema::Null => {
            return res;
        }
        Schema::Boolean(schema) => {
            let description = wrap_text("", "", schema.description, 80);
            res.push_str(&description);
        }
        Schema::Integer(schema) => {
            let description = wrap_text("", "", schema.description, 80);
            res.push_str(&description);
        }
        Schema::Number(schema) => {
            let description = wrap_text("", "", schema.description, 80);
            res.push_str(&description);
        }
        Schema::String(schema) => {
            let description = wrap_text("", "", schema.description, 80);
            res.push_str(&description);
        }
        Schema::Array(schema) => {
            let description = wrap_text("", "", schema.description, 80);
            res.push_str(&description);
        }
        Schema::Object(obj_schema) => {
            res.push_str(&dump_api_parameters(obj_schema));
        }
    }

    res.push('\n');

    res
}

fn dump_method_definition(method: &str, path: &str, def: Option<&ApiMethod>) -> Option<String> {
    match def {
        None => None,
        Some(api_method) => {
            let param_descr = dump_api_parameters(api_method.parameters);

            let return_descr = dump_api_return_schema(api_method.returns);

            let mut method = method;

            if let ApiHandler::AsyncHttp(_) = api_method.handler {
                method = if method == "POST" { "UPLOAD" } else { method };
                method = if method == "GET" { "DOWNLOAD" } else { method };
            }

            let res = format!(
                "**{} {}**\n\n{}\n\n{}",
                method, path, param_descr, return_descr
            );
            Some(res)
        }
    }
}

/// Generate ReST Documentaion for a complete API defined by a ``Router``.
pub fn dump_api(
    output: &mut dyn Write,
    router: &crate::api::Router,
    path: &str,
    mut pos: usize,
) -> Result<(), Error> {
    use crate::api::SubRoute;

    let mut cond_print = |x| -> Result<_, Error> {
        if let Some(text) = x {
            if pos > 0 {
                writeln!(output, "-----\n")?;
            }
            writeln!(output, "{}", text)?;
            pos += 1;
        }
        Ok(())
    };

    cond_print(dump_method_definition("GET", path, router.get))?;
    cond_print(dump_method_definition("POST", path, router.post))?;
    cond_print(dump_method_definition("PUT", path, router.put))?;
    cond_print(dump_method_definition("DELETE", path, router.delete))?;

    match &router.subroute {
        None => return Ok(()),
        Some(SubRoute::MatchAll { router, param_name }) => {
            let sub_path = if path == "." {
                format!("<{}>", param_name)
            } else {
                format!("{}/<{}>", path, param_name)
            };
            dump_api(output, router, &sub_path, pos)?;
        }
        Some(SubRoute::Map(dirmap)) => {
            //let mut keys: Vec<&String> = map.keys().collect();
            //keys.sort_unstable_by(|a, b| a.cmp(b));
            for (key, sub_router) in dirmap.iter() {
                let sub_path = if path == "." {
                    (*key).to_string()
                } else {
                    format!("{}/{}", path, key)
                };
                dump_api(output, sub_router, &sub_path, pos)?;
            }
        }
    }

    Ok(())
}
