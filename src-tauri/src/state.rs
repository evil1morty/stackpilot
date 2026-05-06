use crate::catalog::CatalogCache;

#[derive(Default)]
pub struct AppState {
    pub catalog: CatalogCache,
}
