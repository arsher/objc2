use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use crate::config::LibraryData;
use crate::file::File;

/// Some SDK files have '+' in the file name, so we change those to `_`.
pub(crate) fn clean_file_name(name: &str) -> String {
    name.replace('+', "_")
}

#[derive(Debug, PartialEq, Default)]
pub struct Library {
    pub files: BTreeMap<String, File>,
    link_name: String,
    data: LibraryData,
}

impl Library {
    pub fn new(name: &str, data: &LibraryData) -> Self {
        Self {
            files: BTreeMap::new(),
            link_name: name.to_string(),
            data: data.clone(),
        }
    }

    pub fn output(&self, path: &Path) -> io::Result<()> {
        for (name, file) in &self.files {
            let name = clean_file_name(name);
            let mut path = path.join(name);
            path.set_extension("rs");
            fs::write(&path, file.to_string())?;
        }

        // truncate if the file exists
        fs::write(path.join("mod.rs"), self.to_string())?;

        Ok(())
    }
}

impl fmt::Display for Library {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "// This file has been automatically generated by `objc2`'s `header-translator`."
        )?;
        writeln!(f, "// DO NOT EDIT")?;
        writeln!(f)?;
        writeln!(f, "//! # Bindings to the `{}` framework", self.link_name)?;

        // Lints
        // We emit `use [framework]::*` more than necessary often.
        writeln!(f, "#![allow(unused_imports)]")?;
        // Deprecated items are often still used in other signatures.
        writeln!(f, "#![allow(deprecated)]")?;
        // Methods use a different naming scheme.
        writeln!(f, "#![allow(non_snake_case)]")?;
        // We emit C types with a different naming scheme.
        writeln!(f, "#![allow(non_camel_case_types)]")?;
        // Statics and enum fields use a different naming scheme.
        writeln!(f, "#![allow(non_upper_case_globals)]")?;
        // We don't yet emit documentation for methods.
        writeln!(f, "#![allow(missing_docs)]")?;

        // Clippy lints
        // We have no control over how many arguments a method takes.
        writeln!(f, "#![allow(clippy::too_many_arguments)]")?;
        // We have no control over how complex a type is.
        writeln!(f, "#![allow(clippy::type_complexity)]")?;
        // Apple's naming scheme allows this.
        writeln!(f, "#![allow(clippy::upper_case_acronyms)]")?;
        // Headers often use `x << 0` for clarity.
        writeln!(f, "#![allow(clippy::identity_op)]")?;
        // We don't have the manpower to document the safety of methods.
        writeln!(f, "#![allow(clippy::missing_safety_doc)]")?;

        writeln!(f)?;

        // Link to the correct framework.
        if self.data.cfg_apple_link {
            // Allow a different linking on GNUStep
            writeln!(
                f,
                "#[cfg_attr(feature = \"apple\", link(name = \"{}\", kind = \"framework\"))]",
                self.link_name
            )?;
        } else {
            writeln!(
                f,
                "#[link(name = \"{}\", kind = \"framework\")]",
                self.link_name
            )?;
        }
        writeln!(f, "extern \"C\" {{}}")?;
        writeln!(f)?;

        for name in self.files.keys() {
            let name = clean_file_name(name);
            writeln!(f, "#[path = \"{name}.rs\"]")?;
            writeln!(f, "mod __{name};")?;
        }

        writeln!(f)?;

        for (file_name, file) in &self.files {
            for stmt in &file.stmts {
                let features = stmt.required_features();

                if let Some(item) = stmt.provided_item() {
                    assert_eq!(item.file_name.as_ref(), Some(file_name));

                    write!(f, "{}", features.cfg_gate_ln())?;

                    let visibility = if item.name.starts_with('_') {
                        "pub(crate)"
                    } else {
                        "pub"
                    };

                    write!(
                        f,
                        "{visibility} use self::__{}::{{{}}};",
                        clean_file_name(file_name),
                        item.name,
                    )?;
                }
            }
        }

        Ok(())
    }
}
