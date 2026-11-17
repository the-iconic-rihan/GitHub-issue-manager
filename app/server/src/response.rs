//! Success envelopes: `{"data": ...}` or `{"data": [...], "meta": {pagination}}`.

use actix_web::HttpResponse;
use middleware::github::Page;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PageMeta {
    pub page: u32,
    pub per_page: u32,
    pub has_next: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_count: Option<u64>,
}

#[derive(Serialize)]
struct Envelope<T> {
    data: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    meta: Option<PageMeta>,
}

pub fn ok<T: Serialize>(data: T) -> HttpResponse {
    HttpResponse::Ok().json(Envelope { data, meta: None })
}

pub fn created<T: Serialize>(data: T) -> HttpResponse {
    HttpResponse::Created().json(Envelope { data, meta: None })
}

pub fn paged<T: Serialize>(items: Vec<T>, meta: PageMeta) -> HttpResponse {
    HttpResponse::Ok().json(Envelope {
        data: items,
        meta: Some(meta),
    })
}

pub fn from_page<T: Serialize>(page: Page<T>) -> HttpResponse {
    let meta = PageMeta {
        page: page.page,
        per_page: page.per_page,
        has_next: page.has_next,
        total_count: page.total_count,
    };
    paged(page.items, meta)
}

pub fn no_content() -> HttpResponse {
    HttpResponse::NoContent().finish()
}
