//! Versioned serialized Tint program format.
//!
//! This is intentionally a small, stable container around the current AST
//! program representation. It lets the browser load a compiled Tint artifact
//! through the same VM as source mode. The payload can later be replaced by a
//! denser SSA bytecode representation without changing the host API.

use tint_ast::Program;

const MAGIC: &[u8; 8] = b"TINTBC01";

#[derive(Debug, serde::Serialize)]
struct Container<'a> {
    version: u16,
    program: &'a Program,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct OwnedContainer {
    version: u16,
    program: Program,
}

pub fn encode(program: &Program) -> Result<Vec<u8>, String> {
    let payload = serde_json::to_vec(&Container {
        version: 1,
        program,
    })
    .map_err(|error| format!("failed to encode Tint bytecode: {error}"))?;
    let mut output = Vec::with_capacity(MAGIC.len() + payload.len());
    output.extend_from_slice(MAGIC);
    output.extend_from_slice(&payload);
    Ok(output)
}

pub fn decode(bytes: &[u8]) -> Result<Program, String> {
    if bytes.len() < MAGIC.len() || &bytes[..MAGIC.len()] != MAGIC {
        return Err("invalid Tint bytecode header".to_string());
    }
    let container: OwnedContainer = serde_json::from_slice(&bytes[MAGIC.len()..])
        .map_err(|error| format!("invalid Tint bytecode payload: {error}"))?;
    if container.version != 1 {
        return Err(format!(
            "unsupported Tint bytecode version {}",
            container.version
        ));
    }
    Ok(container.program)
}
