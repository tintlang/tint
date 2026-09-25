#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LifetimeId(pub usize);

#[derive(Debug, Clone)]
pub struct LifetimeRegion {
    pub id: LifetimeId,
    pub parent: Option<LifetimeId>,
}
