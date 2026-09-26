use tint_ast::{AttributeList, FnDecl};

static KNOWN_ATTRIBUTES: &[&str] = &["strict", "speed", "core", "inline", "noinline"];

pub fn validate_attributes(attrs: &AttributeList) -> Result<(), String> {
    for a in &attrs.items {
        if !KNOWN_ATTRIBUTES.contains(&a.name.as_str()) {
            return Err(format!("Unknown attribute '@({})'", a.name));
        }
    }
    Ok(())
}

pub fn validate_fn_decl(f: &FnDecl) -> Result<(), String> {
    validate_attributes(&f.attributes)?;

    // другие проверки…
    Ok(())
}
