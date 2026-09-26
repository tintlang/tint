/// One node's computed layout, in ABSOLUTE pixel coordinates (i.e.
/// already accumulated from root, not just relative to its parent --
/// what a renderer actually needs to draw it).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeLayout {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Computes layout for `tree` rooted at `root`, inside a viewport of
/// `available_width` x `available_height` pixels. Returns every visited
/// node's absolute layout keyed by `UiNodeId`.
pub fn compute_layout(
    tree: &UiTree,
    root: UiNodeId,
    available_width: f32,
    available_height: f32,
) -> HashMap<UiNodeId, NodeLayout> {
    let mut taffy_tree: TaffyTree<()> = TaffyTree::new();
    let mut tint_to_taffy: HashMap<UiNodeId, NodeId> = HashMap::new();

    fn build(
        tree: &UiTree,
        id: UiNodeId,
        taffy_tree: &mut TaffyTree<()>,
        tint_to_taffy: &mut HashMap<UiNodeId, NodeId>,
    ) -> NodeId {
        let node = &tree.nodes[id];
        let child_ids: Vec<NodeId> = node
            .children
            .iter()
            .map(|&c| build(tree, c, taffy_tree, tint_to_taffy))
            .collect();
        let tid = taffy_tree
            .new_with_children(taffy_style(&node.style), &child_ids)
            .expect("taffy node creation");
        tint_to_taffy.insert(id, tid);
        tid
    }

    let taffy_root = build(tree, root, &mut taffy_tree, &mut tint_to_taffy);

    // The root has no parent to stretch against (an ordinary block
    // child's auto width fills its parent, but the root itself has
    // none), so taffy leaves an auto-sized root at its content size
    // instead of the viewport -- same reason a browser gives `html`/
    // `body` an explicit 100% size instead of leaving them auto. Force
    // the root to the full viewport here, the same role.
    let mut root_style = taffy_tree.style(taffy_root).expect("root style").clone();
    root_style.size = Size {
        width: Dimension::length(available_width),
        height: Dimension::length(available_height),
    };
    // A block box (which is what the root logically is, same as
    // `html`/`body` in a page -- children stack vertically and stretch
    // to fill its width by default) rather than taffy's own default
    // (a flex row), so an unstyled top-level node actually gets the
    // full viewport width instead of shrinking to its content.
    root_style.display = Display::Block;
    taffy_tree
        .set_style(taffy_root, root_style)
        .expect("root style override");

    taffy_tree
        .compute_layout(
            taffy_root,
            Size {
                width: AvailableSpace::Definite(available_width),
                height: AvailableSpace::Definite(available_height),
            },
        )
        .expect("taffy layout");

    // taffy hands back each node's rect relative to its own parent;
    // walk down from root accumulating parent offsets so callers get
    // absolute on-screen coordinates without having to know the tree
    // shape themselves.
    let mut out = HashMap::new();

    fn collect(
        tree: &UiTree,
        id: UiNodeId,
        taffy_tree: &TaffyTree<()>,
        tint_to_taffy: &HashMap<UiNodeId, NodeId>,
        parent_x: f32,
        parent_y: f32,
        out: &mut HashMap<UiNodeId, NodeLayout>,
    ) {
        let tid = tint_to_taffy[&id];
        let layout = taffy_tree.layout(tid).expect("taffy layout lookup");
        let x = parent_x + layout.location.x;
        let y = parent_y + layout.location.y;
        out.insert(
            id,
            NodeLayout {
                x,
                y,
                width: layout.size.width,
                height: layout.size.height,
            },
        );
        for &child in &tree.nodes[id].children {
            collect(tree, child, taffy_tree, tint_to_taffy, x, y, out);
        }
    }

    collect(tree, root, &taffy_tree, &tint_to_taffy, 0.0, 0.0, &mut out);

    out
}
