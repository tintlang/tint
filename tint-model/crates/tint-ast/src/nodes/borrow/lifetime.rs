#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct LifetimeId(pub usize);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LifetimeRegion {
    pub id: LifetimeId,
    pub parent: Option<LifetimeId>,
}
