use std::collections::{HashMap, HashSet, VecDeque};

use tint_ast::{Item, UseDecl};

use super::utils::{path_string, use_string};
use super::{Module, ModuleTree};

pub(super) fn flatten(tree: &ModuleTree) -> Result<Vec<Item>, String> {
    let root = tree
        .get(&Vec::<String>::new())
        .ok_or_else(|| "resolver produced no root module".to_string())?;
    let mut merged: HashMap<String, (Vec<String>, Item)> = HashMap::new();
    let mut order = Vec::new();

    for item in &root.items {
        if let Some((name, _)) = export_info(item) {
            insert(
                &mut merged,
                &mut order,
                name.to_string(),
                vec![],
                item.clone(),
            )?;
        }
    }

    let mut visited = HashSet::new();
    let mut queue = VecDeque::from([Vec::<String>::new()]);
    while let Some(module_path) = queue.pop_front() {
        if !visited.insert(module_path.clone()) {
            continue;
        }
        let Some(module) = tree.get(&module_path) else {
            continue;
        };
        for item in &module.items {
            if let Item::Use(use_decl) = item {
                resolve_use(tree, module, use_decl, &mut merged, &mut order, &mut queue)?;
            }
        }
    }

    let mut output = root
        .items
        .iter()
        .filter(|item| !matches!(item, Item::Use(_)) && export_info(item).is_none())
        .cloned()
        .collect::<Vec<_>>();
    output.extend(
        order
            .into_iter()
            .map(|name| merged.remove(&name).unwrap().1),
    );
    Ok(output)
}

fn resolve_use(
    tree: &ModuleTree,
    importer: &Module,
    declaration: &UseDecl,
    merged: &mut HashMap<String, (Vec<String>, Item)>,
    order: &mut Vec<String>,
    queue: &mut VecDeque<Vec<String>>,
) -> Result<(), String> {
    if declaration.wildcard {
        let target = declaration.path.clone();
        let module = tree.get(&target).ok_or_else(|| {
            format!(
                "`use {}` in `{}` -- no module `{}`",
                use_string(declaration),
                path_string(&importer.path),
                path_string(&target)
            )
        })?;
        for item in &module.items {
            if let Some((name, true)) = export_info(item) {
                insert(
                    merged,
                    order,
                    name.to_string(),
                    target.clone(),
                    item.clone(),
                )?;
            }
        }
        queue.push_back(target);
        return Ok(());
    }
    if declaration.path.len() < 2 {
        return Err(format!(
            "`use {}` needs `module::item`",
            use_string(declaration)
        ));
    }
    let split = declaration.path.len() - 1;
    let module_path = declaration.path[..split].to_vec();
    let item_name = &declaration.path[split];
    let bound_name = declaration
        .alias
        .clone()
        .unwrap_or_else(|| item_name.clone());
    let module = tree.get(&module_path).ok_or_else(|| {
        format!(
            "`use {}` in `{}` -- no module `{}`",
            use_string(declaration),
            path_string(&importer.path),
            path_string(&module_path)
        )
    })?;
    let found = module.items.iter().find_map(|item| {
        export_info(item)
            .filter(|(name, _)| *name == item_name)
            .map(|(_, exported)| (item, exported))
    });
    match found {
        Some((item, true)) => {
            let item = declaration
                .alias
                .as_deref()
                .map_or_else(|| item.clone(), |alias| rename_item(item.clone(), alias));
            insert(merged, order, bound_name, module_path.clone(), item)?;
            queue.push_back(module_path);
            Ok(())
        }
        Some((_, false)) => Err(format!(
            "`use {}` in `{}` -- `{}` exists in `{}` but isn't exported (add `export` before it)",
            use_string(declaration),
            path_string(&importer.path),
            item_name,
            path_string(&module_path)
        )),
        None => Err(format!(
            "`use {}` in `{}` -- no exported item named `{}` in `{}`",
            use_string(declaration),
            path_string(&importer.path),
            item_name,
            path_string(&module_path)
        )),
    }
}

fn insert(
    merged: &mut HashMap<String, (Vec<String>, Item)>,
    order: &mut Vec<String>,
    name: String,
    origin: Vec<String>,
    item: Item,
) -> Result<(), String> {
    if let Some((existing, _)) = merged.get(&name) {
        if *existing == origin {
            return Ok(());
        }
        return Err(format!(
            "`{}` is defined in both `{}` and `{}`",
            name,
            path_string(existing),
            path_string(&origin)
        ));
    }
    order.push(name.clone());
    merged.insert(name, (origin, item));
    Ok(())
}

fn export_info(item: &Item) -> Option<(&str, bool)> {
    match item {
        Item::Fn(f) => Some((&f.name, f.exported)),
        Item::ExportFn(f, _) => Some((&f.name, true)),
        Item::Struct(s) => Some((&s.name, false)),
        Item::ExportStruct(s) => Some((&s.name, true)),
        Item::Enum(e) => Some((&e.name, false)),
        Item::ExportEnum(e) => Some((&e.name, true)),
        _ => None,
    }
}

fn rename_item(item: Item, name: &str) -> Item {
    match item {
        Item::Fn(mut f) => {
            f.name = name.into();
            Item::Fn(f)
        }
        Item::ExportFn(mut f, span) => {
            f.name = name.into();
            Item::ExportFn(f, span)
        }
        Item::Struct(mut s) => {
            s.name = name.into();
            Item::Struct(s)
        }
        Item::ExportStruct(mut s) => {
            s.name = name.into();
            Item::ExportStruct(s)
        }
        Item::Enum(mut e) => {
            e.name = name.into();
            Item::Enum(e)
        }
        Item::ExportEnum(mut e) => {
            e.name = name.into();
            Item::ExportEnum(e)
        }
        other => other,
    }
}
