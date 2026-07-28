use std::collections::HashMap;

use a3s_acl::{Block, Document, Value as AclValue};

use super::*;

const SOURCE_SET_PATH: &str = ".a3s/source-set.acl";
const SOURCE_SET_SCHEMA: &str = "a3s.knowledge-source-set.v1";

pub(super) fn read_source_set(base: &Path) -> Result<Option<Vec<SourceRoot>>, KnowledgeStoreError> {
    let path = base.join(SOURCE_SET_PATH);
    let source = match std::fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_error(&path, error)),
    };
    let document = a3s_acl::parse_acl(&source).map_err(|error| {
        KnowledgeStoreError::Invalid(format!("invalid {}: {error}", path.display()))
    })?;
    let source_set = document
        .blocks
        .iter()
        .find(|block| block.name == "source_set")
        .ok_or_else(|| {
            KnowledgeStoreError::Invalid(format!("{} has no source_set block", path.display()))
        })?;
    let schema = required_string(source_set, "schema", &path)?;
    if schema != SOURCE_SET_SCHEMA {
        return Err(KnowledgeStoreError::Invalid(format!(
            "unsupported knowledge source-set schema `{schema}`"
        )));
    }
    let mut roots = Vec::new();
    for block in source_set
        .blocks
        .iter()
        .filter(|block| block.name == "source")
    {
        let origin = PathBuf::from(required_string(block, "path", &path)?);
        let destination = safe_relative_path(&required_string(block, "destination", &path)?)?;
        let kind = SourceRootKind::parse(&required_string(block, "kind", &path)?)?;
        roots.push(SourceRoot {
            path: origin,
            destination,
            kind,
        });
    }
    if roots.is_empty() {
        return Err(KnowledgeStoreError::Invalid(format!(
            "{} contains no source roots",
            path.display()
        )));
    }
    Ok(Some(roots))
}

pub(super) fn write_source_set(
    base: &Path,
    roots: &[SourceRoot],
) -> Result<(), KnowledgeStoreError> {
    let path = base.join(SOURCE_SET_PATH);
    let blocks = roots
        .iter()
        .enumerate()
        .map(|(index, root)| Block {
            name: "source".to_string(),
            labels: vec![format!("source-{}", index + 1)],
            blocks: Vec::new(),
            attributes: HashMap::from([
                (
                    "path".to_string(),
                    AclValue::String(root.path.display().to_string()),
                ),
                (
                    "destination".to_string(),
                    AclValue::String(
                        normalized_relative_path(&root.destination).unwrap_or_default(),
                    ),
                ),
                (
                    "kind".to_string(),
                    AclValue::String(root.kind.as_str().to_string()),
                ),
            ]),
        })
        .collect();
    let document = Document {
        blocks: vec![Block {
            name: "source_set".to_string(),
            labels: Vec::new(),
            blocks,
            attributes: HashMap::from([(
                "schema".to_string(),
                AclValue::String(SOURCE_SET_SCHEMA.to_string()),
            )]),
        }],
    };
    let rendered = a3s_acl::generate_acl(&document);
    a3s_acl::parse_acl(&rendered).map_err(|error| {
        KnowledgeStoreError::Invalid(format!("generated source-set ACL is invalid: {error}"))
    })?;
    atomic_write(&path, rendered.as_bytes())
}

fn required_string(block: &Block, key: &str, path: &Path) -> Result<String, KnowledgeStoreError> {
    block
        .attributes
        .get(key)
        .and_then(AclValue::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            KnowledgeStoreError::Invalid(format!(
                "{} is missing string attribute `{key}`",
                path.display()
            ))
        })
}
