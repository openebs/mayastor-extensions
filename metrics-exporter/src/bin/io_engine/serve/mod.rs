use actix_web::web;
/// module for prometheus handlers.
mod handler;

pub(crate) fn metric_route(cfg: &mut web::ServiceConfig) {
    cfg.route("/metrics", web::get().to(handler::metrics_handler));
}
